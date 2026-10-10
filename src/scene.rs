use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use bevy::camera::visibility::RenderLayers;

use crate::chamber::{Block, Chamber};
use crate::sound::Files;
use crate::texture;
use crate::units::{to_bevy, to_bevy_dir};
use crate::view::VIEW_LAYER;

/// Hammer's default, a texel is a quarter unit
const TEXEL_UNITS: f32 = 0.25;

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

fn block_mesh(block: &Block, size: UVec2) -> Mesh {
    let faces = block.faces();
    let scale = Vec2::new(size.x as f32, size.y as f32) * TEXEL_UNITS;
    let (mut positions, mut normals, mut uvs, mut indices) = (vec![], vec![], vec![], vec![]);
    for f in &faces {
        let base = positions.len() as u32;
        positions.extend(f.corners.map(|p| to_bevy(p).to_array()));
        normals.extend([to_bevy_dir(f.normal).to_array(); 4]);
        uvs.extend(f.uv.map(|[u, v]| [u / scale.x, v / scale.y]));
        indices.extend([0, 1, 2, 0, 2, 3].map(|i| base + i));
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
    let mut looks: HashMap<&str, (Handle<StandardMaterial>, UVec2)> = HashMap::new();
    for block in &level.blocks {
        if !looks.contains_key(block.material) {
            let image = match texture::base_texture(&files, block.material)
                .and_then(|name| texture::load(&files, &name))
            {
                Ok(i) => i,
                Err(e) => {
                    error!("[chamber] refused: material={} reason={e}", block.material);
                    exit.write(AppExit::error());
                    return;
                }
            };
            let size = image.size();
            let material = materials.add(StandardMaterial {
                base_color_texture: Some(images.add(image)),
                perceptual_roughness: 0.9,
                ..default()
            });
            looks.insert(block.material, (material, size));
        }
        let (material, size) = &looks[block.material];
        commands.spawn((
            Mesh3d(meshes.add(block_mesh(block, *size))),
            MeshMaterial3d(material.clone()),
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
            RenderLayers::from_layers(&[0, VIEW_LAYER]),
        ));
    }
    info!(
        "[chamber] blocks={} materials={}",
        level.blocks.len(),
        looks.len()
    );
}
