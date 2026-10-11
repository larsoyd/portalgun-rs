// Rig geometry and segment posing adapted from Funny-Bones and contributors (MIT).
// Copyright (c) 2026 Funny-Bones and contributors.
pub use crate::bodyanim::BodyAnimation;
use crate::bodyanim::PlayerClips;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::player::Player;
use crate::sound::Files;
use crate::units::{to_bevy, to_bevy_dir, METRES_PER_UNIT};
use crate::visual::VisualPlayer;

pub const BODY_LAYER: usize = 16;

#[derive(Clone, Copy)]
enum Skin {
    Armour,
    Cloth,
}

#[derive(Clone, Copy)]
#[allow(non_camel_case_types)]
#[repr(usize)]
enum Joint {
    Pelvis,
    Spine1,
    Spine2,
    Neck,
    Head,
    L_Thigh,
    R_Thigh,
    L_UpperArm,
    R_UpperArm,
    L_Forearm,
    L_Hand,
    R_Forearm,
    R_Hand,
    L_Calf,
    L_Foot,
    L_Toe0,
    R_Calf,
    R_Foot,
    R_Toe0,
}
use Joint::*;

const SEGMENTS: &[(Joint, Joint, f32, Skin)] = &[
    (Pelvis, Spine1, 0.125, Skin::Cloth),
    (Spine1, Spine2, 0.135, Skin::Cloth),
    (Spine2, Neck, 0.12, Skin::Cloth),
    (Neck, Head, 0.05, Skin::Armour),
    (L_Thigh, R_Thigh, 0.095, Skin::Armour),
    (L_UpperArm, R_UpperArm, 0.07, Skin::Armour),
    (L_UpperArm, L_Forearm, 0.05, Skin::Armour),
    (L_Forearm, L_Hand, 0.042, Skin::Armour),
    (R_UpperArm, R_Forearm, 0.05, Skin::Armour),
    (R_Forearm, R_Hand, 0.042, Skin::Armour),
    (L_Thigh, L_Calf, 0.075, Skin::Armour),
    (L_Calf, L_Foot, 0.058, Skin::Armour),
    (L_Foot, L_Toe0, 0.045, Skin::Armour),
    (R_Thigh, R_Calf, 0.075, Skin::Armour),
    (R_Calf, R_Foot, 0.058, Skin::Armour),
    (R_Foot, R_Toe0, 0.045, Skin::Armour),
];

/// (joint, radius): rounds off the ends of the segments.
const BALLS: &[(Joint, f32)] = &[
    (L_UpperArm, 0.07),
    (R_UpperArm, 0.07),
    (L_Forearm, 0.05),
    (R_Forearm, 0.05),
    (L_Hand, 0.042),
    (R_Hand, 0.042),
    (L_Thigh, 0.095),
    (R_Thigh, 0.095),
    (L_Calf, 0.075),
    (R_Calf, 0.075),
    (L_Foot, 0.058),
    (R_Foot, 0.058),
    (L_Toe0, 0.045),
    (R_Toe0, 0.045),
    (Pelvis, 0.125),
    (Neck, 0.12),
];

#[derive(Component)]
pub struct PlayerBody;

#[derive(Component)]
pub struct HeldGun;

#[derive(Resource)]
struct Rig {
    root: Entity,
    segments: Vec<(Entity, Joint, Joint)>,
    balls: Vec<(Entity, Joint)>,
    head: Entity,
    gun: Entity,
    hands: Vec<HandPiece>,
}

struct HandPiece {
    entity: Entity,
    from: usize,
    to: usize,
    tip: bool,
    across: [usize; 2],
}

pub struct BodyPlugin;

impl Plugin for BodyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BodyAnimation>()
            .add_systems(Startup, spawn_body)
            .add_systems(PostUpdate, animate.before(TransformSystems::Propagate));
    }
}

