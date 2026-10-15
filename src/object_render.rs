// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::bodyportal::{material, BodyMaterial, BodyPortalUpdated};
use crate::faces::Portals;
use crate::sound::Files;
use crate::units::{to_bevy, to_bevy_dir, METRES_PER_UNIT};

const MODEL: &str = "models/props/metal_box";

#[derive(Resource)]
struct CubeVisual {
    halves: Vec<(Entity, Handle<BodyMaterial>, bool)>,
    /// a blended twin per piece, drawn at zero alpha so its pipeline is ready for a dissolve
    fades: Vec<(Entity, Handle<BodyMaterial>)>,
    min: Vec3,
    max: Vec3,
}

pub struct CubeRenderPlugin;

impl Plugin for CubeRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CubePose>()
            .add_systems(Startup, spawn)
            .add_systems(
                PostUpdate,
                update
                    .after(BodyPortalUpdated)
                    .before(TransformSystems::Propagate),
            );
    }
}

#[derive(Resource)]
pub struct CubePose {
    pub position: [f32; 3],
    pub rotation: [[f32; 3]; 3],
}

impl Default for CubePose {
    fn default() -> Self {
        Self {
            position: [-220.0, -80.0, 20.1],
            rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }
}

pub fn pose_transform(pose: &CubePose) -> Transform {
    Transform {
        translation: to_bevy(pose.position),
        rotation: Quat::from_mat3(&Mat3::from_cols(
            to_bevy_dir(pose.rotation[0]),
            to_bevy_dir(pose.rotation[1]),
            to_bevy_dir(pose.rotation[2]),
        )),
        scale: Vec3::splat(METRES_PER_UNIT),
    }
}

fn bounds(mdl: &portal::mdl::Mdl) -> Result<(Vec3, Vec3), String> {
    let vector = |offset| -> Result<Vec3, String> {
        let data = mdl
            .bytes
            .get(offset..offset + 12)
            .ok_or("cube hull bounds truncated")?;
        let mut values = [0.0; 3];
        for (value, bytes) in values.iter_mut().zip(data.as_chunks::<4>().0) {
            *value = f32::from_le_bytes(*bytes);
        }
        Ok(Vec3::from(values))
    };
    let (min, max) = (vector(104)?, vector(116)?);
    if !min.is_finite() || !max.is_finite() || min.cmpge(max).any() {
        return Err(format!("invalid cube hull min={min:?} max={max:?}"));
    }
    Ok((min, max))
}

fn companion_material(mdl: &portal::mdl::Mdl, slot: usize) -> Result<usize, String> {
    mdl.skins
        .get(1)
        .and_then(|skin| skin.get(slot))
        .map(|&index| usize::from(index))
        .ok_or_else(|| format!("cube skin=1 material={slot} missing"))
}

#[allow(clippy::too_many_arguments)]
fn spawn(
    mut commands: Commands,
    files: Res<Files>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BodyMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut exit: MessageWriter<AppExit>,
) {
    let result = (|| -> Result<CubeVisual, String> {
        let (mdl, pieces) = crate::viewmodel::read_model(&files, MODEL)?;
        let (min, max) = bounds(&mdl)?;
        let mut halves = Vec::new();
        let mut fades = Vec::new();
        for piece in pieces {
            let index = companion_material(&mdl, piece.material)?;
            let look = crate::viewmodel::look(&files, &mdl, index)?;
            let texture = look
                .texture
                .as_ref()
                .ok_or("companion cube base texture missing")?;
            let texture = images.add(crate::texture::load(&files, texture)?);
            let skin = crate::viewmodel::skin(&piece);
            let mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, skin.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, skin.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, skin.uvs)
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_TANGENT,
                piece.vertices.iter().map(|v| v.tangent).collect::<Vec<_>>(),
            )
            .with_inserted_indices(Indices::U32(skin.indices));
            let mesh = meshes.add(mesh);
            for ghost in [false, true] {
                let mut surface = material(StandardMaterial {
                    base_color_texture: Some(texture.clone()),
                    ..default()
                });
                surface.base.alpha_mode = AlphaMode::Opaque;
                surface.extension.ghost = ghost;
                surface.extension.player_owned = false;
                crate::model_material::apply(&look, &files, &mut images, &mut surface)?;
                let surface = materials.add(surface);
                let entity = commands
                    .spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(surface.clone()),
                        Transform::default(),
                        Visibility::Hidden,
                    ))
                    .id();
                halves.push((entity, surface, ghost));
            }
            let mut surface = material(StandardMaterial {
                base_color: Color::linear_rgba(1.0, 1.0, 1.0, 0.0),
                base_color_texture: Some(texture.clone()),
                alpha_mode: AlphaMode::Blend,
                ..default()
            });
            surface.extension.player_owned = false;
            crate::model_material::apply(&look, &files, &mut images, &mut surface)?;
            let surface = materials.add(surface);
            let entity = commands
                .spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(surface.clone()),
                    Transform::default(),
                    Visibility::Hidden,
                ))
                .id();
            fades.push((entity, surface));
        }
        info!(
            "[cube] model={MODEL} skin=1 min={min:?} max={max:?} halves={}",
            halves.len()
        );
        Ok(CubeVisual {
            halves,
            fades,
            min,
            max,
        })
    })();
    match result {
        Ok(visual) => {
            commands.insert_resource(visual);
        }
        Err(reason) => {
            error!("[cube] load=failed reason={reason}");
            exit.write(AppExit::error());
        }
    }
}

