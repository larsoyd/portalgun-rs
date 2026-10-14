// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::prelude::*;
use portal::carry::{
    add, carry_pitch, dot, hold_distance, pickup_eligible, sub, throw_impulse, Carry, RigidCube,
};
use portal::pair::{through_dir, through_point, Portal, Vec3 as SourceVec};
use portal::teleport::{entered, in_front, nearer, touching};
use portal::viewbob::angle_vectors;

use portal::ivp::body::{axes, from_axes, step, Body, Solid};
use portal::ivp::hull::Convex;
use portal::physics_sound::{
    collision_volume, impact_sound, scrape_sound, slip_energy, ImpactQueue, ScrapeStep,
};
use portal::surfaces::Surface;

use crate::faces::Portals;
use crate::player::{Controls, MovementResult, Player};
use crate::scene::Level;
use crate::simulator::Space;
use crate::walker::Walker;

#[derive(Resource)]
pub struct Objects {
    pub cube: RigidCube,
    pub visual_revision: u64,
    pub carry: Option<Carry>,
    pair: Option<[Portal; 2]>,
    safe_position: SourceVec,
    safe_rotation: portal::carry::Basis,
    pub blocked_until: f32,
    pub body: Body,
    written: Option<RigidCube>,
    rules: (Option<usize>, bool),
    pub surfaces: Vec<Surface>,
    pub cube_surface: Surface,
    pub default_surface: Surface,
    impacts: ImpactQueue,
    last_collision: std::collections::HashMap<usize, f32>,
    /// sounds the last ticks asked for, drained by the sound system
    pub cues: Vec<Cue>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cue {
    Impact {
        name: String,
        volume: f32,
        origin: SourceVec,
    },
    /// a friction report, with the scrape it would play when it gets past the half second gate
    Friction {
        now: f32,
        sound: Option<(String, f32)>,
    },
}

const METRES: f32 = 0.0254;
/// metal_box.phy rotation_inertia for unit mass, in metres squared
const UNIT_INERTIA: [f64; 3] = [0.12165254, 0.12122901, 0.12122901];
/// metal_box.phy upper_limit_radius in metres
const RADIUS: f32 = 0.87988;
const INCHES: f32 = 39.37008;

/// The world's brushes are one static object, the player another, each portal piece group its own.
fn physics_object(id: usize, block: usize) -> usize {
    match id >> 20 {
        0 if block == usize::MAX - 1 => 1,
        0 => 0,
        group => group,
    }
}

fn cube_body(cube: &RigidCube) -> Body {
    let mut body = Body::new(
        Convex::cube(cube.half_size as f64 * METRES as f64),
        cube.mass as f64,
        UNIT_INERTIA,
    );
    body.damp = 0.1;
    body.radius = RADIUS;
    body.friction = 0.8;
    body.elasticity = 0.2;
    body
}

impl Default for Objects {
    fn default() -> Self {
        let position = [-220.0, -80.0, 20.1];
        Self {
            cube: RigidCube::at(position),
            visual_revision: 0,
            carry: None,
            pair: None,
            safe_position: position,
            safe_rotation: portal::carry::IDENTITY,
            blocked_until: 0.0,
            body: cube_body(&RigidCube::at(position)),
            written: None,
            rules: (None, false),
            surfaces: Vec::new(),
            cube_surface: Surface::default(),
            default_surface: Surface {
                friction: 0.8,
                elasticity: 0.25,
                ..Surface::default()
            },
            impacts: ImpactQueue::default(),
            last_collision: Default::default(),
            cues: Vec::new(),
        }
    }
}

impl Objects {
    pub fn holding(&self) -> bool {
        self.carry.is_some()
    }