fn spawn_body(
    mut commands: Commands,
    files: Res<Files>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut exit: MessageWriter<AppExit>,
) {
    let clips = match PlayerClips::read(&files) {
        Ok(clips) => clips,
        Err(reason) => {
            error!("[body] refused: {reason}");
            exit.write(AppExit::error());
            return;
        }
    };
    let armour = materials.add(StandardMaterial {
        base_color: Color::srgb(0.36, 0.38, 0.43),
        perceptual_roughness: 0.6,
        metallic: 0.4,
        ..default()
    });
    let cloth = materials.add(Color::srgb(0.42, 0.16, 0.13));
    let steel = materials.add(Color::srgb(0.85, 0.87, 0.9));
    let root = commands
        .spawn((PlayerBody, Transform::default(), Visibility::Inherited))
        .id();
    let mut part = |parent: Entity, mesh: Mesh, material: &Handle<StandardMaterial>, at: Vec3| {
        commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(at),
                RenderLayers::layer(BODY_LAYER),
                ChildOf(parent),
            ))
            .id()
    };
    let segments = SEGMENTS
        .iter()
        .map(|&(a, b, radius, skin)| {
            let material = match skin {
                Skin::Armour => &armour,
                Skin::Cloth => &cloth,
            };
            (
                part(
                    root,
                    Cylinder::new(radius, 1.0).into(),
                    material,
                    Vec3::ZERO,
                ),
                a,
                b,
            )
        })
        .collect();
    let balls = BALLS
        .iter()
        .map(|&(joint, radius)| {
            let material = if matches!(joint, Pelvis | Neck) {
                &cloth
            } else {
                &armour
            };
            (
                part(root, Sphere::new(radius).into(), material, Vec3::ZERO),
                joint,
            )
        })
        .collect();
    let mut hands = Vec::new();
    for side in ["L", "R"] {
        let bone = |name: &str| clips.bone(&format!("{name}_{side}"));
        let across = [bone("index_base"), bone("mid_base")];
        for (chain, width, thickness) in [
            (
                vec![
                    bone("wrist"),
                    bone("mid_base"),
                    bone("mid_mid"),
                    bone("mid_end"),
                ],
                0.085,
                0.032,
            ),
            (
                vec![bone("thumb_base"), bone("thumb_mid"), bone("thumb_end")],
                0.028,
                0.028,
            ),
        ] {
            for n in 0..chain.len() {
                let tip = n == chain.len() - 1;
                let from = chain[if tip { n - 1 } else { n }];
                let to = chain[if tip { n } else { n + 1 }];
                let entity = part(
                    root,
                    Cuboid::new(width, 1.0, thickness).into(),
                    &armour,
                    Vec3::ZERO,
                );
                hands.push(HandPiece {
                    entity,
                    from,
                    to,
                    tip,
                    across,
                });
            }
        }
    }
    let head = commands
        .spawn((Transform::default(), Visibility::Inherited, ChildOf(root)))
        .id();
    for (mesh, material, at) in [
        (Mesh::from(Sphere::new(0.115)), armour, Vec3::Y * 0.09),
        (
            Mesh::from(Cuboid::new(0.13, 0.035, 0.07)),
            steel,
            Vec3::new(0.0, 0.1, -0.09),
        ),
    ] {
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material),
            Transform::from_translation(at),
            RenderLayers::layer(BODY_LAYER),
            ChildOf(head),
        ));
    }
    let gun = commands
        .spawn((
            HeldGun,
            Transform::default(),
            Visibility::Inherited,
            ChildOf(root),
        ))
        .id();
    if let Err(reason) = spawn_gun(
        &mut commands,
        gun,
        &files,
        &mut meshes,
        &mut materials,
        &mut images,
    ) {
        error!("[body] refused: {reason}");
        exit.write(AppExit::error());
    }
    commands.insert_resource(clips);
    commands.insert_resource(Rig {
        root,
        segments,
        balls,
        head,
        gun,
        hands,
    });
}

fn spawn_gun(
    commands: &mut Commands,
    parent: Entity,
    files: &Files,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) -> Result<(), String> {
    let (mdl, pieces) = crate::viewmodel::read_model(files, "models/weapons/w_portalgun")?;
    for piece in pieces {
        let look = crate::viewmodel::look(files, &mdl, piece.material)?;
        let name = look.texture.ok_or("world gun texture missing")?;
        let texture = images.add(crate::texture::load(files, &name)?);
        let skin = crate::viewmodel::skin(&piece);
        let rotation = Quat::IDENTITY;
        let positions: Vec<_> = skin
            .positions
            .iter()
            .map(|&p| (rotation * Vec3::from(p) * METRES_PER_UNIT).to_array())
            .collect();
        let normals: Vec<_> = skin
            .normals
            .iter()
            .map(|&n| (rotation * Vec3::from(n)).to_array())
            .collect();
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, skin.uvs)
        .with_inserted_indices(Indices::U32(skin.indices));
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color_texture: Some(texture),
                perceptual_roughness: 0.6,
                ..default()
            })),
            RenderLayers::layer(BODY_LAYER),
            ChildOf(parent),
        ));
    }
    Ok(())
}

