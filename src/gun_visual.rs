// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use portal::gun_fx::{beam_noise, Beam, GunFx, Sprite, State};

use crate::particle_material::{MaterialSource, ParticleMaterial, FRAME_BLEND};
use crate::sound::Files;
use crate::units::METRES_PER_UNIT;
use crate::view::{Gun, VIEW_LAYER};

#[derive(Resource, Default)]
pub struct GunVisualState {
    pub holding: bool,
    pub fizzle_revision: u64,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GunVisualUpdate;

#[derive(Resource)]
struct Effects {
    fx: GunFx,
    world_fx: GunFx,
    world_attachments: HashMap<String, Vec3>,
    noise: [[f32; 129]; 6],
    frequency: f32,
    holding: bool,
    fizzle_revision: u64,
    max_size: f32,
    next_think: f32,
    rng: crate::random::Xorshift,
    materials: HashMap<&'static str, Handle<ParticleMaterial>>,
    draws: HashMap<(Entity, bool, &'static str), (Entity, Handle<Mesh>)>,
}

pub struct GunVisualPlugin;
impl Plugin for GunVisualPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GunVisualState>()
            .add_systems(Update, initialize.before(GunVisualUpdate))
            .add_systems(
                Update,
                update
                    .in_set(GunVisualUpdate)
                    .before(crate::view::settle_to_idle)
                    .before(crate::view::pose_view)
                    .run_if(crate::menu::playing),
            )
            .add_systems(Update, holding_sound.after(GunVisualUpdate))
            .add_systems(PostUpdate, draw.after(TransformSystems::Propagate));
    }
}

fn initialize(
    mut commands: Commands,
    gun: Option<Res<Gun>>,
    effects: Option<Res<Effects>>,
    files: Res<Files>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<ParticleMaterial>>,
) {
    if gun.is_none() || effects.is_some() {
        return;
    }
    let mut fx = GunFx::new(0.0);
    fx.set_state(State::Holding, None, 0.0);
    fx.think(Some(1), 0.01, 0.0, &mut || 0.0);
    let mut names: Vec<_> = fx.sprites(0.0).iter().map(|s| s.material).collect();
    names.extend([
        "sprites/orangelight",
        "sprites/grav_beam_noz",
        "sprites/grav_beam",
    ]);
    names.sort_unstable();
    names.dedup();
    let mut loaded = HashMap::new();
    for name in names {
        let material = MaterialSource::read_gun(&files, name)
            .unwrap_or_else(|e| panic!("[gun-effects] refused: {name}: {e}"));
        loaded.insert(name, material.upload(&mut images, &mut materials));
    }
    let mut fx = GunFx::new(0.0);
    fx.set_state(State::Ready, None, 0.0);
    let mut world_fx = GunFx::world(0.0);
    world_fx.set_state(State::Ready, None, 0.0);
    let mdl = portal::mdl::Mdl::parse(
        files
            .read("models/weapons/w_portalgun.mdl")
            .expect("world gun model"),
    )
    .expect("world gun model layout");
    let binds = crate::viewmodel::bindposes(&mdl);
    let world_attachments = mdl
        .attachments
        .iter()
        .map(|a| {
            let local = Vec3::new(a.local[0][3], a.local[1][3], a.local[2][3]);
            let bone = Mat4::from_cols_array(&binds[a.bone]).inverse();
            (
                a.name.clone(),
                bone.transform_point3(local) * METRES_PER_UNIT,
            )
        })
        .collect();
    commands.insert_resource(Effects {
        fx,
        world_fx,
        world_attachments,
        noise: [[0.0; 129]; 6],
        frequency: 0.0,
        holding: false,
        fizzle_revision: 0,
        max_size: 4.0,
        next_think: 0.0,
        rng: default(),
        materials: loaded,
        draws: default(),
    });
}

