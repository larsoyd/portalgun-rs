// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::collections::HashMap;

use bevy::prelude::*;
use portal::anim::cycle_rate;
use portal::brush;
use portal::entity::io::Handle;
use portal::entity::{Env, Sequence, Shape, World};
use portal::mdl::Mdl;
use portal::pair::Vec3;
use portal::vpk::SearchPath;

use crate::chamber::{box_meets, Chamber};
use crate::faces::Portals;
use crate::firing::{fizzle_all, Firing};
use crate::objects::Objects;
use crate::player::Player;
use crate::scene::Level;
use crate::simulator::Space;
use crate::sound::{Files, Sounds};
use crate::walker::Walker;

/// The chamber's entities, run by the original's I/O and class rules.
#[derive(Resource)]
pub struct Entities {
    pub world: World,
    pub player: Handle,
    /// the prop the cube stands for, the map's box or the last copy a maker dropped
    pub cube: Option<Handle>,
    held: bool,
    playing: HashMap<(Handle, String), Entity>,
}

fn sequences(search: &SearchPath, model: &str) -> Result<Vec<Sequence>, String> {
    let mdl = Mdl::parse(search.read(model).map_err(|e| format!("{model}: {e}"))?)
        .map_err(|e| format!("{model}: {e}"))?;
    mdl.sequences
        .iter()
        .map(|s| {
            if s.nodes != [0, 0] {
                return Err(format!(
                    "{model} sequence {} has transition nodes, not ported",
                    s.label
                ));
            }
            let rate = mdl.anims.get(s.anim).map_or(0.0, cycle_rate);
            Ok(Sequence {
                name: s.label.clone(),
                duration: if rate > 0.0 { 1.0 / rate } else { 0.0 },
                looping: s.looping,
            })
        })
        .collect()
}

impl Entities {
    /// Fails closed on anything the world cannot run as the original does.
    pub fn build(chamber: &Chamber, search: &SearchPath) -> Result<Entities, String> {
        let mut models = HashMap::new();
        for e in chamber
            .entities
            .iter()
            .filter(|e| e.classname.starts_with("prop_dynamic"))
        {
            let model = e.get("model").unwrap_or_default().to_string();
            if !models.contains_key(&model) {
                models.insert(model.clone(), sequences(search, &model)?);
            }
        }
        let mut world = World::spawn_with(&chamber.entities, |e| {
            let polys: Vec<_> = e
                .solids
                .iter()
                .flat_map(|s| brush::faces(&s.sides))
                .collect();
            Shape {
                bounds: (!polys.is_empty()).then(|| brush::bounds(&polys)),
                sequences: e
                    .get("model")
                    .and_then(|m| models.get(m))
                    .cloned()
                    .unwrap_or_default(),
            }
        });
        if let Some(reason) = world.refusals().first() {
            return Err(reason.clone());
        }
        let player = world.add_player();
        let cube = world
            .find("box")
            .filter(|&h| world.entity(h).classname == "prop_physics");
        info!(
            "[entity] spawned entities={} cube={}",
            chamber.entities.len(),
            cube.map_or("none".into(), |h| h.0.to_string())
        );
        Ok(Entities {
            world,
            player,
            cube,
            held: false,
            playing: HashMap::new(),
        })
    }
}

/// Translated from 0x861000 and 0x8607e0 (decompiled, server.so), active effects start at activation
pub fn start_particles(
    chamber: &Chamber,
    fx: &mut crate::effects::ParticleEffects,
) -> Result<usize, String> {
    let mut started = 0;
    for e in chamber
        .entities
        .iter()
        .filter(|e| e.classname == "info_particle_system")
    {
        if e.get("start_active") != Some("1") {
            continue;
        }
        let name = e.get("effect_name").unwrap_or_default();
        let origin = e.origin().unwrap_or_default();
        let mut angles = [0.0; 3];
        for (a, v) in angles
            .iter_mut()
            .zip(e.get("angles").unwrap_or_default().split_whitespace())
        {
            *a = v
                .parse()
                .map_err(|_| format!("info_particle_system {name} angles unreadable"))?;
        }
        fx.spawn_named(name, crate::effects::Frame::entity(origin, angles))?;
        info!("[entity] particle name={name} at={origin:?} angles={angles:?}");
        started += 1;
    }
    Ok(started)
}

/// what one tick of the entity world reaches in the game around it
struct Game<'a> {
    level: &'a mut Chamber,
    walker: &'a mut Walker,
    objects: &'a mut Objects,
    gun: &'a mut Firing,
    portals: &'a mut Portals,
    visual: Option<&'a mut crate::gun_visual::GunVisualState>,
    player: Handle,
    cube: Option<Handle>,
}

impl Game<'_> {
    fn triggers(&self, centre: Vec3, half: Vec3) -> Vec<Handle> {
        let mut hits: Vec<Handle> = self
            .level
            .volumes
            .iter()
            .filter(|v| box_meets(&v.planes, centre, half))
            .map(|v| Handle(v.owner as u32))
            .collect();
        hits.dedup();
        hits
    }

    fn player_stuck(&self) -> bool {
        let half = self.walker.half();
        let centre = self.walker.centre();
        let lo = half.map(|h| -h);
        Space::new(self.level, None)
            .trace(None, centre, centre, lo, half)
            .start_solid
    }
}