    pub fn push(&mut self, player: &Walker, forward: f32, side: f32) {
        if self.holding() || player.ground == Some(usize::MAX - 1) {
            return;
        }
        let mut direction = add(
            player.forward().map(|v| v * forward),
            player.right().map(|v| v * side),
        );
        direction[2] = 0.0;
        let length = dot(direction, direction).sqrt();
        if length == 0.0 {
            return;
        }
        direction = direction.map(|v| v / length);
        let mut expanded = self.cube.clone();
        expanded.half_size += 16.0;
        if ray_cube(player.centre(), direction, &expanded, 3.0).is_some() {
            let speed = dot(self.cube.velocity, direction);
            if speed < 150.0 {
                self.cube.velocity = add(
                    self.cube.velocity,
                    direction.map(|v| v * (150.0 - speed) * 80.0 / (80.0 + self.cube.mass)),
                );
            }
        }
    }

    pub fn release(&mut self, now: f32) {
        self.cube.limit_release_velocity(190.0);
        self.carry = None;
        self.blocked_until = now + 0.5;
        info!(
            "[object] release position={:?} velocity={:?}",
            self.cube.position, self.cube.velocity
        );
    }

    /// The throw torque is in degrees and turns the cube through its inertia in metres.
    /// Translated from 0x209460 (decompiled, server.so) and 0x3f890 (decompiled, vphysics.so)
    pub fn throw(&mut self, forward: SourceVec, random_axes: SourceVec) {
        let cube = &mut self.cube;
        let impulse = throw_impulse(cube.mass) / cube.mass;
        cube.velocity = add(cube.velocity, forward.map(|v| v * impulse));
        let torque = portal::carry::throw_torque(cube.mass, random_axes);
        let mut spin = cube.angular_velocity;
        for (i, axis) in cube.rotation.into_iter().enumerate() {
            let turn = dot(axis, torque) / (UNIT_INERTIA[i] as f32 * cube.mass);
            spin = add(spin, axis.map(|v| v * turn));
        }
        cube.angular_velocity = spin;
    }

    pub fn use_object(&mut self, player: &Walker, space: &Space, now: f32) -> bool {
        if self.holding() {
            self.release(now);
            return true;
        }
        if player.ground == Some(usize::MAX - 1)
            || now < self.blocked_until
            || !pickup_eligible(self.cube.mass, [self.cube.half_size * 2.0; 3], true)
        {
            return false;
        }
        let Some(portal) = pickup_ray(player, &self.cube, space) else {
            return false;
        };
        let rotation = match (portal, space.pair) {
            (Some(i), Some(pair)) => self
                .cube
                .rotation
                .map(|v| through_dir(&pair[1 - i], &pair[i], v)),
            _ => self.cube.rotation,
        };
        self.carry = Some(Carry::with_angles(rotation, player.angles(), portal));
        self.pair = space.pair;
        info!(
            "[object] pickup position={:?} portal={portal:?}",
            self.cube.position
        );
        true
    }

