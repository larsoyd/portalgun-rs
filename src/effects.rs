// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::collections::{BTreeMap, HashMap};

use bevy::asset::{embedded_asset, RenderAssetUsages};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use portal::install::Install;
use portal::mdl::Mdl;
use portal::pair::Portal;
use portal::particles::collection::Particle;
use portal::particles::forces::{Trace, World};
use portal::particles::initializers::Query as ParticleQuery;
use portal::particles::random::{RandomTable, SimdRandom};
use portal::particles::renderers::{Camera as ParticleCamera, Look};
use portal::particles::system::{Definition, System};

use crate::faces::Portals;
use crate::particle_material::{FrameCopy, ParticleMaterial, ParticleMaterials, FRAME_BLEND};
use crate::portals::{Colour, Pair};
use crate::scene::Level;
use crate::units::{from_bevy, from_bevy_dir, to_bevy};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Particles,
    Edge,
    Success,
    Near,
    Overlap,
    NoFit,
    BadVolume,
    BadSurface,
    Close,
    Stream,
}
impl Effect {
    pub fn name(self, colour: Colour) -> String {
        let suffix = match self {
            Self::Particles => "particles",
            Self::Edge => "edge",
            Self::Success => "success",
            Self::Near => "near",
            Self::Overlap => "overlap",
            Self::NoFit => "nofit",
            Self::BadVolume => "badvolume",
            Self::BadSurface => "badsurface",
            Self::Close => "close",
            Self::Stream => "projectile_stream",
        };
        format!("portal_{}_{}", colour.index() + 1, suffix)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
}
impl Frame {
    fn from_matrix(m: Mat4) -> Self {
        Self {
            origin: m.w_axis.truncate(),
            forward: m.x_axis.truncate(),
            right: -m.y_axis.truncate(),
            up: m.z_axis.truncate(),
        }
    }
    /// SetAbsOrigin never builds the blast's rotation, so its point gets a zero basis
    /// Translated from 0x560b80, 0x361d40, 0x361920 and 0x260b40 (decompiled, client.so)
    pub fn stream(at: [f32; 3]) -> Self {
        Self {
            origin: Vec3::from(at),
            forward: Vec3::ZERO,
            right: Vec3::ZERO,
            up: Vec3::ZERO,
        }
    }

