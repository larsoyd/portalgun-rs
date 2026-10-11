use bevy::asset::embedded_asset;
use bevy::camera::primitives::{Aabb, MeshAabb};
use bevy::camera::visibility::RenderLayers;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use portal::pair::Portal;
use std::collections::HashMap;

use crate::body::PlayerBody;
use crate::effects::ParticleView;
use crate::faces::{PortalViewsUpdated, Portals};
use crate::units::{from_bevy, to_bevy, to_bevy_dir, METRES_PER_UNIT};
use crate::visual::VisualPlayer;

pub const GHOST_LAYER: usize = 17;
pub type BodyMaterial = ExtendedMaterial<StandardMaterial, BodyClip>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct BodyClip {
    #[uniform(100)]
    pub plane: Vec4,
    pub ghost: bool,
}

impl MaterialExtension for BodyClip {
    fn fragment_shader() -> ShaderRef {
        "embedded://portalgun/shaders/body.wgsl".into()
    }
    fn prepass_fragment_shader() -> ShaderRef {
        "embedded://portalgun/shaders/body_prepass.wgsl".into()
    }
}

pub fn material(mut base: StandardMaterial) -> BodyMaterial {
    base.alpha_mode = AlphaMode::Mask(0.5);
    BodyMaterial {
        base,
        extension: BodyClip::default(),
    }
}

#[derive(Component)]
pub struct PortalGhost;

#[derive(Resource)]
struct Ghost {
    original: Entity,
    root: Entity,
    copies: Vec<CopyPart>,
}

struct CopyPart {
    source: Entity,
    target: Entity,
    parent: Entity,
    bounds: Option<Aabb>,
}

pub struct Split {
    pub source: Vec4,
    pub exit: Vec4,
    pub transform: Mat4,
}

// Translated from 0x5562a0 (decompiled, client.so), using capsule rig bounds.
pub fn split(pair: Option<[Portal; 2]>, centre: Vec3, half: Vec3) -> Option<Split> {
    let pair = pair?;
    let i = pair.iter().position(|p| {
        p.local(centre.to_array())[0] >= -5.0
            && portal::teleport::meets_opening(p, centre.to_array(), half.to_array())
    })?;
    let plane = |p: &Portal, bias| {
        let normal = to_bevy_dir(p.forward);
        normal.extend(-normal.dot(to_bevy(p.origin)) + bias * METRES_PER_UNIT)
    };
    Some(Split {
        source: plane(&pair[i], 0.3),
        exit: plane(&pair[1 - i], 0.75),
        transform: crate::views::through(&pair[i], &pair[1 - i]),
    })
}

pub struct BodyPortalPlugin;
impl Plugin for BodyPortalPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/body.wgsl");
        embedded_asset!(app, "shaders/body_prepass.wgsl");
        app.add_plugins(MaterialPlugin::<BodyMaterial>::default())
            .add_systems(PostStartup, clone_body)
            .add_systems(
                PostUpdate,
                update
                    .after(crate::body::BodyPoseUpdated)
                    .before(TransformSystems::Propagate),
            )
            .add_systems(
                PostUpdate,
                view_layers
                    .after(PortalViewsUpdated)
                    .before(TransformSystems::Propagate),
            );
    }
}