    pub fn advance(
        &mut self,
        dt: f32,
        player: &Walker,
        chamber: &crate::chamber::Chamber,
        pair: Option<[Portal; 2]>,
        crossed: Option<usize>,
        now: f32,
    ) {
        let mut space = Space::new(chamber, pair.as_ref());
        if self.pair != pair {
            if self.carry.as_ref().is_some_and(|c| c.portal.is_some()) {
                self.release(now);
            }
            self.cube.owner = None;
            let tr = space.trace_oriented(
                None,
                self.cube.position,
                self.cube.position,
                self.cube.rotation,
                self.cube.half_size,
            );
            if tr.start_solid {
                self.cube.position = self.safe_position;
                self.cube.rotation = self.safe_rotation;
                self.visual_revision += 1;
                self.cube.velocity = [0.0; 3];
                self.cube.angular_velocity = [0.0; 3];
                if self.holding() {
                    self.release(now);
                }
                info!(
                    "[object] recover reason=portal_changed position={:?}",
                    self.cube.position
                );
            }
            self.pair = pair;
        }
        if let (Some(carry), Some(i)) = (&mut self.carry, crossed) {
            carry.player_crossed(i);
            let basis = match (carry.portal, pair) {
                (Some(p), Some(pair)) => self
                    .cube
                    .rotation
                    .map(|v| through_dir(&pair[1 - p], &pair[p], v)),
                _ => self.cube.rotation,
            };
            carry.relative_rotation = portal::carry::relative_rotation(basis, player.angles());
        }
        if !self.holding() {
            space.add_solid(crate::collision::box_planes(
                add(player.feet, [-16.0, -16.0, 0.0]),
                add(player.feet, [16.0, 16.0, player.height()]),
            ));
        }
        self.own(&space);
        let prior_rotation = self.cube.rotation;
        let target = self
            .carry
            .as_ref()
            .and_then(|carry| hold_target(carry, &self.cube, player, &space));
        if let Some((position, rotation)) = target {
            let delta = sub(position, self.cube.position);
            let distance = dot(delta, delta).sqrt();
            let desired = delta.map(|v| v / 0.05);
            let speed = dot(desired, desired).sqrt();
            self.cube.velocity = desired.map(|v| v * (1000.0 / speed.max(1000.0)));
            self.cube.angular_velocity =
                portal::carry::rotation_velocity(prior_rotation, rotation, dt);
            let looks = self.carry.as_ref().and_then(|c| c.portal).is_none_or(|i| {
                pair.is_some_and(|p| {
                    portal_ray(&p[i], player.eye(), player.forward(), 1024.0).is_some()
                })
            });
            if self
                .carry
                .as_mut()
                .is_some_and(|c| c.update_error(distance, dt, looks))
            {
                self.release(now);
            }
        } else if self.holding() {
            self.release(now);
        }
        self.simulate(&space, pair, dt, now);
        if let (Some(i), Some(pair)) = (self.cube.owner, pair) {
            if entered(&pair[i], self.cube.position, self.cube.half()) {
                self.cube.teleport(&pair[i], &pair[1 - i], 1 - i);
                self.visual_revision += 1;
                if let Some(carry) = &mut self.carry {
                    carry.object_crossed(i);
                }
                info!(
                    "[object] cross from={i} position={:?} velocity={:?}",
                    self.cube.position, self.cube.velocity
                );
            }
        }
        let clear = space.trace_oriented(
            None,
            self.cube.position,
            self.cube.position,
            self.cube.rotation,
            self.cube.half_size,
        );
        if !clear.start_solid {
            self.safe_position = self.cube.position;
            self.safe_rotation = self.cube.rotation;
        }
    }

    /// One vphysics step of the cube against what it collides with this tick.
    fn simulate(&mut self, space: &Space, pair: Option<[Portal; 2]>, dt: f32, now: f32) {
        self.load_body();
        let in_hole = match (self.cube.owner, pair) {
            (Some(i), Some(p)) => {
                portal::teleport::in_hole(&p[i], self.cube.position, self.cube.half())
            }
            _ => false,
        };
        if self.rules != (self.cube.owner, in_hole) {
            self.rules = (self.cube.owner, in_hole);
            self.body.detach();
        }
        let pieces = space.object_solids(self.cube.owner, in_hole);
        let solids: Vec<Solid> = pieces
            .iter()
            .map(|&(id, block, planes)| {
                let surface = self.surface(block);
                let planes: Vec<_> = planes
                    .iter()
                    .map(|p| (p.normal.map(f64::from), (p.distance * METRES) as f64))
                    .collect();
                Solid {
                    id,
                    object: physics_object(id, block),
                    hull: Convex::from_planes(&planes),
                    friction: surface.friction,
                    elasticity: surface.elasticity,
                }
            })
            .filter(|s| s.hull.points.len() >= 4)
            .collect();
        let gravity = if self.holding() {
            0.0
        } else {
            -600.0 * METRES as f64
        };
        let was_asleep = self.body.asleep;
        step(
            &mut self.body,
            &solids,
            [0.0, 0.0, gravity],
            dt as f64,
            now as f64,
        );
        self.store_body();
        let blocks: std::collections::HashMap<usize, usize> =
            pieces.iter().map(|&(id, block, _)| (id, block)).collect();
        self.cue_sounds(&blocks, dt, now);
        if self.body.asleep != was_asleep {
            info!(
                "[object] asleep={} position={:?} contacts={} solids={}",
                self.body.asleep,
                self.cube.position,
                self.body.contacts.len(),
                solids.len()
            );
        }
    }