    pub fn impact(p: &Portal, offset: f32) -> Self {
        Self {
            origin: Vec3::from(p.origin) + Vec3::from(p.forward) * offset,
            forward: Vec3::from(p.right),
            right: Vec3::from(p.up),
            up: Vec3::from(p.forward),
        }
    }
    fn apply(self, system: &mut System) {
        system.set_control_point(0, self.origin.to_array());
        system.set_control_point_orientation(
            0,
            self.forward.to_array(),
            self.right.to_array(),
            self.up.to_array(),
        );
    }
}

struct Instance {
    system: System,
    owner: Option<(Colour, Effect)>,
}

#[derive(Resource)]
pub struct ParticleEffects {
    pub(crate) defs: Vec<Definition>,
    names: HashMap<String, usize>,
    table: RandomTable,
    attachments: [[Mat4; 2]; 2],
    simd: SimdRandom,
    live: BTreeMap<u64, Instance>,
    next: u64,
    seen: [u64; 2],
    open: [Option<Portal>; 2],
}

impl ParticleEffects {
    pub fn read(install: &Install) -> Result<Self, String> {
        let mut defs = Vec::new();
        for file in [
            "particles/portals.pcf",
            "particles/portalgun.pcf",
            "particles/portal_projectile.pcf",
        ] {
            let bytes = install.search.read(file).map_err(|e| e.to_string())?;
            let elements = portal::dmx::parse(&bytes).map_err(|e| e.to_string())?;
            let base = defs.len();
            for mut def in portal::pcf::systems(&elements).map_err(|e| e.to_string())? {
                for child in &mut def.children {
                    child.system += base;
                }
                defs.push(Definition::new(&def)?);
            }
        }
        let mut attachments = [[Mat4::IDENTITY; 2]; 2];
        for c in Colour::ALL {
            let path = format!("models/portals/portal{}.mdl", c.index() + 1);
            let mdl = Mdl::parse(install.search.read(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            for (i, name) in ["particles_2", "particlespin"].into_iter().enumerate() {
                attachments[c.index()][i] = attachment(&mdl, name)?;
            }
        }
        let names = defs
            .iter()
            .enumerate()
            .map(|(i, d)| (d.name.to_ascii_lowercase(), i))
            .collect();
        Ok(Self {
            defs,
            names,
            table: RandomTable::from_client(&install.client).map_err(|e| e.to_string())?,
            attachments,
            simd: SimdRandom::default(),
            live: BTreeMap::new(),
            next: 0,
            seen: [0; 2],
            open: [None; 2],
        })
    }
    pub fn attachment_frame(&self, c: Colour, effect: Effect, p: &Portal) -> Frame {
        let entity = Mat4::from_cols(
            Vec3::from(p.forward).extend(0.0),
            (-Vec3::from(p.right)).extend(0.0),
            Vec3::from(p.up).extend(0.0),
            Vec3::from(p.origin).extend(1.0),
        );
        Frame::from_matrix(
            entity * self.attachments[c.index()][usize::from(effect == Effect::Edge)],
        )
    }
    pub fn spawn(
        &mut self,
        colour: Colour,
        effect: Effect,
        frame: Frame,
        cp1: Option<[f32; 3]>,
    ) -> u64 {
        let name = effect.name(colour);
        let index = self.names[&name];
        self.next += 1;
        let seed = (self.next as u32).wrapping_mul(0x9e37_79b9) | 1;
        let mut system = System::new(&self.defs, index, 0.0, seed, &self.table);
        frame.apply(&mut system);
        if let Some(at) = cp1 {
            system.set_control_point(1, at);
        }
        self.live.insert(
            self.next,
            Instance {
                system,
                owner: None,
            },
        );
        self.next
    }
    /// Control point 0 follows the effect's entity, as the shot stream does every frame.
    pub fn move_point(&mut self, id: u64, at: [f32; 3]) {
        if let Some(live) = self.live.get_mut(&id) {
            live.system.set_control_point(0, at);
        }
    }

    /// Stops emitting and lets the particles already out finish their lives.
    pub fn stop(&mut self, id: u64) {
        if let Some(live) = self.live.get_mut(&id) {
            live.system.stop_emission(false);
        }
    }

    pub fn sync(&mut self, pair: &Pair) {
        for c in Colour::ALL {
            let i = c.index();
            let changed = self.seen[i] != pair.revisions[i] || self.open[i] != pair.open[i];
            if changed {
                self.live
                    .retain(|_, l| !l.owner.is_some_and(|(owner, _)| owner == c));
                if let Some(p) = pair.open[i] {
                    for effect in [Effect::Particles, Effect::Edge] {
                        let id = self.spawn(c, effect, self.attachment_frame(c, effect, &p), None);
                        self.live.get_mut(&id).unwrap().owner = Some((c, effect));
                    }
                }
                self.seen[i] = pair.revisions[i];
                self.open[i] = pair.open[i];
            }
        }
    }
    pub fn simulate(&mut self, dt: f32, world: &ParticleWorld) {
        if dt <= 0.0 {
            return;
        }
        for live in self.live.values_mut() {
            live.system.simulate(dt, &mut self.simd, world, world);
        }
        self.live.retain(|_, l| !l.system.is_finished());
    }
    pub fn persistent_count(&self) -> usize {
        self.live.values().filter(|l| l.owner.is_some()).count()
    }
    pub fn instance_count(&self) -> usize {
        self.live.len()
    }
    pub fn snapshot(&self) -> Vec<Particle> {
        let mut particles = Vec::new();
        for live in self.live.values() {
            let mut nodes = Vec::new();
            walk(&live.system, &mut nodes);
            for node in nodes {
                particles.extend_from_slice(&node.c.particles);
            }
        }
        particles
    }
}

fn attachment(mdl: &Mdl, name: &str) -> Result<Mat4, String> {
    let a = mdl
        .attachments
        .iter()
        .find(|a| a.name == name)
        .ok_or_else(|| format!("{}: no {name}", mdl.name))?;
    let pose = portal::anim::sample(mdl, mdl.sequences[0].anim, 0.0).map_err(|e| e.to_string())?;
    let mut bones = Vec::new();
    for (i, b) in pose.iter().enumerate() {
        let local = Mat4::from_rotation_translation(Quat::from_array(b.quat), Vec3::from(b.pos));
        bones.push(
            mdl.bones[i]
                .parent
                .map_or(local, |parent| bones[parent] * local),
        );
    }
    let m = a.local;
    let local = Mat4::from_cols_array(&[
        m[0][0], m[1][0], m[2][0], 0.0, m[0][1], m[1][1], m[2][1], 0.0, m[0][2], m[1][2], m[2][2],
        0.0, m[0][3], m[1][3], m[2][3], 1.0,
    ]);
    Ok(bones[a.bone] * local)
}

#[derive(Default)]
pub struct ParticleWorld {
    solids: Vec<(usize, Vec<crate::collision::Plane>)>,
}
impl ParticleWorld {
    pub fn new(chamber: &crate::chamber::Chamber) -> Self {
        Self {
            solids: chamber
                .blocks
                .iter()
                .enumerate()
                .map(|(i, b)| (i, b.planes()))
                .collect(),
        }
    }
}
impl World for ParticleWorld {
    fn trace(&self, start: [f32; 3], end: [f32; 3], _: bool) -> Trace {
        let t = crate::collision::trace_brushes(start, end, [0.0; 3], [0.0; 3], &self.solids);
        Trace {
            fraction: t.fraction,
            normal: t.normal,
            start_solid: t.start_solid,
        }
    }
}
impl ParticleQuery for ParticleWorld {
    fn trace(&self, start: [f32; 3], end: [f32; 3], _: i32) -> f32 {
        World::trace(self, start, end, false).fraction
    }
}

fn walk<'a>(system: &'a System, nodes: &mut Vec<&'a System>) {
    nodes.push(system);
    for child in &system.children {
        walk(child, nodes);
    }
}

#[derive(Component, Clone, Copy)]
pub struct ParticleView(pub usize);

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParticleSimulation;

pub struct EffectsPlugin;
impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/particle.wgsl");
        embedded_asset!(app, "shaders/frame_copy.wgsl");
        app.add_plugins(MaterialPlugin::<ParticleMaterial>::default())
            .add_plugins(MaterialPlugin::<FrameCopy>::default())
            .init_resource::<Draws>()
            .add_systems(Startup, crate::particle_material::load)
            .add_systems(
                PostUpdate,
                (simulate, draw)
                    .chain()
                    .in_set(ParticleSimulation)
                    .after(crate::faces::PortalViewsUpdated)
                    .before(TransformSystems::Propagate),
            );
    }
}
fn simulate(
    mut fx: ResMut<ParticleEffects>,
    pair: Res<Portals>,
    level: Res<Level>,
    time: Res<Time>,
) {
    fx.sync(&pair);
    fx.simulate(time.delta_secs(), &ParticleWorld::new(&level.0));
}