fn update(
    state: Res<GunVisualState>,
    effects: Option<ResMut<Effects>>,
    gun: Option<ResMut<Gun>>,
    firing: Res<crate::firing::Firing>,
    time: Res<Time>,
    fixed: Res<Time<Fixed>>,
) {
    let (Some(mut effects), Some(mut gun)) = (effects, gun) else {
        return;
    };
    let effects = &mut *effects;
    let now = time.elapsed_secs();
    let last = (firing.last_fired != 0).then_some(firing.last_fired);
    if effects.holding != state.holding {
        effects.holding = state.holding;
        let effect_state = if state.holding {
            State::Holding
        } else {
            State::Ready
        };
        effects.fx.set_state(effect_state, last, now);
        effects.world_fx.set_state(effect_state, last, now);
        let Gun { mdl, anim, .. } = &mut *gun;
        anim.play(
            mdl,
            if state.holding {
                "ACT_VM_PICKUP"
            } else {
                "ACT_VM_RELEASE"
            },
            now,
        );
        info!("[gun-effects] holding={}", state.holding);
    }
    if effects.fizzle_revision != state.fizzle_revision {
        effects.fizzle_revision = state.fizzle_revision;
        effects.max_size = 50.0;
        let Gun { mdl, anim, .. } = &mut *gun;
        anim.play(mdl, "ACT_VM_FIZZLE", now);
    }
    if now >= effects.next_think {
        effects.max_size = (effects.max_size - 400.0 * fixed.timestep().as_secs_f32()).max(4.0);
        effects.next_think = now + 0.1;
    }
    effects.fx.set_max_size(effects.max_size);
    effects
        .fx
        .think(last, time.delta_secs(), now, &mut || effects.rng.unit());
    effects.world_fx.set_max_size(effects.max_size);
    effects
        .world_fx
        .think(last, time.delta_secs(), now, &mut || effects.rng.unit());
    effects.frequency += time.delta_secs();
    for noise in &mut effects.noise {
        *noise = beam_noise(&mut || effects.rng.range(-1.0, 1.0));
    }
}

#[derive(Default)]
struct HoldSound {
    entity: Option<Entity>,
    volume: f32,
}

#[allow(clippy::too_many_arguments)]
fn holding_sound(
    mut commands: Commands,
    state: Res<GunVisualState>,
    mut sound: Local<HoldSound>,
    sounds: Option<ResMut<crate::sound::Sounds>>,
    files: Res<Files>,
    audio: Option<ResMut<Assets<AudioSource>>>,
    mut sinks: Query<&mut AudioSink>,
    time: Res<Time>,
) {
    if state.holding && sound.entity.is_none() {
        if let (Some(mut sounds), Some(mut audio)) = (sounds, audio) {
            sound.entity = sounds.looped(
                &mut commands,
                &files,
                &mut audio,
                "Weapon_Portalgun.HoldSound",
                0.0,
            );
        }
    }
    let target = if state.holding { 1.0 } else { 0.0 };
    let step = time.delta_secs() / 0.1;
    sound.volume += (target - sound.volume).clamp(-step, step);
    if let Some(entity) = sound.entity {
        if let Ok(mut sink) = sinks.get_mut(entity) {
            sink.set_volume(bevy::audio::Volume::Linear(sound.volume));
        }
    }
}

pub fn attachment_position(gun: &Gun, name: &str, transforms: &Query<&GlobalTransform>) -> Vec3 {
    let index = gun
        .mdl
        .attachment(name)
        .unwrap_or_else(|| panic!("[gun-effects] missing_attachment={name}"));
    let a = &gun.mdl.attachments[index];
    let bone = transforms
        .get(gun.joints[a.bone])
        .expect("gun joint transform");
    bone.transform_point(Vec3::new(a.local[0][3], a.local[1][3], a.local[2][3]))
}

#[derive(Default)]
pub struct Geometry {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}
impl Geometry {
    fn quad(&mut self, corners: [Vec3; 4], color: [f32; 4], uv: [[f32; 2]; 4]) {
        let base = self.positions.len() as u32;
        self.positions.extend(corners.map(|v| v.to_array()));
        self.uvs.extend(uv);
        self.colors.extend([color; 4]);
        self.indices.extend([0, 1, 2, 0, 2, 3].map(|i| base + i));
    }

