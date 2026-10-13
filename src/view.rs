// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::camera::ClearColorConfig;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use portal::mdl::Mdl;
use portal::model::MeshData;
use portal::pair::Portal;
use portal::teleport::box_meets_portal;

use portal::viewbob::{add_bob, angle_vectors, look_down, Angles, Bob, Lag};

use crate::player::{Config, PlayerCamera};
use crate::settings::horizontal_to_vertical;
use crate::sound::Files;
use crate::texture;
use crate::units::METRES_PER_UNIT;
use crate::viewanim::GunAnim;
use crate::viewmodel::{bindposes, look, skin, Blend};
use crate::visual::VisualPlayer;

pub const VIEW_LAYER: usize = 1;
/// viewmodel_fov's default in the client
const VIEWMODEL_FOV: f32 = 54.0;
/// zNearViewmodel, the immediate 1.0 SetUpView stores at 0x595a8c (disassembly, client.so)
const VIEWMODEL_NEAR: f32 = 1.0;
/// g_fMaxViewModelLag, kept active through portals unless the original is asked for
const MAX_LAG: f32 = 1.5;

#[derive(Resource)]
pub struct Gun {
    pub mdl: Mdl,
    pub anim: GunAnim,
    pub(crate) joints: Vec<Entity>,
    pub(crate) root: Entity,
    motion: ViewMotion,
}

pub struct ViewMotion {
    bob: Bob,
    lag: Lag,
    reset_revision: Option<u64>,
    teleport_rotation: Quat,
    pub max_lag: f32,
}

impl Default for ViewMotion {
    fn default() -> Self {
        Self {
            bob: Bob::default(),
            lag: Lag::default(),
            reset_revision: None,
            teleport_rotation: Quat::IDENTITY,
            max_lag: MAX_LAG,
        }
    }
}

#[derive(Resource, Default)]
pub struct GunMeshes(pub Mdl, pub Vec<MeshData>);

pub struct ViewPlugin;

impl Plugin for ViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<crate::bodyportal::BodyMaterial>()
            .add_systems(PostStartup, spawn_view)
            .add_systems(Update, (settle_to_idle, pose_view).chain())
            .add_systems(PostUpdate, sway.before(TransformSystems::Propagate));
    }
}

/// zero lag in the original while the hull touches a portal, 0x53cca0 via 0x27b200 (client.so)
pub fn max_lag(original: bool, visual: &VisualPlayer, linked: Option<[Portal; 2]>) -> f32 {
    let (centre, half) = (visual.pose.centre.to_array(), visual.pose.half.to_array());
    let touching =
        linked.is_some_and(|pair| pair.iter().any(|p| box_meets_portal(p, centre, half)));
    if original && touching {
        0.0
    } else {
        MAX_LAG
    }
}

/// The view model's place and turn relative to the eye, in Source's view space.
pub fn eye_relative(eye: [f32; 3], angles: Angles, at: [f32; 3], turned: Angles) -> Mat4 {
    let basis = |a: Angles| {
        let (f, r, u) = angle_vectors(a);
        Mat3::from_cols(Vec3::from(f), -Vec3::from(r), Vec3::from(u))
    };
    let e = basis(angles);
    let local = e.transpose() * (Vec3::from(at) - Vec3::from(eye));
    Mat4::from_mat3_translation(e.transpose() * basis(turned), local)
}

/// Translated from 0x53cca0 and 0x20e9d0 (disassembled, client.so), with continuous portal sway.
fn sway(
    gun: Option<ResMut<Gun>>,
    time: Res<Time>,
    visual: Res<VisualPlayer>,
    portals: Option<Res<crate::faces::Portals>>,
    config: Option<Res<Config>>,
    mut transforms: Query<&mut Transform>,
) {
    let Some(mut gun) = gun else { return };
    let gun = &mut *gun;
    let original = config.is_some_and(|c| c.0.original_gun_lag);
    gun.motion.max_lag = max_lag(original, &visual, portals.and_then(|p| p.linked()));
    let local = gun
        .motion
        .relative(&visual, time.elapsed_secs(), time.delta_secs());
    if let Ok(mut root) = transforms.get_mut(gun.root) {
        *root = Transform::from_matrix(source_to_camera().to_matrix() * local);
    }
}