fn clone_body(world: &mut World) {
    let Ok(original) = world
        .query_filtered::<Entity, With<PlayerBody>>()
        .single(world)
    else {
        return;
    };
    let root = world
        .spawn((PortalGhost, Transform::default(), Visibility::Hidden))
        .id();
    let mut parents = HashMap::from([(original, root)]);
    let mut handles = HashMap::new();
    let mut copies = Vec::new();
    let mut pending = vec![original];
    while let Some(parent) = pending.pop() {
        let children: Vec<_> = world
            .get::<Children>(parent)
            .map_or_else(Vec::new, |c| c.iter().collect());
        for child in children {
            let transform = *world.get::<Transform>(child).unwrap();
            let mesh = world.get::<Mesh3d>(child).cloned();
            let bounds = mesh
                .as_ref()
                .and_then(|m| world.resource::<Assets<Mesh>>().get(&m.0))
                .and_then(MeshAabb::compute_aabb);
            let handle = world
                .get::<MeshMaterial3d<BodyMaterial>>(child)
                .map(|m| m.0.clone());
            let target = world
                .spawn((transform, Visibility::Inherited, ChildOf(parents[&parent])))
                .id();
            if let (Some(mesh), Some(handle)) = (mesh, handle) {
                let copy = handles
                    .entry(handle.id())
                    .or_insert_with(|| {
                        let mut materials = world.resource_mut::<Assets<BodyMaterial>>();
                        let mut copy = materials.get(&handle).unwrap().clone();
                        copy.extension.ghost = true;
                        materials.add(copy)
                    })
                    .clone();
                world.entity_mut(target).insert((
                    mesh,
                    MeshMaterial3d(copy),
                    RenderLayers::layer(GHOST_LAYER),
                ));
            }
            parents.insert(child, target);
            copies.push(CopyPart {
                source: child,
                target,
                parent,
                bounds,
            });
            pending.push(child);
        }
    }
    world.insert_resource(Ghost {
        original,
        root,
        copies,
    });
}

fn update(
    ghost: Option<Res<Ghost>>,
    portals: Option<Res<Portals>>,
    mut materials: ResMut<Assets<BodyMaterial>>,
    mut transforms: Query<&mut Transform>,
    mut visibility: Query<&mut Visibility>,
) {
    let Some(ghost) = ghost else { return };
    let root_matrix = transforms.get(ghost.original).unwrap().to_matrix();
    let mut matrices = HashMap::from([(ghost.original, root_matrix)]);
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for part in &ghost.copies {
        let matrix = matrices[&part.parent] * transforms.get(part.source).unwrap().to_matrix();
        matrices.insert(part.source, matrix);
        if let Some(bounds) = part.bounds {
            for x in [-1.0, 1.0] {
                for y in [-1.0, 1.0] {
                    for z in [-1.0, 1.0] {
                        let local = Vec3::from(bounds.center)
                            + Vec3::from(bounds.half_extents) * Vec3::new(x, y, z);
                        let p = matrix.transform_point3(local);
                        lo = lo.min(p);
                        hi = hi.max(p);
                    }
                }
            }
        }
    }
    let centre = Vec3::from(from_bevy((lo + hi) * 0.5));
    let half = Vec3::from(from_bevy((hi - lo) * 0.5)).abs();
    let halves = split(portals.and_then(|p| p.linked()), centre, half);
    *visibility.get_mut(ghost.root).unwrap() = if halves.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for (_, material) in materials.iter_mut() {
        material.extension.plane = halves.as_ref().map_or(Vec4::ZERO, |s| {
            if material.extension.ghost {
                s.exit
            } else {
                s.source
            }
        });
    }
    if let Some(halves) = halves {
        let original = transforms.get(ghost.original).unwrap().to_matrix();
        *transforms.get_mut(ghost.root).unwrap() =
            Transform::from_matrix(halves.transform * original);
        for part in &ghost.copies {
            let transform = *transforms.get(part.source).unwrap();
            *transforms.get_mut(part.target).unwrap() = transform;
        }
    }
}

// Translated from 0x54c4c0 (decompiled, client.so).
fn view_layers(
    visual: Res<VisualPlayer>,
    portals: Option<Res<Portals>>,
    mut views: Query<(&ParticleView, &mut RenderLayers)>,
) {
    let crossed = visual
        .camera_portal(portals.and_then(|p| p.linked()))
        .is_some();
    for (view, mut layers) in &mut views {
        let visible = match view.0 {
            32 | 33 => !crossed,
            35 | 36 => crossed,
            _ => true,
        };
        *layers = if visible {
            layers.clone().with(GHOST_LAYER)
        } else {
            layers.clone().without(GHOST_LAYER)
        };
    }
}