    pub fn sprite(&mut self, sprite: &Sprite, at: Vec3, right: Vec3, up: Vec3) {
        let radius = sprite.size * METRES_PER_UNIT * 0.5;
        let r = right * radius;
        let u = up * radius;
        self.quad(
            [at - r + u, at - r - u, at + r - u, at + r + u],
            [
                sprite.color[0] / 255.0,
                sprite.color[1] / 255.0,
                sprite.color[2] / 255.0,
                sprite.alpha / 255.0,
            ],
            [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn beam(
        &mut self,
        beam: &Beam,
        start: Vec3,
        end: Vec3,
        eye: Vec3,
        forward: Vec3,
        noise: &[f32; 129],
        frequency: f32,
    ) {
        let delta = end - start;
        let noise_axis = forward.cross(delta.normalize_or_zero()).normalize_or_zero();
        let points = beam.points(delta.length() / METRES_PER_UNIT, frequency, noise);
        let points: Vec<_> = points
            .into_iter()
            .map(|p| {
                (
                    start + delta * p.fraction + noise_axis * p.offset * METRES_PER_UNIT,
                    p.radius * METRES_PER_UNIT,
                    p.texture,
                )
            })
            .collect();
        let normals: Vec<_> = points
            .windows(2)
            .map(|p| (p[0].0 - p[1].0).cross(p[0].0 - eye).normalize_or_zero())
            .collect();
        let vertices: Vec<_> = points
            .iter()
            .enumerate()
            .map(|(i, &(at, radius, uv))| {
                let side = if i == 0 {
                    normals[0]
                } else if i == points.len() - 1 {
                    normals[i - 1]
                } else {
                    (normals[i - 1] + normals[i]).normalize_or_zero()
                };
                (at - side * radius, at + side * radius, uv)
            })
            .collect();
        for p in vertices.windows(2) {
            let brightness = beam.alpha / 255.0;
            self.quad(
                [p[0].0, p[0].1, p[1].1, p[1].0],
                [brightness, brightness, brightness, 1.0],
                [[1.0, p[0].2], [0.0, p[0].2], [0.0, p[1].2], [1.0, p[1].2]],
            );
        }
    }

    pub fn clip(&mut self, plane: Vec4) {
        if plane == Vec4::ZERO {
            return;
        }
        let mut clipped = Self::default();
        for triangle in self.indices.as_chunks::<3>().0 {
            let mut polygon: Vec<_> = triangle
                .iter()
                .map(|&i| {
                    let i = i as usize;
                    (
                        Vec3::from(self.positions[i]),
                        Vec2::from(self.uvs[i]),
                        Vec4::from(self.colors[i]),
                    )
                })
                .collect();
            let mut output = Vec::new();
            for i in 0..polygon.len() {
                let a = polygon[i];
                let b = polygon[(i + 1) % polygon.len()];
                let da = plane.dot(a.0.extend(1.0));
                let db = plane.dot(b.0.extend(1.0));
                if da >= 0.0 {
                    output.push(a);
                }
                if (da >= 0.0) != (db >= 0.0) {
                    let t = da / (da - db);
                    output.push((a.0.lerp(b.0, t), a.1.lerp(b.1, t), a.2.lerp(b.2, t)));
                }
            }
            polygon = output;
            for i in 1..polygon.len().saturating_sub(1) {
                for p in [polygon[0], polygon[i], polygon[i + 1]] {
                    clipped.indices.push(clipped.positions.len() as u32);
                    clipped.positions.push(p.0.to_array());
                    clipped.uvs.push(p.1.to_array());
                    clipped.colors.push(p.2.to_array());
                }
            }
        }
        *self = clipped;
    }

    fn mesh(self) -> Mesh {
        let count = self.positions.len();
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, self.uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_attribute(FRAME_BLEND, vec![0.0; count])
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn draw(
    mut commands: Commands,
    effects: Option<ResMut<Effects>>,
    gun: Option<Res<Gun>>,
    time: Res<Time>,
    transforms: Query<&GlobalTransform>,
    cameras: Query<
        (
            Entity,
            &Camera,
            &GlobalTransform,
            &RenderLayers,
            Option<&crate::effects::ParticleView>,
        ),
        With<Camera3d>,
    >,
    held: Query<(Entity, &GlobalTransform), With<crate::body::HeldGun>>,
    model_materials: Query<(&MeshMaterial3d<crate::bodyportal::BodyMaterial>, &ChildOf)>,
    ghost: Option<Res<crate::bodyportal::Ghost>>,
    body_materials: Res<Assets<crate::bodyportal::BodyMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut visibility: Query<&mut Visibility>,
) {
    let (Some(mut effects), Some(gun)) = (effects, gun) else {
        return;
    };
    let mut built: HashMap<(Entity, bool, &'static str), Geometry> = HashMap::new();
    let source_plane = held
        .single()
        .ok()
        .and_then(|(entity, _)| {
            model_materials
                .iter()
                .find(|(_, parent)| parent.parent() == entity)
                .and_then(|(handle, _)| body_materials.get(&handle.0))
        })
        .map_or(Vec4::ZERO, |m| m.extension.plane);
    let exit_plane = body_materials
        .iter()
        .find(|(_, m)| m.extension.ghost)
        .map_or(Vec4::ZERO, |(_, m)| m.extension.plane);
    let ghost_matrix = ghost.as_ref().and_then(|g| {
        if visibility
            .get(g.root)
            .is_ok_and(|v| *v != Visibility::Hidden)
        {
            Some(
                transforms.get(g.root).ok()?.to_matrix()
                    * transforms.get(g.original).ok()?.to_matrix().inverse(),
            )
        } else {
            None
        }
    });
    for (view, camera, transform, layers, particle_view) in &cameras {
        if !camera.is_active {
            continue;
        }
        let first_person = *layers == RenderLayers::layer(VIEW_LAYER);
        if !first_person && particle_view.is_none() {
            continue;
        }
        for copy in [false, true] {
            if first_person && copy {
                continue;
            }
            let (fx, plane, matrix) = if first_person {
                (&effects.fx, Vec4::ZERO, Mat4::IDENTITY)
            } else {
                let layer = if copy {
                    crate::bodyportal::GHOST_LAYER
                } else {
                    crate::body::BODY_LAYER
                };
                if !layers.intersects(&RenderLayers::layer(layer)) {
                    continue;
                }
                let Ok((_, held)) = held.single() else {
                    continue;
                };
                let matrix = if copy {
                    let Some(ghost) = ghost_matrix else {
                        continue;
                    };
                    ghost * held.to_matrix()
                } else {
                    held.to_matrix()
                };
                (
                    &effects.world_fx,
                    if copy { exit_plane } else { source_plane },
                    matrix,
                )
            };
            let position = |name: &str| {
                if first_person {
                    attachment_position(&gun, name, &transforms)
                } else {
                    matrix.transform_point3(effects.world_attachments[name])
                }
            };
            let rotation = transform.rotation();
            for sprite in fx.sprites(time.elapsed_secs()) {
                built
                    .entry((view, copy, sprite.material))
                    .or_default()
                    .sprite(
                        &sprite,
                        position(sprite.attachment),
                        rotation * Vec3::X,
                        rotation * Vec3::Y,
                    );
            }
            for (i, beam) in fx.beams().iter().enumerate() {
                built.entry((view, copy, beam.material)).or_default().beam(
                    beam,
                    position(beam.start),
                    position(beam.end),
                    transform.translation(),
                    rotation * Vec3::NEG_Z,
                    &effects.noise[i + if first_person { 0 } else { 3 }],
                    effects.frequency,
                );
            }
            for ((v, c, _), geometry) in &mut built {
                if *v == view && *c == copy {
                    geometry.clip(plane);
                }
            }
        }
    }
    built.retain(|_, g| !g.indices.is_empty());
    for (key, (entity, _)) in &effects.draws {
        if let Ok(mut v) = visibility.get_mut(*entity) {
            *v = if built.contains_key(key) {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
    for (key, geometry) in built {
        let mesh = geometry.mesh();
        if let Some((_, handle)) = effects.draws.get(&key) {
            *meshes.get_mut(handle).expect("gun effects mesh") = mesh;
        } else {
            let handle = meshes.add(mesh);
            let (_, _, _, _, particle_view) = cameras.get(key.0).expect("effect camera");
            let layer = particle_view.map_or(VIEW_LAYER, |v| v.0);
            let entity = commands
                .spawn((
                    Mesh3d(handle.clone()),
                    MeshMaterial3d(effects.materials[key.2].clone()),
                    Transform::IDENTITY,
                    Visibility::Visible,
                    NoFrustumCulling,
                    NotShadowCaster,
                    NotShadowReceiver,
                    RenderLayers::layer(layer),
                ))
                .id();
            effects.draws.insert(key, (entity, handle));
        }
    }
}