    fn surface(&self, block: usize) -> &Surface {
        self.surfaces.get(block).unwrap_or(&self.default_surface)
    }

    /// Translated from 0x48e740, 0x870170, 0x8718e0 (server.so) and raw 0x20d70 (vphysics.so)
    fn cue_sounds(&mut self, blocks: &std::collections::HashMap<usize, usize>, dt: f32, now: f32) {
        let block = |id: usize| blocks.get(&id).copied().unwrap_or(usize::MAX);
        for (id, speed) in std::mem::take(&mut self.body.collisions) {
            let hit = block(id);
            let object = physics_object(id, hit);
            let since = self
                .last_collision
                .insert(object, now)
                .map_or(1.0, |last| now - last);
            let silent = self.cube_surface.material == b'X' || self.surface(hit).material == b'X';
            let speed = speed as f32 * INCHES;
            if let Some(volume) = collision_volume(speed, since).filter(|_| !silent) {
                self.impacts.add(0, hit, volume, speed, self.cube.position);
            }
        }
        for e in self.impacts.flush(dt) {
            if let Some(name) = impact_sound(&self.cube_surface, self.surface(e.hit), e.speed) {
                self.cues.push(Cue::Impact {
                    name: name.to_owned(),
                    volume: e.volume,
                    origin: e.origin,
                });
            }
        }
        let inv_mass = 1.0 / self.body.mass;
        let mut reports = Vec::new();
        for c in &mut self.body.contacts {
            let work = std::mem::take(&mut c.slip_work);
            if let Some(energy) = (work != 0.0)
                .then(|| slip_energy(work, inv_mass, dt as f64))
                .flatten()
            {
                reports.push((block(c.solid), energy));
            }
        }
        for (hit, energy) in reports {
            let sound = scrape_sound(&self.cube_surface, self.surface(hit), energy)
                .map(|(name, volume)| (name.to_owned(), volume));
            self.cues.push(Cue::Friction { now, sound });
        }
    }

    /// Copies any change made to the cube from outside the simulation into the body.
    fn load_body(&mut self) {
        let cube = &self.cube;
        let same_pose = self
            .written
            .as_ref()
            .is_some_and(|w| w.position == cube.position && w.rotation == cube.rotation);
        if !same_pose {
            self.body.position = cube.position.map(|v| (v * METRES) as f64);
            self.body.rotation = from_axes(cube.rotation.map(|a| a.map(f64::from)));
            self.body.previous_rotation = self.body.rotation;
            self.body.detach();
        }
        let same_motion = self.written.as_ref().is_some_and(|w| {
            same_pose && w.velocity == cube.velocity && w.angular_velocity == cube.angular_velocity
        });
        if !same_motion {
            self.body.speed = cube.velocity.map(|v| (v * METRES) as f64);
            self.body
                .set_world_spin(cube.angular_velocity.map(|v| v.to_radians() as f64));
            self.body.wake();
        }
    }

    fn store_body(&mut self) {
        let b = &self.body;
        self.cube.position = b.position.map(|v| v as f32 / METRES);
        self.cube.rotation = axes(b.rotation).map(|a| a.map(|v| v as f32));
        self.cube.velocity = b.speed.map(|v| v as f32 / METRES);
        self.cube.angular_velocity = b.world_spin().map(|v| (v as f32).to_degrees());
        self.written = Some(self.cube.clone());
    }

