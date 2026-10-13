// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

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
    pub portal: usize,
    pub source: Vec4,
    pub exit: Vec4,
    pub transform: Mat4,
}

// Translated from 0x5562a0 (decompiled, client.so), using capsule rig bounds.
pub fn split(
    pair: Option<[Portal; 2]>,
    entity_centre: Vec3,
    centre: Vec3,
    half: Vec3,
) -> Option<Split> {
    split_where(pair, entity_centre, centre, half, |_| true)
}

fn split_where(
    pair: Option<[Portal; 2]>,
    entity_centre: Vec3,
    centre: Vec3,
    half: Vec3,
    admit: impl Fn(&Portal) -> bool,
) -> Option<Split> {
    let pair = pair?;
    let i = pair.iter().position(|p| {
        p.local(entity_centre.to_array())[0] >= -5.0
            && portal::teleport::in_hole(p, centre.to_array(), half.to_array())
            && admit(p)
    })?;
    let plane = |p: &Portal, bias| {
        let normal = to_bevy_dir(p.forward);
        normal.extend(-normal.dot(to_bevy(p.origin)) + bias * METRES_PER_UNIT)
    };
    Some(Split {
        portal: i,
        source: plane(&pair[i], 0.3),
        exit: plane(&pair[1 - i], 0.75),
        transform: crate::views::through(&pair[i], &pair[1 - i]),
    })
}

// The capsule rig can dip below its supporting floor without entering the portal.
pub fn supported_floor(portal: &Portal, centre: Vec3, half: Vec3, grounded: bool) -> bool {
    grounded
        && portal.forward[2] > portal::teleport::FLOORISH
        && portal.local(centre.to_array())[0] - Vec3::from(portal.forward).abs().dot(half)
            >= -crate::collision::DIST_EPSILON
}

#[derive(Default)]
struct BodyReport {
    state: Option<(Option<usize>, Option<usize>, u64, u64)>,
    next: f32,
    faults: crate::diagnostics::Faults,
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

#[allow(clippy::too_many_arguments)]
fn update(
    ghost: Option<Res<Ghost>>,
    player: Res<crate::player::Player>,
    time: Res<Time>,
    mut report: Local<BodyReport>,
    visual: Res<VisualPlayer>,
    portals: Option<Res<Portals>>,
    mut materials: ResMut<Assets<BodyMaterial>>,
    mut transforms: Query<&mut Transform>,
    mut visibility: Query<&mut Visibility>,
) {
    let Some(ghost) = ghost else { return };
    let missing = [ghost.original, ghost.root]
        .into_iter()
        .chain(ghost.copies.iter().flat_map(|p| [p.source, p.target]))
        .find(|e| !transforms.contains(*e))
        .or_else(|| (!visibility.contains(ghost.root)).then_some(ghost.root));
    if report.faults.raised("entities", missing.is_some()) {
        error!(
            "[body-portal] update=failed reason=missing_component entity={missing:?} root={:?} copies={}",
            ghost.original,
            ghost.copies.len()
        );
    }
    if missing.is_some() {
        if let Ok(mut visible) = visibility.get_mut(ghost.root) {
            *visible = Visibility::Hidden;
        }
        for (_, material) in materials.iter_mut() {
            material.extension.plane = Vec4::ZERO;
        }
        return;
    }
    let root_matrix = transforms.get(ghost.original).unwrap().to_matrix();
    let mut nonfinite = !root_matrix.is_finite();
    let mut matrices = HashMap::from([(ghost.original, root_matrix)]);
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for part in &ghost.copies {
        let matrix = matrices[&part.parent] * transforms.get(part.source).unwrap().to_matrix();
        nonfinite |= !matrix.is_finite();
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
    let invalid = nonfinite
        || !lo.is_finite()
        || !hi.is_finite()
        || !visual.pose.centre.is_finite()
        || !visual.pose.half.is_finite();
    if report.faults.raised("bounds", invalid) {
        error!("[body-portal] update=failed reason=invalid_bounds nonfinite_transform={nonfinite} min={lo:?} max={hi:?} hull={:?} revision={}", visual.pose.centre, visual.revision);
    }
    if invalid {
        if let Ok(mut visible) = visibility.get_mut(ghost.root) {
            *visible = Visibility::Hidden;
        }
        for (_, material) in materials.iter_mut() {
            material.extension.plane = Vec4::ZERO;
        }
        return;
    }
    let centre = Vec3::from(from_bevy((lo + hi) * 0.5));
    let half = Vec3::from(from_bevy((hi - lo) * 0.5)).abs();
    let pair = portals.as_ref().and_then(|p| p.linked());
    let candidate = split(pair, visual.pose.centre, centre, half);
    let candidate = candidate.as_ref().map(|s| s.portal);
    let supported =
        |p: &Portal| supported_floor(p, visual.pose.centre, visual.pose.half, player.grounded);
    let halves = split_where(pair, visual.pose.centre, centre, half, |p| !supported(p));
    let selected = halves.as_ref().map(|s| s.portal);
    let supported = candidate.is_some_and(|i| supported(&pair.unwrap()[i]));
    let state = (
        candidate,
        selected,
        visual.revision,
        portals.as_ref().map_or(0, |p| p.changes),
    );
    let now = time.elapsed_secs();
    if report.state != Some(state) || candidate.is_some() && now >= report.next {
        report.state = Some(state);
        report.next = now + 0.5;
        let gap = |half: Vec3, centre: Vec3| {
            candidate.map(|i| {
                let p = &pair.unwrap()[i];
                p.local(centre.to_array())[0] - Vec3::from(p.forward).abs().dot(half)
            })
        };
        let hull_gap = gap(visual.pose.half, visual.pose.centre);
        let render_gap = gap(half, centre);
        debug!("[body-portal] at={now:.3} candidate={candidate:?} selected={selected:?} hull_gap={hull_gap:?} render_gap={render_gap:?} ghost={} supported_floor={supported} grounded={} ducked={} feet={:?} hull_centre={:?} hull_half={:?} render_centre={centre:?} render_half={half:?} camera_portal={:?} revision={} pair={pair:?} planes_bevy={:?}",
            halves.is_some(), player.grounded, player.duck.ducked, player.feet, visual.pose.centre, visual.pose.half, visual.camera_portal(pair), visual.revision, halves.as_ref().map(|s| (s.source, s.exit)));
    }
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