impl ViewMotion {
    pub fn relative(&mut self, visual: &VisualPlayer, now: f32, dt: f32) -> Mat4 {
        let angles = visual.view_angles();
        let eye = visual.pose.eye.to_array();
        if self.reset_revision != Some(visual.reset_revision) {
            self.lag.reset(angles);
            self.bob.last = now;
            self.reset_revision = Some(visual.reset_revision);
        } else if self.teleport_rotation != visual.teleport_rotation {
            let rotation = visual.teleport_rotation * self.teleport_rotation.inverse();
            self.lag.facing = (rotation * Vec3::from(self.lag.facing)).to_array();
        }
        self.teleport_rotation = visual.teleport_rotation;
        self.bob.calc(visual.pose.velocity.to_array(), now, dt);
        let (mut at, mut turned) = add_bob(
            look_down(eye, angles),
            angles,
            self.bob.vertical,
            self.bob.lateral,
        );
        self.lag
            .apply(&mut at, &mut turned, angles, self.max_lag, dt);
        eye_relative(eye, angles, at, turned)
    }
}

/// Source's view model space, x ahead y left z up in inches, inside the camera's.
fn source_to_camera() -> Transform {
    Transform {
        rotation: Quat::from_mat3(&Mat3::from_cols(Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y)),
        scale: Vec3::splat(METRES_PER_UNIT),
        ..default()
    }
}

/// The materialsystem's stand-in for a texture it cannot find.
fn error_texture() -> Image {
    let data = (0..16)
        .flat_map(|i| match (i % 4 < 2) ^ (i / 8 == 1) {
            true => [255, 0, 255, 255],
            false => [0, 0, 0, 255],
        })
        .collect();
    let mut image = Image::new(
        Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::nearest());
    image
}

fn gun_mesh(data: &MeshData) -> Mesh {
    let s = skin(data);
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, s.positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, s.normals)
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_TANGENT,
        data.vertices.iter().map(|v| v.tangent).collect::<Vec<_>>(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, s.uvs)
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_JOINT_INDEX,
        VertexAttributeValues::Uint16x4(s.joints),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, s.weights)
    .with_inserted_indices(Indices::U32(s.indices))
}

