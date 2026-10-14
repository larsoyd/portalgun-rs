// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::collections::BTreeMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use bevy::camera::visibility::RenderLayers;

use crate::chamber::{Chamber, Side};
use crate::sound::Files;
use crate::texture::{self, WorldLook};
use crate::units::{to_bevy, to_bevy_dir};
use crate::view::VIEW_LAYER;

#[derive(Resource, Deref)]
pub struct Level(pub Chamber);

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GlobalAmbientLight {
            brightness: 400.0,
            ..default()
        })
        .add_systems(Startup, spawn_chamber);
    }
}

/// every visible face drawn in one material, fanned into triangles
fn material_mesh(faces: &[&Side], size: UVec2) -> Mesh {
    let size = Vec2::new(size.x as f32, size.y as f32);
    let (mut positions, mut normals, mut uvs, mut indices) = (vec![], vec![], vec![], vec![]);
    for f in faces {
        let base = positions.len() as u32;
        positions.extend(f.points.iter().map(|&p| to_bevy(p).to_array()));
        normals.extend(vec![to_bevy_dir(f.plane.normal).to_array(); f.points.len()]);
        uvs.extend(f.uv.iter().map(|[u, v]| [u / size.x, v / size.y]));
        for i in 1..f.points.len() as u32 - 1 {
            indices.extend([base, base + i, base + i + 1]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

fn spawn_chamber(
    mut commands: Commands,
    level: Res<Level>,
    files: Res<Files>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut exit: MessageWriter<AppExit>,
) {
    let mut by_material: BTreeMap<&str, Vec<&Side>> = BTreeMap::new();
    for side in level.blocks.iter().flat_map(|b| b.faces()) {
        by_material.entry(&side.material).or_default().push(side);
    }
    for (name, faces) in &by_material {
        let look = match texture::world_look(&files, name) {
            Ok(l) => l,
            Err(e) => {
                error!("[chamber] refused: material={name} reason={e}");
                exit.write(AppExit::error());
                return;
            }
        };
        let (material, size) = match &look {
            WorldLook::Refract(tint) => (
                StandardMaterial {
                    base_color: Color::srgb(tint[0], tint[1], tint[2]),
                    alpha_mode: AlphaMode::Multiply,
                    unlit: true,
                    ..default()
                },
                UVec2::ONE,
            ),
            WorldLook::Opaque(base) | WorldLook::Translucent(base) => {
                let image = match texture::load(&files, base) {
                    Ok(i) => i,
                    Err(e) => {
                        error!("[chamber] refused: material={name} reason={e}");
                        exit.write(AppExit::error());
                        return;
                    }
                };
                let size = image.size();
                let blend = matches!(look, WorldLook::Translucent(_));
                let material = StandardMaterial {
                    base_color_texture: Some(images.add(image)),
                    perceptual_roughness: 0.9,
                    alpha_mode: if blend {
                        AlphaMode::Blend
                    } else {
                        AlphaMode::Opaque
                    },
                    double_sided: blend,
                    cull_mode: if blend {
                        None
                    } else {
                        Some(bevy::render::render_resource::Face::Back)
                    },
                    ..default()
                };
                (material, size)
            }
        };
        let material = materials.add(material);
        commands.spawn((
            Mesh3d(meshes.add(material_mesh(faces, size))),
            MeshMaterial3d(material),
        ));
    }
    for at in [[-192.0, 0.0, 360.0], [192.0, 0.0, 360.0]] {
        commands.spawn((
            PointLight {
                intensity: 2_000_000.0,
                range: 40.0,
                shadow_maps_enabled: true,
                ..default()
            },
            Transform::from_translation(to_bevy(at)),
            RenderLayers::from_layers(&[
                0,
                VIEW_LAYER,
                crate::body::BODY_LAYER,
                crate::bodyportal::GHOST_LAYER,
            ]),
        ));
    }
    info!(
        "[chamber] blocks={} materials={}",
        level.blocks.len(),
        by_material.len()
    );
}