fn animate(
    rig: Option<Res<Rig>>,
    clips: Option<Res<PlayerClips>>,
    mut animation: ResMut<BodyAnimation>,
    visual: Res<VisualPlayer>,
    player: Res<Player>,
    time: Res<Time>,
    mut transforms: Query<&mut Transform>,
) {
    let (Some(rig), Some(clips)) = (rig, clips) else {
        return;
    };
    let pose = match animation.pose(
        &clips,
        &visual,
        player.grounded,
        player.duck.ducked || player.duck.ducking,
        time.elapsed_secs(),
        time.delta_secs(),
    ) {
        Ok(pose) => pose,
        Err(reason) => {
            error!("[body] pose refused: {reason}");
            return;
        }
    };
    let names = [
        "hips",
        "spine_mid",
        "chest",
        "neck",
        "head",
        "thigh_L",
        "thigh_R",
        "bicep_L",
        "bicep_R",
        "elbow_L",
        "wrist_L",
        "elbow_R",
        "wrist_R",
        "knee_L",
        "ankle_L",
        "toe_L",
        "knee_R",
        "ankle_R",
        "toe_R",
    ];
    let joints =
        names.map(|name| to_bevy(pose.bones[clips.bone(name)].w_axis.truncate().to_array()));
    for &(entity, a, b) in &rig.segments {
        let (a, b) = (joints[a as usize], joints[b as usize]);
        if let Ok(mut transform) = transforms.get_mut(entity) {
            let span = b - a;
            let length = span.length().max(1e-4);
            transform.translation = (a + b) / 2.0;
            transform.rotation = Quat::from_rotation_arc(Vec3::Y, span / length);
            transform.scale = Vec3::new(1.0, length, 1.0);
        }
    }
    for &(entity, joint) in &rig.balls {
        if let Ok(mut transform) = transforms.get_mut(entity) {
            transform.translation = joints[joint as usize];
        }
    }
    let point = |bone: usize| to_bevy(pose.bones[bone].w_axis.truncate().to_array());
    for hand in &rig.hands {
        let (mut a, mut b) = (point(hand.from), point(hand.to));
        if hand.tip {
            (a, b) = (b, b + (b - a) * 0.8);
        }
        let span = b - a;
        let length = span.length().max(1e-4);
        let along = span / length;
        let across = point(hand.across[0]) - point(hand.across[1]);
        let face = across.cross(along).normalize_or(Vec3::Z);
        if let Ok(mut t) = transforms.get_mut(hand.entity) {
            t.translation = (a + b) / 2.0;
            t.rotation = Quat::from_mat3(&Mat3::from_cols(along.cross(face), along, face));
            t.scale = Vec3::new(1.0, length * 1.12, 1.0);
        }
    }
    let basis = Mat4::from_cols(
        to_bevy_dir([1.0, 0.0, 0.0]).extend(0.0),
        to_bevy_dir([0.0, 1.0, 0.0]).extend(0.0),
        to_bevy_dir([0.0, 0.0, 1.0]).extend(0.0),
        Vec4::W,
    );
    let bone_transform = |name| {
        let mut t = Transform::from_matrix(basis * pose.bones[clips.bone(name)]);
        t.translation *= METRES_PER_UNIT;
        t
    };
    let feet = pose.origin;
    for (entity, transform) in [
        (
            rig.root,
            Transform::from_translation(to_bevy(feet.to_array()))
                .with_rotation(Quat::from_rotation_y(pose.yaw.to_radians())),
        ),
        (rig.gun, bone_transform("weapon_bone")),
        (
            rig.head,
            bone_transform("head").with_rotation(
                bone_transform("head").rotation * Quat::from_rotation_y(std::f32::consts::PI),
            ),
        ),
    ] {
        if let Ok(mut t) = transforms.get_mut(entity) {
            *t = transform;
        }
    }
}