impl Env for Game<'_> {
    /// Translated from 0x8868a0 (decompiled, server.so), riders move with the pusher, a stuck one blocks it
    fn push(&mut self, pusher: Handle, delta: Vec3) -> Option<Handle> {
        let owner = pusher.0 as usize;
        let riding = self.walker.ground.is_some_and(|b| {
            self.level
                .blocks
                .get(b)
                .is_some_and(|b| b.owner == Some(owner))
        });
        self.level.move_owner(owner, delta);
        let swept = self.player_stuck();
        if !riding && !swept {
            return None;
        }
        let back = delta.map(|v| -v);
        let feet = self.walker.feet;
        self.walker.feet = [0, 1, 2].map(|i| feet[i] + delta[i]);
        if self.player_stuck() {
            self.walker.feet = feet;
            self.level.move_owner(owner, back);
            return Some(self.player);
        }
        None
    }

    fn overlapping(&mut self, trigger: Handle) -> Vec<Handle> {
        let mut inside = Vec::new();
        if self
            .triggers(self.walker.centre(), self.walker.half())
            .contains(&trigger)
        {
            inside.push(self.player);
        }
        if let Some(cube) = self.cube.filter(|_| self.objects.present) {
            if self
                .triggers(self.objects.cube.position, self.objects.cube.half())
                .contains(&trigger)
            {
                inside.push(cube);
            }
        }
        inside
    }

    fn movers(&mut self) -> Vec<(Handle, Vec<Handle>)> {
        let mut movers = vec![(
            self.player,
            self.triggers(self.walker.centre(), self.walker.half()),
        )];
        if let Some(cube) = self
            .cube
            .filter(|_| self.objects.present && !self.objects.body.asleep)
        {
            movers.push((
                cube,
                self.triggers(self.objects.cube.position, self.objects.cube.half()),
            ));
        }
        movers
    }

    fn fizzle(&mut self, _player: Handle) -> bool {
        fizzle_all(
            self.gun,
            self.portals,
            self.visual.as_deref_mut(),
            "cleanser",
        )
    }

    fn dissolve(&mut self, entity: Handle) {
        if Some(entity) == self.cube {
            self.objects.dissolve(self.walker);
            info!(
                "[entity] dissolve entity={} at={:?}",
                entity.0,
                self.objects.cube.position.map(|v| v.round())
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn step(
    mut commands: Commands,
    mut entities: ResMut<Entities>,
    mut level: ResMut<Level>,
    mut player: ResMut<Player>,
    mut objects: ResMut<Objects>,
    mut gun: ResMut<Firing>,
    mut portals: ResMut<Portals>,
    mut visual: Option<ResMut<crate::gun_visual::GunVisualState>>,
    sounds: Option<ResMut<Sounds>>,
    files: Option<Res<Files>>,
    mut audio: ResMut<Assets<AudioSource>>,
) {
    let entities = &mut *entities;
    if let Some(cube) = entities.cube.filter(|_| objects.present) {
        let held = objects.holding();
        if held != entities.held {
            if held {
                entities.world.player_use(cube, entities.player);
                entities.world.picked_up(cube, entities.player);
            } else {
                entities.world.dropped(cube, entities.player);
            }
            entities.held = held;
        }
        let velocity = objects.cube.velocity;
        entities
            .world
            .prop_update(cube, objects.cube.position, velocity);
    }
    let mut game = Game {
        level: &mut level.0,
        walker: &mut player.0,
        objects: &mut objects,
        gun: &mut gun,
        portals: &mut portals,
        visual: visual.as_deref_mut(),
        player: entities.player,
        cube: entities.cube,
    };
    entities.world.step(&mut game);
    for f in entities.world.drain_fired() {
        info!(
            "[entity] t={:.2} input caller={} target={} input={} param={} handled={}",
            f.time, f.caller, f.target, f.input, f.param, f.handled
        );
    }
    for h in entities.world.drain_spawned() {
        let e = entities.world.entity(h);
        if e.classname == "prop_physics" && e.get("model") == Some("models/props/metal_box.mdl") {
            let (origin, angles) = entities.world.pose(h);
            objects.place(origin, angles);
            entities.cube = Some(h);
            entities.held = false;
            info!(
                "[entity] cube spawned entity={} at={:?}",
                h.0,
                origin.map(|v| v.round())
            );
        }
    }
    if entities.cube.is_some() && !objects.present {
        entities.cube = None;
    }
    let (Some(mut sounds), Some(files)) = (sounds, files) else {
        entities.world.drain_sounds();
        return;
    };
    for s in entities.world.drain_sounds() {
        let key = (s.source, s.name.clone());
        if let Some(playing) = entities.playing.remove(&key) {
            commands.entity(playing).try_despawn();
        }
        if !s.stop {
            if let Some(id) = sounds.play(&mut commands, &files, &mut audio, &s.name) {
                entities.playing.insert(key, id);
            }
        }
        info!(
            "[entity] sound source={} name={} stop={}",
            s.source.0, s.name, s.stop
        );
    }
}

/// the cube starts as the map's box, an entity chamber without one starts with none
fn place_cube(entities: Res<Entities>, level: Res<Level>, mut objects: ResMut<Objects>) {
    match entities.cube {
        Some(h) => {
            let (origin, angles) = entities.world.pose(h);
            objects.place(origin, angles);
        }
        None if level.0.entities.len() > 1 => objects.present = false,
        None => {}
    }
}

pub struct EntitiesPlugin;

impl Plugin for EntitiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, place_cube.run_if(resource_exists::<Entities>))
            .add_systems(
                FixedUpdate,
                step.after(crate::objects::ObjectsUpdated)
                    .run_if(crate::menu::playing)
                    .run_if(resource_exists::<Entities>),
            );
    }
}
