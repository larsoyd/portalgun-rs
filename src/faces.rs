// the bind group derive leaves checks the compiler calls unused
#![allow(dead_code)]

use bevy::asset::{embedded_asset, RenderAssetUsages};
use bevy::image::{ImageAddressMode, ImageSampler};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::{load_shader_library, ShaderRef};
use portal::portal_face::shader_time;

use crate::portals::{face_quad, overlay_look, Colour, Pair};
use crate::sound::Files;
use crate::texture;
use crate::units::{to_bevy, to_bevy_dir};

/// TEXTUREFLAGS_CLAMPS and TEXTUREFLAGS_CLAMPT
const CLAMP_S: u32 = 0x4;
const CLAMP_T: u32 = 0x8;

#[derive(Resource, Default, Deref, DerefMut)]
pub struct Portals(pub Pair);

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct PortalMaterial {
    #[uniform(0)]
    pub params: PortalParams,
    #[texture(1)]
    #[sampler(2)]
    pub noise: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    pub ramp: Handle<Image>,
}

#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct PortalParams {
    open: f32,
    keep: f32,
    scale: f32,
    time: f32,
}

impl Material for PortalMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://portalgun/shaders/portal.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Premultiplied
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }
}

#[derive(Resource)]
struct Faces {
    entity: [Entity; 2],
    material: [Handle<PortalMaterial>; 2],
    mesh: [Handle<Mesh>; 2],
    seen: u64,
}

pub struct FacesPlugin;

impl Plugin for FacesPlugin {
    fn build(&self, app: &mut App) {
        load_shader_library!(app, "shaders/face.wgsl");
        embedded_asset!(app, "shaders/portal.wgsl");
        app.add_plugins(MaterialPlugin::<PortalMaterial>::default())
            .init_resource::<Portals>()
            .add_systems(Startup, spawn_faces)
            .add_systems(Update, draw_faces);
    }
}

/// The colour ramp clamps where its VTF says so, the noise repeats.
fn overlay_texture(files: &Files, name: &str) -> Result<Image, String> {
    let mut image = texture::load(files, name)?;
    let vtf = files
        .read(&format!("materials/{name}.vtf"))
        .map_err(|e| format!("{name}: {e}"))?;
    let flags = portal::vtf::Vtf::parse(&vtf)
        .map_err(|e| format!("{name}: {e}"))?
        .flags;
    if let ImageSampler::Descriptor(s) = &mut image.sampler {
        let mode = |clamp| match clamp {
            true => ImageAddressMode::ClampToEdge,
            false => ImageAddressMode::Repeat,
        };
        s.address_mode_u = mode(flags & CLAMP_S != 0);
        s.address_mode_v = mode(flags & CLAMP_T != 0);
    }
    Ok(image)
}

fn face_mesh(pair: &Pair, c: Colour) -> Mesh {
    let (positions, uvs, normal) = match pair.get(c) {
        Some(p) => {
            let (corners, uvs) = face_quad(p);
            (
                corners.map(|q| to_bevy(q).to_array()),
                uvs,
                to_bevy_dir(p.forward),
            )
        }
        None => ([[0.0; 3]; 4], [[0.0; 2]; 4], Vec3::Y),
    };
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions.to_vec())
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![normal.to_array(); 4])
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs.to_vec())
    .with_inserted_indices(Indices::U16(vec![0, 1, 2, 0, 2, 3]))
}

fn spawn_faces(
    mut commands: Commands,
    files: Res<Files>,
    portals: Res<Portals>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<PortalMaterial>>,
    mut exit: MessageWriter<AppExit>,
) {
    let mut make = |c: Colour| -> Result<(Entity, Handle<PortalMaterial>, Handle<Mesh>), String> {
        let look = overlay_look(&files, c)?;
        let material = materials.add(PortalMaterial {
            params: PortalParams {
                scale: look.scale,
                ..default()
            },
            noise: images.add(overlay_texture(&files, &look.noise)?),
            ramp: images.add(overlay_texture(&files, &look.ramp)?),
        });
        let mesh = meshes.add(face_mesh(&portals, c));
        let entity = commands
            .spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Visibility::Hidden,
            ))
            .id();
        Ok((entity, material, mesh))
    };
    match (make(Colour::Blue), make(Colour::Orange)) {
        (Ok((b, bm, bmesh)), Ok((o, om, omesh))) => commands.insert_resource(Faces {
            entity: [b, o],
            material: [bm, om],
            mesh: [bmesh, omesh],
            seen: 0,
        }),
        (Err(e), _) | (_, Err(e)) => {
            error!("[portal] refused: overlay reason={e}");
            exit.write(AppExit::error());
        }
    }
}

/// Until the faces show their linked view they show full static, as an unlinked portal does.
fn draw_faces(
    time: Res<Time>,
    mut portals: ResMut<Portals>,
    faces: Option<ResMut<Faces>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<PortalMaterial>>,
    mut visibility: Query<&mut Visibility>,
) {
    let Some(mut faces) = faces else { return };
    portals.think(time.delta_secs());
    let moved = faces.seen != portals.changes;
    for c in Colour::ALL {
        let i = c.index();
        if moved {
            if let Some(mut mesh) = meshes.get_mut(&faces.mesh[i]) {
                *mesh = face_mesh(&portals, c);
            }
            if let Ok(mut v) = visibility.get_mut(faces.entity[i]) {
                *v = match portals.get(c) {
                    Some(_) => Visibility::Inherited,
                    None => Visibility::Hidden,
                };
            }
        }
        if let Some(mut m) = materials.get_mut(&faces.material[i]) {
            m.params.open = portals.fade[i].open;
            m.params.keep = 0.0;
            m.params.time = shader_time(time.elapsed_secs());
        }
    }
    faces.seen = portals.changes;
}