fn update(
    pose: Res<CubePose>,
    visual: Option<Res<CubeVisual>>,
    portals: Option<Res<Portals>>,
    tint: Option<Res<crate::object_visual::CubeTint>>,
    mut materials: ResMut<Assets<BodyMaterial>>,
    mut halves: Query<(&mut Transform, &mut Visibility)>,
) {
    let Some(visual) = visual else { return };
    let tint = tint.map_or(Some([255; 4]), |t| t.0);
    let fading = tint.map(|[.., a]| a < 255);
    let linear = |c: [u8; 4]| {
        Color::linear_rgba(
            c[0] as f32 / 255.0,
            c[1] as f32 / 255.0,
            c[2] as f32 / 255.0,
            c[3] as f32 / 255.0,
        )
    };
    let transform = pose_transform(&pose);
    let rotation = Mat3::from_cols_array_2d(&pose.rotation);
    let centre = Vec3::from(pose.position) + rotation * ((visual.min + visual.max) * 0.5);
    let half = rotation.abs() * ((visual.max - visual.min) * 0.5);
    let split = crate::bodyportal::split(
        portals.as_ref().and_then(|portals| portals.linked()),
        Vec3::from(pose.position),
        centre,
        half,
    );
    for (entity, handle, ghost) in &visual.halves {
        let Ok((mut at, mut visibility)) = halves.get_mut(*entity) else {
            continue;
        };
        *visibility = if fading.is_some_and(|f| !f) && (!ghost || split.is_some()) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        *at = if *ghost {
            split.as_ref().map_or(transform, |split| {
                Transform::from_matrix(split.transform * transform.to_matrix())
            })
        } else {
            transform
        };
        if let (Some(mut surface), Some([r, g, b, _])) = (materials.get_mut(handle), tint) {
            surface.base.base_color = linear([r, g, b, 255]);
            surface.extension.plane =
                split.as_ref().map_or(
                    Vec4::ZERO,
                    |split| if *ghost { split.exit } else { split.source },
                );
        }
    }
    for (entity, handle) in &visual.fades {
        if let Ok((mut at, mut visibility)) = halves.get_mut(*entity) {
            *at = transform;
            *visibility = if tint.is_some() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        if let Some(mut surface) = materials.get_mut(handle) {
            let [r, g, b, a] = tint.unwrap_or([255; 4]);
            surface.base.base_color = linear([r, g, b, if fading == Some(true) { a } else { 0 }]);
            surface.extension.plane = split.as_ref().map_or(Vec4::ZERO, |split| split.source);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::to_bevy;

    #[test]
    fn cube_pose_maps_source_basis_and_origin() {
        for rotation in [
            [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
            [[0., 1., 0.], [0., 0., 1.], [1., 0., 0.]],
            [[0., 0., -1.], [0., 1., 0.], [1., 0., 0.]],
        ] {
            let pose = CubePose {
                position: [100., -32., 74.],
                rotation,
            };
            let transform = pose_transform(&pose);
            for point in [[0., 0., 0.], [20., -20., 20.], [-20., 20., -20.]] {
                let source = Vec3::from(pose.position)
                    + Vec3::from(rotation[0]) * point[0]
                    + Vec3::from(rotation[1]) * point[1]
                    + Vec3::from(rotation[2]) * point[2];
                assert!(
                    transform
                        .transform_point(Vec3::from(point))
                        .distance(to_bevy(source.to_array()))
                        < 0.00001
                );
            }
        }
    }
    #[test]
    fn rotated_cube_halves_keep_matching_points_for_every_exit() {
        use portal::pair::{through_point, Portal};
        let entry = Portal::new([0., 0., 0.], [0., 0., 1.], [1., 0., 0.], (32., 54.));
        for (normal, up) in [
            ([1., 0., 0.], [0., 0., 1.]),
            ([0., 0., 1.], [1., 0., 0.]),
            ([0., 0., -1.], [1., 0., 0.]),
        ] {
            let exit = Portal::new([200., 100., 300.], normal, up, (32., 54.));
            for depth in [19., 1., -1., -4.99] {
                let pose = CubePose {
                    position: [0., 0., depth],
                    rotation: [[0., 1., 0.], [0., 0., 1.], [1., 0., 0.]],
                };
                let model = pose_transform(&pose).to_matrix();
                let split = crate::bodyportal::split(
                    Some([entry, exit]),
                    Vec3::from(pose.position),
                    Vec3::from(pose.position),
                    Vec3::splat(20.25),
                )
                .expect("cube intersects entry");
                let point = Vec3::new(20., -20., 20.);
                let source = model.transform_point3(point);
                let mapped = split.transform.transform_point3(source);
                let expected = to_bevy(through_point(
                    &entry,
                    &exit,
                    crate::units::from_bevy(source),
                ));
                assert!(mapped.distance(expected) < 0.00001);
                let source_plane = split.source.dot(source.extend(1.));
                let exit_plane = split.exit.dot(mapped.extend(1.));
                assert!(
                    (source_plane + exit_plane - 1.05 * crate::units::METRES_PER_UNIT).abs()
                        < 0.00001
                );
            }
        }
    }
    #[test]
    fn companion_uses_second_skin_and_original_model_bounds() {
        let root = portal::install::root(
            None,
            None,
            std::path::Path::new(&std::env::var("HOME").unwrap()),
        );
        let install = portal::install::Install::open(&root).unwrap();
        let (mdl, meshes) = crate::viewmodel::read_model(&install.search, MODEL).unwrap();
        assert_eq!(mdl.version, 44);
        assert_eq!(mdl.sequences.len(), 1);
        assert_eq!(mdl.sequences[0].label, "idle");
        assert_eq!(
            bounds(&mdl).unwrap(),
            (
                Vec3::new(-20.250004, -20.25, -20.25),
                Vec3::new(20.250004, 20.25, 20.25)
            )
        );
        assert!(!meshes.is_empty());
        for piece in meshes {
            let index = companion_material(&mdl, piece.material).unwrap();
            assert_eq!(mdl.textures[index], "metal_box_skin001");
            let look = crate::viewmodel::look(&install.search, &mdl, index).unwrap();
            assert_eq!(
                look.texture.unwrap().replace('\\', "/"),
                "models/props/metal_box_skin001"
            );
        }
        assert!(companion_material(&mdl, 2).is_err());
    }

    #[test]
    fn portal_removal_restores_whole_cube_and_hides_ghost() {
        use portal::pair::Portal;
        let mut app = App::new();
        app.init_resource::<Assets<BodyMaterial>>()
            .insert_resource(CubePose {
                position: [0., 0., 1.],
                ..default()
            })
            .add_systems(Update, update);
        let source = app
            .world_mut()
            .spawn((Transform::default(), Visibility::Hidden))
            .id();
        let ghost = app
            .world_mut()
            .spawn((Transform::default(), Visibility::Hidden))
            .id();
        let mut materials = app.world_mut().resource_mut::<Assets<BodyMaterial>>();
        let original_material = materials.add(material(StandardMaterial::default()));
        let ghost_material = materials.add(material(StandardMaterial::default()));
        let other_material = materials.add(material(StandardMaterial::default()));
        materials.get_mut(&other_material).unwrap().extension.plane = Vec4::W;
        app.insert_resource(CubeVisual {
            halves: vec![
                (source, original_material.clone(), false),
                (ghost, ghost_material.clone(), true),
            ],
            fades: Vec::new(),
            min: Vec3::splat(-20.25),
            max: Vec3::splat(20.25),
        });
        let pair = Portals(crate::portals::Pair {
            open: [
                Some(Portal::new([0.; 3], [0., 0., 1.], [1., 0., 0.], (32., 54.))),
                Some(Portal::new(
                    [200., 0., 300.],
                    [0., 0., -1.],
                    [1., 0., 0.],
                    (32., 54.),
                )),
            ],
            ..default()
        });
        app.insert_resource(pair);
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(ghost).unwrap(),
            Visibility::Inherited
        );
        assert_ne!(
            app.world()
                .resource::<Assets<BodyMaterial>>()
                .get(&original_material)
                .unwrap()
                .extension
                .plane,
            Vec4::ZERO
        );
        app.world_mut().resource_mut::<Portals>().open[1] = None;
        app.update();
        assert_eq!(
            *app.world().get::<Visibility>(source).unwrap(),
            Visibility::Inherited
        );
        assert_eq!(
            *app.world().get::<Visibility>(ghost).unwrap(),
            Visibility::Hidden
        );
        let materials = app.world().resource::<Assets<BodyMaterial>>();
        for handle in [original_material, ghost_material] {
            assert_eq!(materials.get(&handle).unwrap().extension.plane, Vec4::ZERO);
        }
        assert_eq!(
            materials.get(&other_material).unwrap().extension.plane,
            Vec4::W
        );
    }

    #[test]
    fn a_fading_cube_draws_its_blended_twin_and_hides_the_opaque_piece() {
        let mut app = App::new();
        app.init_resource::<Assets<BodyMaterial>>()
            .init_resource::<CubePose>()
            .insert_resource(crate::object_visual::CubeTint(Some([255; 4])))
            .add_systems(Update, update);
        let piece = app
            .world_mut()
            .spawn((Transform::default(), Visibility::Hidden))
            .id();
        let twin = app
            .world_mut()
            .spawn((Transform::default(), Visibility::Hidden))
            .id();
        let mut materials = app.world_mut().resource_mut::<Assets<BodyMaterial>>();
        let solid = materials.add(material(StandardMaterial::default()));
        let blended = materials.add(material(StandardMaterial::default()));
        app.insert_resource(CubeVisual {
            halves: vec![(piece, solid.clone(), false)],
            fades: vec![(twin, blended.clone())],
            min: Vec3::splat(-20.25),
            max: Vec3::splat(20.25),
        });
        let seen = |app: &mut App| {
            app.update();
            let w = app.world();
            let colour = |h: &Handle<BodyMaterial>| {
                w.resource::<Assets<BodyMaterial>>()
                    .get(h)
                    .unwrap()
                    .base
                    .base_color
                    .to_linear()
                    .to_f32_array()
            };
            (
                *w.get::<Visibility>(piece).unwrap(),
                *w.get::<Visibility>(twin).unwrap(),
                colour(&solid),
                colour(&blended),
            )
        };
        assert_eq!(
            seen(&mut app),
            (
                Visibility::Inherited,
                Visibility::Inherited,
                [1.0; 4],
                [1.0, 1.0, 1.0, 0.0]
            )
        );
        app.world_mut()
            .resource_mut::<crate::object_visual::CubeTint>()
            .0 = Some([0, 0, 0, 51]);
        assert_eq!(
            seen(&mut app),
            (
                Visibility::Hidden,
                Visibility::Inherited,
                [0.0, 0.0, 0.0, 1.0],
                [0.0, 0.0, 0.0, 0.2]
            )
        );
        app.world_mut()
            .resource_mut::<crate::object_visual::CubeTint>()
            .0 = None;
        let (piece_seen, twin_seen, _, _) = seen(&mut app);
        assert_eq!(
            (piece_seen, twin_seen),
            (Visibility::Hidden, Visibility::Hidden)
        );
    }
}
