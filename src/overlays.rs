// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use portal::entity::io::Handle as EntityHandle;
use portal::overlay::{Overlay, Vertex};

use crate::bodyportal::BodyMaterial;
use crate::chamber::Chamber;
use crate::entities::Entities;
use crate::scene::Level;
use crate::sound::Files;
use crate::units::{to_bevy, to_bevy_dir};

/// An info_overlay cut to its sides, with the entity whose texture frame it shows.
#[derive(Debug)]
pub struct Built {
    pub entity: usize,
    pub material: String,
    pub polys: Vec<Vec<Vertex>>,
}

pub fn build(chamber: &Chamber) -> Result<Vec<Built>, String> {
    let sides: Vec<_> = chamber.blocks.iter().flat_map(|b| b.faces()).collect();
    let mut built = Vec::new();
    for (entity, e) in chamber.entities.iter().enumerate() {
        if e.classname != "info_overlay" {
            continue;
        }
        let overlay = Overlay::from_keys(e).map_err(|err| format!("overlay {entity}: {err}"))?;
        let mut polys = Vec::new();
        for id in &overlay.faces {
            let side = sides
                .iter()
                .find(|s| s.id == *id)
                .ok_or_else(|| format!("overlay {entity} names side {id}, not in the chamber"))?;
            let plane = (side.plane.normal, side.plane.distance);
            polys.extend(
                overlay
                    .fragments(&side.points, plane)
                    .map_err(|err| format!("overlay {entity}: {err}"))?,
            );
        }
        let material = e.get("material").unwrap_or_default().to_ascii_lowercase();
        built.push(Built {
            entity,
            material,
            polys,
        });
    }
    Ok(built)
}

/// Translated from 0x58edb0 (decompiled, client.so), the frame the toggle proxy picks, always wrapping
pub fn frame(index: u8, frames: usize) -> usize {
    index as usize % frames.max(1)
}

#[derive(Component)]
struct Frames {
    entity: EntityHandle,
    materials: Vec<Handle<BodyMaterial>>,
    shown: usize,
}

fn overlay_mesh(polys: &[Vec<Vertex>]) -> Mesh {
    let (mut positions, mut normals, mut uvs, mut indices) = (vec![], vec![], vec![], vec![]);
    for p in polys {
        let base = positions.len() as u32;
        positions.extend(p.iter().map(|v| to_bevy(v.pos).to_array()));
        normals.extend(p.iter().map(|v| to_bevy_dir(v.normal).to_array()));
        uvs.extend(p.iter().map(|v| v.uv));
        // the engine winds fragments clockwise for its front faces, bevy wants the other way
        for i in 1..p.len() as u32 - 1 {
            indices.extend([base, base + i + 1, base + i]);
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

fn frame_materials(
    files: &Files,
    material: &str,
    images: &mut Assets<Image>,
    materials: &mut Assets<BodyMaterial>,
) -> Result<Vec<Handle<BodyMaterial>>, String> {
    let path = format!("materials/{material}.vmt");
    let bytes = files.read(&path).map_err(|e| format!("{path}: {e}"))?;
    let root = portal::keyvalues::parse(&String::from_utf8_lossy(&bytes))
        .map_err(|e| format!("{path}: {e}"))?;
    let shader = root.first().ok_or_else(|| format!("{path}: empty"))?;
    let look = crate::model_material::Phong::parse(shader)?;
    let base = crate::texture::base_texture(files, material)?;
    (0..crate::texture::frames(files, &base)?)
        .map(|f| {
            let image = images.add(crate::texture::load_frame(files, &base, f)?);
            let mut m = crate::bodyportal::material(StandardMaterial {
                base_color_texture: Some(image),
                perceptual_roughness: 0.9,
                ..default()
            });
            m.extension.player_owned = false;
            m.extension.model_flags = Vec4::new(0.0, 0.0, look.self_illum as u8 as f32, 0.0);
            Ok(materials.add(m))
        })
        .collect()
}

fn spawn(
    mut commands: Commands,
    level: Res<Level>,
    files: Res<Files>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<BodyMaterial>>,
    mut exit: MessageWriter<AppExit>,
) {
    let result = build(&level.0).and_then(|built| {
        for b in &built {
            let frames = frame_materials(&files, &b.material, &mut images, &mut materials)
                .map_err(|e| format!("overlay {} material {}: {e}", b.entity, b.material))?;
            commands.spawn((
                Mesh3d(meshes.add(overlay_mesh(&b.polys))),
                MeshMaterial3d(frames[0].clone()),
                Frames {
                    entity: EntityHandle(b.entity as u32),
                    materials: frames,
                    shown: 0,
                },
            ));
            info!(
                "[overlay] entity={} material={} fragments={}",
                b.entity,
                b.material,
                b.polys.len()
            );
        }
        Ok(built.len())
    });
    if let Err(reason) = result {
        error!("[overlay] refused: {reason}");
        exit.write(AppExit::error());
    }
}

fn show_frames(
    entities: Res<Entities>,
    mut overlays: Query<(&mut Frames, &mut MeshMaterial3d<BodyMaterial>)>,
) {
    for (mut f, mut material) in &mut overlays {
        let wanted = frame(entities.world.frame(f.entity), f.materials.len());
        if wanted != f.shown {
            f.shown = wanted;
            material.0 = f.materials[wanted].clone();
            info!("[overlay] entity={} frame={wanted}", f.entity.0);
        }
    }
}

pub struct OverlaysPlugin;

impl Plugin for OverlaysPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn.run_if(resource_exists::<Entities>))
            .add_systems(Update, show_frames.run_if(resource_exists::<Entities>));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_triangles_face_along_their_normal() {
        let v = |x: f32, y: f32| Vertex {
            pos: [x, y, 0.1],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0; 2],
            st: [0.0; 2],
        };
        let mesh = overlay_mesh(&[vec![v(-1.0, -1.0), v(-1.0, 1.0), v(1.0, 1.0), v(1.0, -1.0)]]);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("no positions")
        };
        let Some(Indices::U32(i)) = mesh.indices() else {
            panic!("no indices")
        };
        let up = to_bevy_dir([0.0, 0.0, 1.0]);
        for t in i.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|k| Vec3::from(p[t[k] as usize]));
            assert!(
                (b - a).cross(c - a).dot(up) > 0.0,
                "triangle {t:?} faces away"
            );
        }
    }
}