#[allow(clippy::too_many_arguments)]
fn spawn_view(
    mut commands: Commands,
    mut loaded: ResMut<GunMeshes>,
    files: Res<Files>,
    config: Res<Config>,
    camera: Single<(Entity, &Msaa), With<PlayerCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<crate::bodyportal::BodyMaterial>>,
    mut binds: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    mut exit: MessageWriter<AppExit>,
) {
    let GunMeshes(mdl, pieces) = std::mem::take(&mut *loaded);
    let (player_camera, msaa) = *camera;
    let view_camera = commands
        .spawn((
            Camera3d::default(),
            Camera {
                order: 1,
                clear_color: ClearColorConfig::None,
                ..default()
            },
            *msaa,
            Projection::Perspective(PerspectiveProjection {
                fov: horizontal_to_vertical(VIEWMODEL_FOV),
                near: VIEWMODEL_NEAR * METRES_PER_UNIT,
                ..default()
            }),
            RenderLayers::layer(VIEW_LAYER),
            ChildOf(player_camera),
        ))
        .id();
    let root = commands
        .spawn((
            source_to_camera(),
            Visibility::Inherited,
            ChildOf(view_camera),
        ))
        .id();
    let joints: Vec<Entity> = mdl
        .bones
        .iter()
        .map(|b| {
            commands
                .spawn((
                    Transform::from_translation(Vec3::from(b.pos))
                        .with_rotation(Quat::from_array(b.quat)),
                    Visibility::Inherited,
                ))
                .id()
        })
        .collect();
    for (i, b) in mdl.bones.iter().enumerate() {
        commands
            .entity(joints[i])
            .insert(ChildOf(b.parent.map_or(root, |p| joints[p])));
    }
    let bind = binds.add(SkinnedMeshInverseBindposes::from(
        bindposes(&mdl)
            .iter()
            .map(Mat4::from_cols_array)
            .collect::<Vec<_>>(),
    ));
    let error = images.add(error_texture());
    for piece in &pieces {
        let l = match look(&files, &mdl, piece.material) {
            Ok(l) => l,
            Err(e) => {
                error!("[view] refused: material={} reason={e}", piece.material);
                exit.write(AppExit::error());
                return;
            }
        };
        let image = match &l.texture {
            Some(name) => match texture::load(&files, name) {
                Ok(i) => images.add(i),
                Err(e) => {
                    error!("[view] refused: texture={name} reason={e}");
                    exit.write(AppExit::error());
                    return;
                }
            },
            None => {
                warn!(
                    "[view] texture_missing material={} name={}",
                    mdl.textures[piece.material],
                    l.missing.as_deref().unwrap_or("")
                );
                error.clone()
            }
        };
        let alpha_mode = match l.blend {
            Blend::Opaque => AlphaMode::Opaque,
            Blend::Blend => AlphaMode::Blend,
            Blend::Add => AlphaMode::Add,
        };
        let mut material = crate::bodyportal::BodyMaterial {
            base: StandardMaterial {
                base_color_texture: Some(image),
                alpha_mode,
                perceptual_roughness: 0.6,
                ..default()
            },
            extension: default(),
        };
        if let Err(reason) = crate::model_material::apply(&l, &files, &mut images, &mut material) {
            error!(
                "[view] refused: material={} reason={reason}",
                piece.material
            );
            exit.write(AppExit::error());
            return;
        }
        commands.spawn((
            Mesh3d(meshes.add(gun_mesh(piece))),
            MeshMaterial3d(materials.add(material)),
            SkinnedMesh {
                inverse_bindposes: bind.clone(),
                joints: joints.clone(),
            },
            NoFrustumCulling,
            // Translated from 0x21f390 and 0x21f3b0 (decompiled, client.so).
            NotShadowCaster,
            NotShadowReceiver,
            RenderLayers::layer(VIEW_LAYER),
            ChildOf(root),
        ));
    }
    let mut anim = GunAnim::default();
    anim.play(&mdl, "ACT_VM_DRAW", 0.0);
    info!(
        "[view] spawned bones={} pieces={} fov={} msaa={}",
        joints.len(),
        pieces.len(),
        VIEWMODEL_FOV,
        config.0.msaa
    );
    commands.insert_resource(Gun {
        mdl,
        anim,
        joints,
        root,
        motion: ViewMotion::default(),
    });
    commands.remove_resource::<GunMeshes>();
}

pub(crate) fn settle_to_idle(
    gun: Option<ResMut<Gun>>,
    time: Res<Time>,
    effects: Option<Res<crate::gun_visual::GunVisualState>>,
) {
    if effects.is_some_and(|effects| effects.holding) {
        return;
    }
    let Some(mut gun) = gun else { return };
    let now = time.elapsed_secs();
    let Gun { mdl, anim, .. } = &mut *gun;
    let s = &mdl.sequences[anim.seq];
    if !s.looping && anim.left(mdl, now) == 0.0 {
        anim.play(mdl, "ACT_VM_IDLE", now);
    }
}

pub(crate) fn pose_view(
    gun: Option<Res<Gun>>,
    time: Res<Time>,
    mut transforms: Query<&mut Transform>,
    mut pose_error: Local<Option<String>>,
) {
    let Some(gun) = gun else { return };
    let pose = match gun.anim.pose(&gun.mdl, time.elapsed_secs()) {
        Ok(p) => {
            *pose_error = None;
            p
        }
        Err(e) => {
            let e = e.to_string();
            if pose_error.as_ref() != Some(&e) {
                warn!(
                    "[view] pose=failed reason={e} sequence={} at={}",
                    gun.anim.seq,
                    time.elapsed_secs()
                );
                *pose_error = Some(e);
            }
            return;
        }
    };
    for (joint, p) in gun.joints.iter().zip(pose) {
        if let Ok(mut t) = transforms.get_mut(*joint) {
            t.translation = Vec3::from(p.pos);
            t.rotation = Quat::from_array(p.quat);
        }
    }
}