type DrawKey = (u64, usize, usize, Entity);
type DrawMesh = (Entity, Handle<Mesh>);

#[derive(Resource, Default)]
struct Draws(HashMap<DrawKey, DrawMesh>);

fn particle_mesh(built: portal::particles::renderers::Mesh) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        built
            .vertices
            .iter()
            .map(|v| to_bevy(v.pos).to_array())
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        built.vertices.iter().map(|v| v.uv).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_1,
        built.vertices.iter().map(|v| v.uv_next).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        built.vertices.iter().map(|v| v.color).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        FRAME_BLEND,
        built.vertices.iter().map(|v| v.blend).collect::<Vec<_>>(),
    );
    mesh.insert_indices(Indices::U32(built.indices));
    mesh
}
#[allow(clippy::too_many_arguments)]
fn draw(
    mut commands: Commands,
    fx: Res<ParticleEffects>,
    materials: Res<ParticleMaterials>,
    views: Query<(Entity, &Camera, &Transform, &ParticleView)>,
    mut draws: ResMut<Draws>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut visibility: Query<&mut Visibility>,
) {
    draws.0.retain(|(id, _, _, view), (entity, mesh)| {
        let keep = fx.live.contains_key(id) && views.get(*view).is_ok();
        if !keep {
            commands.entity(*entity).despawn();
            meshes.remove(mesh.id());
        }
        keep
    });
    for ((_, _, _, view), (entity, _)) in &draws.0 {
        if let Ok(mut v) = visibility.get_mut(*entity) {
            *v = if views.get(*view).is_ok_and(|(_, c, _, _)| c.is_active) {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
    for (view_id, camera, transform, layer) in &views {
        if !camera.is_active {
            continue;
        }
        let cam = ParticleCamera {
            pos: from_bevy(transform.translation),
            right: from_bevy_dir(*transform.right()),
            up: from_bevy_dir(*transform.up()),
        };
        for (&id, live) in &fx.live {
            let mut nodes = Vec::new();
            walk(&live.system, &mut nodes);
            for (n, node) in nodes.into_iter().enumerate() {
                let Some(material) = materials.0.get(&node.def.material) else {
                    continue;
                };
                let look = Look {
                    height: material.height,
                    sheet: material.sheet.as_ref(),
                    card: material.card,
                };
                for (r, renderer) in node.def.renderers.iter().enumerate() {
                    let key = (id, n, r, view_id);
                    let built = renderer.build(&node.c, &cam, &look);
                    // Bevy skips empty mesh allocations but still attempts their upload.
                    if built.indices.is_empty() {
                        if let Some((entity, _)) = draws.0.get(&key) {
                            if let Ok(mut v) = visibility.get_mut(*entity) {
                                *v = Visibility::Hidden;
                            }
                        }
                        continue;
                    }
                    if let Some((_, handle)) = draws.0.get(&key) {
                        if let Some(mut mesh) = meshes.get_mut(handle) {
                            *mesh = particle_mesh(built);
                        }
                    } else {
                        let handle = meshes.add(particle_mesh(built));
                        let entity = commands
                            .spawn((
                                Mesh3d(handle.clone()),
                                MeshMaterial3d(material.handle.clone()),
                                Transform::IDENTITY,
                                Visibility::Visible,
                                RenderLayers::layer(layer.0),
                                NoFrustumCulling,
                            ))
                            .id();
                        if let Some(copy) = &material.frame_copy {
                            commands.spawn((
                                Mesh3d(handle.clone()),
                                MeshMaterial3d(copy.clone()),
                                Transform::IDENTITY,
                                Visibility::Inherited,
                                RenderLayers::layer(layer.0),
                                NoFrustumCulling,
                                ChildOf(entity),
                            ));
                        }
                        draws.0.insert(key, (entity, handle));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound::Files;
    use bevy::asset::Assets;

    #[test]
    fn source_colours_and_child_control_points_survive_integration() {
        let home = std::env::var("HOME").unwrap();
        let install = Install::open(&portal::install::root(
            None,
            None,
            std::path::Path::new(&home),
        ))
        .unwrap();
        let mut fx = ParticleEffects::read(&install).unwrap();
        let mut pair = Pair::default();
        for c in Colour::ALL {
            let p = Portal::new(
                [100.0 + c.index() as f32 * 200.0, 0.0, 64.0],
                [-1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
                (32.0, 54.0),
            );
            pair.place(c, p);
        }
        fx.sync(&pair);
        for _ in 0..20 {
            fx.simulate(0.05, &ParticleWorld::default());
        }
        for live in fx.live.values() {
            let (c, effect) = live.owner.unwrap();
            let frame = fx.attachment_frame(c, effect, pair.get(c).unwrap());
            let mut nodes = Vec::new();
            walk(&live.system, &mut nodes);
            for node in nodes {
                let cp = node.c.control_points[0];
                assert_eq!(cp.pos, frame.origin.to_array());
                assert_eq!(cp.forward, frame.forward.to_array());
                assert_eq!(cp.right, frame.right.to_array());
                assert_eq!(cp.up, frame.up.to_array());
            }
            if effect == Effect::Edge {
                assert!(!live.system.c.particles.is_empty());
                for p in &live.system.c.particles {
                    assert!(match c {
                        Colour::Blue => p.tint[2] > p.tint[0],
                        Colour::Orange => p.tint[0] > p.tint[2],
                    });
                }
            }
        }
        let id = fx.spawn(
            Colour::Blue,
            Effect::Overlap,
            Frame::impact(pair.get(Colour::Blue).unwrap(), 1.0),
            Some([5.0, 6.0, 7.0]),
        );
        let mut nodes = Vec::new();
        walk(&fx.live[&id].system, &mut nodes);
        assert!(nodes.len() > 1);
        for node in nodes {
            assert_eq!(node.c.control_points[1].pos, [5.0, 6.0, 7.0]);
        }
    }

    #[test]
    fn a_flying_shot_stream_leaves_its_trail_particles_still_and_its_streaks_invisible() {
        use portal::particles::renderers::Renderer;
        let home = std::env::var("HOME").unwrap();
        let install = Install::open(&portal::install::root(
            None,
            std::env::var("PORTAL_DIR").ok().as_deref(),
            std::path::Path::new(&home),
        ))
        .unwrap();
        let mut fx = ParticleEffects::read(&install).unwrap();
        let from = [-860.0, -128.0, 190.0];
        let id = fx.spawn(Colour::Orange, Effect::Stream, Frame::stream(from), None);
        let cam = ParticleCamera {
            pos: [-960.0, -128.0, 210.0],
            right: [0.0, -1.0, 0.0],
            up: [0.0, 0.0, 1.0],
        };
        let (mut checked, mut widest) = (0, 0.0f32);
        for step in 1..=40 {
            let t = step as f32 / 40.0;
            fx.move_point(id, [from[0] + 490.0 * t, from[1], from[2] - 60.0 * t]);
            fx.simulate(1.0 / 60.0, &ParticleWorld::default());
            let mut nodes = Vec::new();
            walk(&fx.live[&id].system, &mut nodes);
            for node in nodes {
                for renderer in &node.def.renderers {
                    let Renderer::Trail { .. } = renderer else {
                        continue;
                    };
                    for p in &node.c.particles {
                        assert_eq!(p.prev, p.xyz, "trail particle moved at step={step}");
                        checked += 1;
                    }
                    let mesh = renderer.build(&node.c, &cam, &Look::default());
                    for quad in mesh.vertices.chunks(4) {
                        let span = Vec3::from(quad[0].pos).distance(Vec3::from(quad[2].pos));
                        widest = widest.max(span);
                    }
                }
            }
        }
        assert!(checked > 100, "only {checked} trail particles checked");
        assert!(widest < 0.01, "trail streak spans {widest} units");
    }

    #[test]
    fn a_fizzle_warp_draws_after_a_frame_copy_and_its_sparks_take_none() {
        let home = std::env::var("HOME").unwrap();
        let install = Install::open(&portal::install::root(
            None,
            std::env::var("PORTAL_DIR").ok().as_deref(),
            std::path::Path::new(&home),
        ))
        .unwrap();
        let mut fx = ParticleEffects::read(&install).unwrap();
        let wall = Portal::new(
            [100.0, 0.0, 64.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        );
        fx.spawn(
            Colour::Orange,
            Effect::BadSurface,
            Frame::impact(&wall, 1.0),
            None,
        );
        for _ in 0..5 {
            fx.simulate(0.02, &ParticleWorld::default());
        }
        let mut app = App::new();
        app.insert_resource(fx)
            .insert_resource(Files(install.search))
            .insert_resource(Assets::<Image>::default())
            .insert_resource(Assets::<Mesh>::default())
            .insert_resource(Assets::<ParticleMaterial>::default())
            .insert_resource(Assets::<FrameCopy>::default())
            .init_resource::<Draws>()
            .add_systems(Startup, crate::particle_material::load)
            .add_systems(Update, draw);
        app.world_mut().spawn((
            Camera::default(),
            Transform::from_xyz(0.0, 1.0, 0.0),
            ParticleView(32),
        ));
        app.update();
        let world = app.world();
        let warp = &world.resource::<ParticleMaterials>().0["particle\\warp2_warp.vmt"];
        let copy = warp
            .frame_copy
            .clone()
            .expect("the warp asks for a frame copy");
        let copy_material = world.resource::<Assets<FrameCopy>>().get(&copy).unwrap();
        assert!(copy_material.reads_view_transmission_texture());
        assert_eq!(copy_material.alpha_mode(), AlphaMode::Opaque);
        let (mut warps, mut sparks) = (0, 0);
        for (entity, mesh) in world.resource::<Draws>().0.values() {
            let material = world
                .get::<MeshMaterial3d<ParticleMaterial>>(*entity)
                .unwrap();
            let anchors: Vec<_> = world
                .get::<Children>(*entity)
                .map(|c| c.iter().collect())
                .unwrap_or_default();
            if material.0 == warp.handle {
                warps += 1;
                assert_eq!(anchors.len(), 1);
                let anchor = anchors[0];
                assert_eq!(
                    world.get::<MeshMaterial3d<FrameCopy>>(anchor).unwrap().0,
                    copy
                );
                assert_eq!(world.get::<Mesh3d>(anchor).unwrap().0, *mesh);
                assert_eq!(
                    world.get::<RenderLayers>(anchor),
                    world.get::<RenderLayers>(*entity)
                );
            } else {
                sparks += 1;
                assert!(anchors.is_empty());
            }
        }
        assert_eq!(
            (warps > 0, sparks > 0),
            (true, true),
            "warps={warps} sparks={sparks}"
        );
    }

    #[test]
    fn camera_geometry_and_assets_are_reused_and_removed() {
        let home = std::env::var("HOME").unwrap();
        let install = Install::open(&portal::install::root(
            None,
            std::env::var("PORTAL_DIR").ok().as_deref(),
            std::path::Path::new(&home),
        ))
        .unwrap();
        let mut fx = ParticleEffects::read(&install).unwrap();
        let mut pair = Pair::default();
        let portal = Portal::new(
            [100.0, 0.0, 64.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        );
        pair.place(Colour::Blue, portal);
        fx.sync(&pair);
        for _ in 0..20 {
            fx.simulate(0.05, &ParticleWorld::default());
        }
        let mut app = App::new();
        app.insert_resource(fx)
            .insert_resource(Files(install.search))
            .insert_resource(Assets::<Image>::default())
            .insert_resource(Assets::<Mesh>::default())
            .insert_resource(Assets::<ParticleMaterial>::default())
            .insert_resource(Assets::<FrameCopy>::default())
            .init_resource::<Draws>()
            .add_systems(Startup, crate::particle_material::load)
            .add_systems(Update, draw);
        let main = app
            .world_mut()
            .spawn((
                Camera::default(),
                Transform::from_xyz(0.0, 1.0, 0.0),
                ParticleView(32),
            ))
            .id();
        let nested = app
            .world_mut()
            .spawn((
                Camera::default(),
                Transform::from_xyz(1.0, 2.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
                ParticleView(35),
            ))
            .id();
        let gun = app
            .world_mut()
            .spawn((
                Camera::default(),
                Transform::default(),
                RenderLayers::layer(1),
            ))
            .id();
        app.update();
        let count = app.world().resource::<Assets<Mesh>>().len();
        assert!(count >= 4);
        let handles: Vec<_> = app
            .world()
            .resource::<Draws>()
            .0
            .values()
            .map(|(_, h)| h.id())
            .collect();
        app.update();
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), count);
        assert!(handles
            .iter()
            .all(|h| app.world().resource::<Assets<Mesh>>().contains(*h)));
        let draws = &app.world().resource::<Draws>().0;
        assert!(draws.keys().any(|k| k.3 == main));
        assert!(draws.keys().any(|k| k.3 == nested));
        assert!(draws.keys().all(|k| k.3 != gun));
        for ((_, _, _, view), (entity, _)) in draws {
            let layer = app.world().get::<RenderLayers>(*entity).unwrap();
            assert!(layer.intersects(&RenderLayers::layer(if *view == main { 32 } else { 35 })));
            assert!(!layer.intersects(&RenderLayers::layer(if *view == main { 35 } else { 32 })));
            assert!(!layer.intersects(&RenderLayers::layer(0)));
        }
        let matching: Vec<_> = draws
            .iter()
            .filter(|(k, _)| k.1 == 0 && k.2 == 0)
            .map(|(_, (_, h))| h.clone())
            .collect();
        assert_eq!(matching.len(), 2);
        let meshes = app.world().resource::<Assets<Mesh>>();
        assert_ne!(
            format!(
                "{:?}",
                meshes
                    .get(&matching[0])
                    .unwrap()
                    .attribute(Mesh::ATTRIBUTE_POSITION)
            ),
            format!(
                "{:?}",
                meshes
                    .get(&matching[1])
                    .unwrap()
                    .attribute(Mesh::ATTRIBUTE_POSITION)
            )
        );
        fn clear(system: &mut System) {
            system.c.particles.clear();
            for child in &mut system.children {
                clear(child);
            }
        }
        for live in app
            .world_mut()
            .resource_mut::<ParticleEffects>()
            .live
            .values_mut()
        {
            clear(&mut live.system);
        }
        app.update();
        for (entity, handle) in app.world().resource::<Draws>().0.values() {
            assert_eq!(
                *app.world().get::<Visibility>(*entity).unwrap(),
                Visibility::Hidden
            );
            assert!(
                app.world()
                    .resource::<Assets<Mesh>>()
                    .get(handle)
                    .unwrap()
                    .count_vertices()
                    > 0
            );
        }
        for _ in 0..50 {
            pair.place(Colour::Blue, portal);
            let mut fx = app.world_mut().resource_mut::<ParticleEffects>();
            fx.sync(&pair);
            for _ in 0..20 {
                fx.simulate(0.05, &ParticleWorld::default());
            }
            app.update();
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), count);
        }
        pair.deactivate(Colour::Blue);
        app.world_mut()
            .resource_mut::<ParticleEffects>()
            .sync(&pair);
        app.update();
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
        assert!(app.world().resource::<Draws>().0.is_empty());
        assert_eq!(
            app.world_mut().query::<&Mesh3d>().iter(app.world()).count(),
            0
        );
    }
}
