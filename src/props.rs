// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::collections::HashMap;
use std::sync::Arc;

use bevy::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::prelude::*;
use portal::anim::sample;
use portal::entity::io::Handle as EntityHandle;
use portal::mdl::Mdl;
use portal::model::MeshData;
use portal::pair::Vec3 as Source;
use portal::viewbob::{angle_vectors, Angles};

use crate::entities::Entities;
use crate::scene::Level;
use crate::sound::Files;
use crate::units::{to_bevy, to_bevy_dir, METRES_PER_UNIT};
use crate::viewmodel::{bindposes, look, read_model, Blend};

/// A studio model the chamber places, prop_dynamic or prop_static.
#[derive(Component)]
struct Prop {
    entity: EntityHandle,
    mdl: Arc<Mdl>,
    joints: Vec<Entity>,
    origin: Source,
    /// the pusher it rides and where that pusher started
    parent: Option<(EntityHandle, Source)>,
}

fn angles(text: Option<&str>) -> Angles {
    let mut v = text
        .unwrap_or_default()
        .split_whitespace()
        .map(|p| p.parse().unwrap_or(0.0));
    Angles {
        pitch: v.next().unwrap_or(0.0),
        yaw: v.next().unwrap_or(0.0),
        roll: v.next().unwrap_or(0.0),
    }
}

fn root(origin: Source, a: Angles) -> Transform {
    let (forward, right, up) = angle_vectors(a);
    Transform {
        translation: to_bevy(origin),
        rotation: Quat::from_mat3(&Mat3::from_cols(
            to_bevy_dir(forward),
            to_bevy_dir(right.map(|v| -v)),
            to_bevy_dir(up),
        )),
        scale: Vec3::splat(METRES_PER_UNIT),
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn(
    mut commands: Commands,
    level: Res<Level>,
    entities: Res<Entities>,
    files: Res<Files>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<crate::bodyportal::BodyMaterial>>,
    mut binds: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    mut exit: MessageWriter<AppExit>,
) {
    let mut models: HashMap<String, (Arc<Mdl>, Vec<MeshData>)> = HashMap::new();
    let mut count = 0;
    for (index, e) in level.0.entities.iter().enumerate() {
        if !matches!(e.classname.as_str(), "prop_dynamic" | "prop_static") {
            continue;
        }
        let name = e
            .get("model")
            .unwrap_or_default()
            .trim_end_matches(".mdl")
            .to_string();
        if !models.contains_key(&name) {
            match read_model(&files, &name) {
                Ok((mdl, pieces)) => models.insert(name.clone(), (Arc::new(mdl), pieces)),
                Err(reason) => {
                    error!("[prop] refused: model={name} reason={reason}");
                    exit.write(AppExit::error());
                    return;
                }
            };
        }
        let (mdl, pieces) = &models[&name];
        let handle = EntityHandle(index as u32);
        let origin = e.origin().unwrap_or_default();
        let parent = e
            .get("parentname")
            .and_then(|p| entities.world.find(p))
            .map(|p| (p, entities.world.origin(p)));
        let skin = e
            .get("skin")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        let prop_root = commands
            .spawn((root(origin, angles(e.get("angles"))), Visibility::Inherited))
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
                .insert(ChildOf(b.parent.map_or(prop_root, |p| joints[p])));
        }
        let bind = binds.add(SkinnedMeshInverseBindposes::from(
            bindposes(mdl)
                .iter()
                .map(Mat4::from_cols_array)
                .collect::<Vec<_>>(),
        ));
        for piece in pieces {
            let slot = mdl
                .skins
                .get(skin)
                .and_then(|f| f.get(piece.material))
                .map_or(piece.material, |&i| i as usize);
            let material = match prop_material(&files, mdl, slot, &mut images) {
                Ok(m) => materials.add(m),
                Err(reason) => {
                    error!("[prop] refused: model={name} material={slot} reason={reason}");
                    exit.write(AppExit::error());
                    return;
                }
            };
            commands.spawn((
                Mesh3d(meshes.add(crate::view::gun_mesh(piece))),
                MeshMaterial3d(material),
                SkinnedMesh {
                    inverse_bindposes: bind.clone(),
                    joints: joints.clone(),
                },
                ChildOf(prop_root),
            ));
        }
        commands.entity(prop_root).insert(Prop {
            entity: handle,
            mdl: mdl.clone(),
            joints,
            origin,
            parent,
        });
        count += 1;
    }
    info!("[prop] spawned props={count} models={}", models.len());
}

fn prop_material(
    files: &Files,
    mdl: &Mdl,
    slot: usize,
    images: &mut Assets<Image>,
) -> Result<crate::bodyportal::BodyMaterial, String> {
    let l = look(files, mdl, slot)?;
    let texture = l.texture.as_ref().ok_or("base texture missing")?;
    let image = images.add(crate::texture::load(files, texture)?);
    let mut material = crate::bodyportal::material(StandardMaterial {
        base_color_texture: Some(image),
        alpha_mode: match l.blend {
            Blend::Opaque => AlphaMode::Opaque,
            Blend::Blend => AlphaMode::Blend,
            Blend::Add => AlphaMode::Add,
        },
        ..default()
    });
    material.extension.player_owned = false;
    crate::model_material::apply(&l, files, images, &mut material)?;
    Ok(material)
}

/// Rides each prop on its pusher, shows or hides it and poses it at its sequence and cycle.
fn update(
    entities: Res<Entities>,
    mut props: Query<(&Prop, &mut Transform, &mut Visibility)>,
    mut joints: Query<&mut Transform, Without<Prop>>,
) {
    for (prop, mut transform, mut visibility) in &mut props {
        let shift = prop.parent.map_or([0.0; 3], |(p, start)| {
            let now = entities.world.origin(p);
            [0, 1, 2].map(|i| now[i] - start[i])
        });
        transform.translation = to_bevy([0, 1, 2].map(|i| prop.origin[i] + shift[i]));
        *visibility = if entities.world.visible(prop.entity) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let Some((seq, cycle)) = entities.world.animation(prop.entity) else {
            continue;
        };
        let Some(s) = prop.mdl.sequences.get(seq) else {
            continue;
        };
        let frames = prop.mdl.anims.get(s.anim).map_or(1, |a| a.frames.max(1));
        let Ok(pose) = sample(&prop.mdl, s.anim, cycle * (frames - 1) as f32) else {
            continue;
        };
        for (joint, p) in prop.joints.iter().zip(pose) {
            if let Ok(mut t) = joints.get_mut(*joint) {
                t.translation = Vec3::from(p.pos);
                t.rotation = Quat::from_array(p.quat);
            }
        }
    }
}

pub struct PropsPlugin;

impl Plugin for PropsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn.run_if(resource_exists::<Entities>))
            .add_systems(
                PostUpdate,
                update
                    .before(TransformSystems::Propagate)
                    .run_if(resource_exists::<Entities>),
            );
    }
}