    fn own(&mut self, space: &Space) {
        let Some(pair) = space.pair else {
            self.cube.owner = None;
            return;
        };
        let center = self.cube.position;
        let half = self.cube.half();
        if self
            .cube
            .owner
            .is_some_and(|i| !touching(&pair[i], center, half))
        {
            self.cube.owner = None;
        }
        for i in 0..2 {
            if touching(&pair[i], center, half)
                && in_front(&pair[i], center)
                && self
                    .cube
                    .owner
                    .is_none_or(|old| nearer(center, &pair[i], &pair[old]))
            {
                self.cube.owner = Some(i);
            }
        }
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectsUpdated;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectInteraction;

pub struct ObjectsPlugin;
impl Plugin for ObjectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Objects>()
            .add_systems(
                FixedUpdate,
                push.before(crate::player::simulate)
                    .run_if(crate::menu::playing),
            )
            .add_systems(
                Update,
                interact
                    .in_set(ObjectInteraction)
                    .before(crate::firing::fire)
                    .run_if(crate::menu::playing),
            )
            .add_systems(
                FixedUpdate,
                simulate
                    .in_set(ObjectsUpdated)
                    .after(crate::player::simulate)
                    .run_if(crate::menu::playing),
            )
            .add_systems(FixedUpdate, sound.after(ObjectsUpdated));
    }
}

#[derive(Default)]
struct ScrapeLoop {
    state: portal::physics_sound::Scrape,
    sound: Option<Entity>,
    volume: Ramp,
    pitch: Ramp,
}

/// A value moving linearly to its target over a tenth of a second.
#[derive(Default, Clone, Copy)]
struct Ramp {
    from: f32,
    to: f32,
    t: f32,
}

impl Ramp {
    fn now(&self) -> f32 {
        self.from + (self.to - self.from) * (self.t / 0.1).min(1.0)
    }

    fn set(&mut self, to: f32) {
        *self = Ramp {
            from: self.now(),
            to,
            t: 0.0,
        };
    }
}

/// Plays queued impacts and keeps the one scrape loop the cube may have, ramping it over a tenth.
/// Translated from 0x870590 and 0x86e370 (decompiled, server.so)
#[allow(clippy::too_many_arguments)]
fn sound(
    mut commands: Commands,
    mut objects: ResMut<Objects>,
    sounds: Option<ResMut<crate::sound::Sounds>>,
    files: Option<Res<crate::sound::Files>>,
    mut audio: ResMut<Assets<AudioSource>>,
    mut sinks: Query<&mut AudioSink>,
    time: Res<Time<Fixed>>,
    mut scrape: Local<ScrapeLoop>,
) {
    let (Some(mut sounds), Some(files)) = (sounds, files) else {
        objects.cues.clear();
        return;
    };
    for cue in std::mem::take(&mut objects.cues) {
        match cue {
            Cue::Impact { name, volume, .. } => {
                sounds.play_scaled(&mut commands, &files, &mut audio, &name, volume);
            }
            Cue::Friction { now, sound } => {
                let entry = sound.as_ref().and_then(|(name, _)| sounds.entry(name));
                let script = entry.map_or(1.0, |e| e.volume.0);
                let pitch = match entry.map(|e| e.pitch) {
                    Some(portal::soundscript::Pitch::Range(lo, hi)) => (lo, hi),
                    Some(portal::soundscript::Pitch::Fixed(p)) => (p, p),
                    None => (100.0, 100.0),
                };
                let step = scrape
                    .state
                    .friction(now, sound.as_ref().map(|s| s.1), script, pitch);
                match (step, sound) {
                    (ScrapeStep::Start { volume }, Some((name, _))) => {
                        scrape.sound =
                            sounds.looped(&mut commands, &files, &mut audio, &name, volume);
                        scrape.volume = Ramp {
                            from: volume,
                            to: volume,
                            t: 0.0,
                        };
                        scrape.pitch = Ramp::default();
                        info!("[object] scrape start name={name} volume={volume:.3}");
                    }
                    (ScrapeStep::Retarget { volume, pitch }, _) => {
                        scrape.volume.set(volume);
                        if scrape.pitch.to == 0.0 {
                            scrape.pitch = Ramp {
                                from: pitch,
                                to: pitch,
                                t: 0.0,
                            };
                        }
                        scrape.pitch.set(pitch);
                    }
                    _ => {}
                }
            }
        }
    }
    let Some(entity) = scrape.sound else {
        return;
    };
    if scrape.state.expired(time.elapsed_secs()) {
        commands.entity(entity).despawn();
        scrape.sound = None;
        info!("[object] scrape stop");
        return;
    }
    scrape.volume.t += time.delta_secs();
    scrape.pitch.t += time.delta_secs();
    if let Ok(mut sink) = sinks.get_mut(entity) {
        sink.set_volume(bevy::audio::Volume::Linear(scrape.volume.now()));
        if scrape.pitch.to > 0.0 {
            sink.set_speed(scrape.pitch.now() / 100.0);
        }
    }
}

fn push(mut objects: ResMut<Objects>, player: Res<Player>, controls: Res<Controls>) {
    objects.push(&player, controls.forward, controls.side);
}

fn interact(
    mut objects: ResMut<Objects>,
    mut controls: ResMut<Controls>,
    player: Res<Player>,
    level: Res<Level>,
    portals: Res<Portals>,
    time: Res<Time>,
    mut random: Local<crate::random::Xorshift>,
) {
    let now = time.elapsed_secs();
    if objects.holding() && (controls.fire1 || controls.fire2) {
        let throw = controls.fire1;
        controls.fire1 = false;
        controls.fire2 = false;
        let mut forward = player.forward();
        if let (Some(i), Some(pair)) = (
            objects.carry.as_ref().and_then(|c| c.portal),
            portals.linked(),
        ) {
            forward = through_dir(&pair[i], &pair[1 - i], forward);
        }
        objects.release(now);
        if throw {
            let axes = [0, 1, 2].map(|_| random.range(-10.0, 10.0));
            objects.throw(forward, axes);
        }
    }
    if std::mem::take(&mut controls.use_object) {
        let space = Space::new(&level, portals.linked().as_ref());
        objects.use_object(&player, &space, now);
    }
}

fn simulate(
    mut objects: ResMut<Objects>,
    mut player: ResMut<Player>,
    level: Res<Level>,
    portals: Res<Portals>,
    result: Res<MovementResult>,
    time: Res<Time<Fixed>>,
) {
    objects.advance(
        time.delta_secs(),
        &player,
        &level,
        portals.linked(),
        result.0,
        time.elapsed_secs(),
    );
    if objects.holding() {
        player.pitch_reorientation = false;
    }
}

fn hold_target(
    carry: &Carry,
    cube: &RigidCube,
    player: &Walker,
    space: &Space,
) -> Option<(SourceVec, portal::carry::Basis)> {
    let mut angles = player.angles();
    angles.pitch = carry_pitch(angles.pitch);
    let forward = angle_vectors(angles).0;
    let support_direction = match (carry.portal, space.pair) {
        (Some(i), Some(pair)) => through_dir(&pair[i], &pair[1 - i], forward),
        _ => forward,
    };
    let radius = cube.support(support_direction) + (16.0_f32 * 16.0 * 2.0).sqrt();
    let maximum = 24.0 + 2.0 * radius;
    let end = add(player.eye(), forward.map(|v| v * maximum));
    let owner = carry.portal.or(player.owner);
    let tr = space.trace(owner, player.eye(), end, [0.0; 3], [0.0; 3]);
    carry.target_angles(
        player.eye(),
        forward,
        hold_distance(radius, tr.fraction),
        player.angles(),
        space.pair.as_ref(),
    )
}

// Translated from 0x2e52a0 and 0x2e64b0 (decompiled, server.so)
fn pickup_ray(player: &Walker, cube: &RigidCube, space: &Space) -> Option<Option<usize>> {
    if select_frame(
        player.eye(),
        player.forward(),
        player.up(),
        cube,
        space,
        None,
        [player.centre()[2], player.centre()[2] + player.height()],
    ) {
        return Some(None);
    }
    let pair = space.pair.as_ref()?;
    let (i, distance) = (0..2)
        .filter_map(|i| {
            portal_ray(&pair[i], player.eye(), player.forward(), 1024.0).map(|d| (i, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))?;
    let entrance_point = add(player.eye(), player.forward().map(|v| v * distance));
    let entrance = space.trace(None, player.eye(), entrance_point, [0.0; 3], [0.0; 3]);
    let entrance_length = distance;
    if (1.0 - entrance.fraction) * entrance_length > 0.04 {
        return None;
    }
    let eye = through_point(&pair[i], &pair[1 - i], player.eye());
    let forward = through_dir(&pair[i], &pair[1 - i], player.forward());
    let up = through_dir(&pair[i], &pair[1 - i], player.up());
    select_frame(
        eye,
        forward,
        up,
        cube,
        space,
        Some(1 - i),
        [player.centre()[2], player.centre()[2] + player.height()],
    )
    .then_some(Some(i))
}

fn select_frame(
    eye: SourceVec,
    forward: SourceVec,
    up: SourceVec,
    cube: &RigidCube,
    space: &Space,
    owner: Option<usize>,
    vertical: [f32; 2],
) -> bool {
    let cast = |dir: SourceVec, length: f32, hull: f32| {
        let end = add(eye, dir.map(|v| v * length));
        let wall = space.trace(owner, eye, end, [-hull; 3], [hull; 3]);
        let planes: Vec<_> = cube
            .rotation
            .into_iter()
            .flat_map(|axis| {
                [-1.0, 1.0].map(|sign| {
                    let normal = axis.map(|v| v * sign);
                    crate::collision::Plane {
                        normal,
                        distance: dot(normal, cube.position) + cube.half_size,
                    }
                })
            })
            .collect();
        let hit = crate::collision::trace_brushes(eye, end, [-hull; 3], [hull; 3], &[(0, planes)]);
        let z = hit.end[2] - hit.end[2].clamp(vertical[0], vertical[1]);
        let delta = sub(hit.end, eye);
        let distance = (delta[0] * delta[0] + delta[1] * delta[1] + z * z).sqrt();
        hit.block.is_some() && distance < 80.0 && hit.fraction <= wall.fraction + 0.0001
    };
    if cast(forward, 1024.0, 0.0) {
        return true;
    }
    for tangent in [
        1.0,
        0.57735026,
        0.36397022,
        0.2679492,
        0.17632698,
        -0.17632698,
        -0.2679492,
    ] {
        let dir = sub(forward, up.map(|v| v * tangent));
        let length = dot(dir, dir).sqrt();
        if cast(dir.map(|v| v / length), 72.0, 16.0) {
            return true;
        }
    }
    let relative = sub(eye, cube.position);
    let nearest = cube.rotation.iter().fold(cube.position, |point, axis| {
        add(
            point,
            axis.map(|v| v * dot(relative, *axis).clamp(-cube.half_size, cube.half_size)),
        )
    });
    let delta = sub(nearest, eye);
    let length = dot(delta, delta).sqrt();
    if length > 80.0 || length == 0.0 || ((dot(delta, forward) / length) as f64) < 0.8 {
        return false;
    }
    let trace = space.trace(owner, eye, nearest, [0.0; 3], [0.0; 3]);
    trace.fraction >= 0.9999
}

fn portal_ray(portal: &Portal, eye: SourceVec, dir: SourceVec, max: f32) -> Option<f32> {
    let denominator = dot(dir, portal.forward);
    if denominator >= 0.0 {
        return None;
    }
    let distance = -portal.local(eye)[0] / denominator;
    (distance >= 0.0
        && distance <= max
        && portal.inside(portal.local(add(eye, dir.map(|v| v * distance)))))
    .then_some(distance)
}

pub fn ray_cube(eye: SourceVec, dir: SourceVec, cube: &RigidCube, max: f32) -> Option<f32> {
    let relative = sub(eye, cube.position);
    let mut near = 0.0_f32;
    let mut far = max;
    for axis in cube.rotation {
        let origin = dot(relative, axis);
        let direction = dot(dir, axis);
        if direction.abs() < 1e-6 {
            if origin.abs() > cube.half_size {
                return None;
            }
        } else {
            let a = (-cube.half_size - origin) / direction;
            let b = (cube.half_size - origin) / direction;
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return None;
            }
        }
    }
    Some(near)
}
