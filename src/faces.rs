// the bind group derive leaves checks the compiler calls unused
#![allow(dead_code)]

use bevy::asset::{embedded_asset, RenderAssetUsages};
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::camera::{CameraProjection, CameraUpdateSystems, Hdr, RenderTarget, SubCameraView};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::image::{ImageAddressMode, ImageSampler};
use bevy::math::Vec3A;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, RenderPipelineDescriptor, ShaderType,
    SpecializedMeshPipelineError, TextureFormat,
};
use bevy::shader::{load_shader_library, ShaderRef};
use bevy::window::PrimaryWindow;
use portal::portal_face::{near, shader_time};
use portal::view::{
    clip_plane, draws_doubler, max_depth, near_plane, remaining, render_fix, shows_view,
    static_amount, STENCIL_DEPTH,
};

use crate::player::PlayerCamera;
use crate::portals::{doubler_mask, face_quad, overlay_look, Colour, Pair};
use crate::sound::Files;
use crate::texture;
use crate::units::{from_bevy, from_bevy_dir, to_bevy, to_bevy_dir, METRES_PER_UNIT};
use crate::views::{frustum_planes, through, view_clip_plane};

/// TEXTUREFLAGS_CLAMPS and TEXTUREFLAGS_CLAMPT
const CLAMP_S: u32 = 0x4;
const CLAMP_T: u32 = 0x8;
/// the faces the window's view draws, levels below get a layer per colour
pub const MAIN_FACES: usize = 2;
/// a D24S8 depth buffer, what the game's views are counted against
const STENCIL_BITS: u32 = 8;
/// _rt_DepthDoubler is made 512 square
const DOUBLER_SIZE: u32 = 512;
const DOUBLER_ORDER: isize = -2;

fn face_layer(c: Colour, level: usize) -> usize {
    MAIN_FACES + 1 + c.index() + 2 * (level - 1)
}

#[derive(Resource, Default, Deref, DerefMut)]
pub struct Portals(pub Pair);

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(PortalKey)]
pub struct PortalMaterial {
    #[uniform(0)]
    pub params: PortalParams,
    #[texture(1)]
    #[sampler(2)]
    pub noise: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    pub ramp: Handle<Image>,
    #[texture(5)]
    #[sampler(6)]
    pub view: Handle<Image>,
    #[texture(7)]
    #[sampler(8)]
    pub mask: Handle<Image>,
    /// drawn over whatever is there, as the stencil lets the view overwrite it
    pub on_top: bool,
}

#[repr(C)]
#[derive(Eq, PartialEq, Hash, Copy, Clone)]
pub struct PortalKey {
    on_top: bool,
}

impl From<&PortalMaterial> for PortalKey {
    fn from(m: &PortalMaterial) -> Self {
        PortalKey { on_top: m.on_top }
    }
}

#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct PortalParams {
    open: f32,
    keep: f32,
    scale: f32,
    time: f32,
    hole: f32,
    doubler: f32,
    fix: f32,
    doubler_clip_from_world: Mat4,
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

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if key.bind_group_data.on_top {
            descriptor.primitive.cull_mode = None;
            if let Some(depth) = descriptor.depth_stencil.as_mut() {
                depth.depth_compare = Some(CompareFunction::Always);
                depth.depth_write_enabled = Some(false);
            }
        }
        Ok(())
    }
}

/// The window's projection kept on a target of another shape, as the screen copy is.
#[derive(Debug, Clone)]
struct Kept(PerspectiveProjection);

impl CameraProjection for Kept {
    fn get_clip_from_view(&self) -> Mat4 {
        self.0.get_clip_from_view()
    }

    fn get_clip_from_view_for_sub(&self, sub_view: &SubCameraView) -> Mat4 {
        self.0.get_clip_from_view_for_sub(sub_view)
    }

    fn update(&mut self, _width: f32, _height: f32) {}

    fn far(&self) -> f32 {
        self.0.far
    }

    fn get_frustum_corners(&self, z_near: f32, z_far: f32) -> [Vec3A; 8] {
        self.0.get_frustum_corners(z_near, z_far)
    }
}

/// One colour's faces, from the window's view down, and the cameras looking through it.
struct Chain {
    faces: Vec<(Entity, Handle<PortalMaterial>)>,
    cameras: Vec<Entity>,
    images: Vec<Handle<Image>>,
    /// the view the doubler projects through, from the last frame (0x990 in client.so)
    saved: Option<Mat4>,
    /// the near plane's cut through the hole, filled with the first view
    fix: (Entity, Handle<Mesh>, Handle<PortalMaterial>),
}

#[derive(Resource)]
struct Faces {
    depth: usize,
    mesh: [Handle<Mesh>; 2],
    chain: [Chain; 2],
    doubler: (Entity, Handle<Image>),
    blank: Handle<Image>,
    size: UVec2,
    seen: u64,
    /// which views rendered last frame, logged when it changes
    shown: String,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortalViewsUpdated;

pub struct FacesPlugin;

impl Plugin for FacesPlugin {
    fn build(&self, app: &mut App) {
        load_shader_library!(app, "shaders/face.wgsl");
        embedded_asset!(app, "shaders/portal.wgsl");
        app.add_plugins(MaterialPlugin::<PortalMaterial>::default())
            .init_resource::<Portals>()
            .add_systems(PostStartup, spawn_faces)
            .add_systems(
                PostUpdate,
                draw_views
                    .in_set(PortalViewsUpdated)
                    .before(CameraUpdateSystems)
                    .before(TransformSystems::Propagate),
            );
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

/// A convex polygon in Source units as a fan.
fn fix_mesh(poly: &[[f32; 3]]) -> Mesh {
    let positions: Vec<[f32; 3]> = poly.iter().map(|&p| to_bevy(p).to_array()).collect();
    let fan: Vec<u32> = (1..poly.len().saturating_sub(1) as u32)
        .flat_map(|i| [0, i, i + 1])
        .collect();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        vec![[0.0, 0.0, 1.0]; positions.len()],
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; positions.len()])
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_indices(Indices::U32(fan))
}

fn target(size: UVec2) -> Image {
    Image::new_target_texture(size.x, size.y, TextureFormat::Rgba16Float, None)
}

fn view_camera(commands: &mut Commands, image: Handle<Image>, order: isize, msaa: Msaa) -> Entity {
    commands
        .spawn((
            Camera3d::default(),
            Camera {
                order,
                is_active: false,
                ..default()
            },
            RenderTarget::Image(image.into()),
            Hdr,
            Tonemapping::None,
            DebandDither::Disabled,
            msaa,
        ))
        .id()
}

#[allow(clippy::too_many_arguments)]
fn spawn_faces(
    mut commands: Commands,
    files: Res<Files>,
    portals: Res<Portals>,
    main: Single<(Entity, &Msaa), With<PlayerCamera>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<PortalMaterial>>,
    mut exit: MessageWriter<AppExit>,
) {
    let depth = max_depth(STENCIL_DEPTH, STENCIL_BITS) as usize;
    let (main_entity, msaa) = *main;
    let msaa = *msaa;
    commands.entity(main_entity).insert((
        RenderLayers::from_layers(&[0, MAIN_FACES, 32]),
        crate::effects::ParticleView(32),
        bevy::core_pipeline::prepass::DepthPrepass,
    ));
    let blank = images.add(target(UVec2::ONE));
    let mask = match doubler_mask(&files).and_then(|name| texture::load(&files, &name)) {
        Ok(i) => images.add(i),
        Err(e) => {
            error!("[portal] refused: doubler reason={e}");
            exit.write(AppExit::error());
            return;
        }
    };
    let doubler_image = images.add(target(UVec2::splat(DOUBLER_SIZE)));
    let doubler = view_camera(&mut commands, doubler_image.clone(), DOUBLER_ORDER, msaa);
    commands.entity(doubler).insert((
        RenderLayers::from_layers(&[0, MAIN_FACES, 33]),
        crate::effects::ParticleView(33),
        bevy::core_pipeline::prepass::DepthPrepass,
    ));
    let mut make = |c: Colour| -> Result<(Handle<Mesh>, Chain), String> {
        let look = overlay_look(&files, c)?;
        let noise = images.add(overlay_texture(&files, &look.noise)?);
        let ramp = images.add(overlay_texture(&files, &look.ramp)?);
        let mesh = meshes.add(face_mesh(&portals, c));
        let mut chain = Chain {
            faces: Vec::new(),
            cameras: Vec::new(),
            images: Vec::new(),
            saved: None,
            fix: (Entity::PLACEHOLDER, Handle::default(), Handle::default()),
        };
        for level in 0..=depth {
            let material = materials.add(PortalMaterial {
                params: PortalParams {
                    scale: look.scale,
                    ..default()
                },
                noise: noise.clone(),
                ramp: ramp.clone(),
                view: blank.clone(),
                mask: mask.clone(),
                on_top: false,
            });
            let layer = match level {
                0 => MAIN_FACES,
                l => face_layer(c, l),
            };
            let face = commands
                .spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Visibility::Hidden,
                    RenderLayers::layer(layer),
                ))
                .id();
            chain.faces.push((face, material));
            if level > 0 {
                let image = images.add(target(UVec2::ONE));
                let order = DOUBLER_ORDER - (2 * level + c.index()) as isize;
                let camera = view_camera(&mut commands, image.clone(), order, msaa);
                commands.entity(camera).insert((
                    RenderLayers::from_layers(&[0, layer, 32 + layer]),
                    crate::effects::ParticleView(32 + layer),
                    bevy::core_pipeline::prepass::DepthPrepass,
                ));
                chain.cameras.push(camera);
                chain.images.push(image);
            }
        }
        let fix_mesh = meshes.add(fix_mesh(&[[0.0; 3]; 3]));
        let fix_material = materials.add(PortalMaterial {
            params: PortalParams {
                fix: 1.0,
                ..default()
            },
            noise: noise.clone(),
            ramp: ramp.clone(),
            view: chain.images[0].clone(),
            mask: mask.clone(),
            on_top: true,
        });
        let fix = commands
            .spawn((
                Mesh3d(fix_mesh.clone()),
                MeshMaterial3d(fix_material.clone()),
                RenderLayers::layer(MAIN_FACES),
                NoFrustumCulling,
            ))
            .id();
        chain.fix = (fix, fix_mesh, fix_material);
        Ok((mesh, chain))
    };
    match (make(Colour::Blue), make(Colour::Orange)) {
        (Ok((bmesh, blue)), Ok((omesh, orange))) => {
            info!("[portal] views depth={depth} cameras={}", 2 * depth + 1);
            commands.insert_resource(Faces {
                depth,
                mesh: [bmesh, omesh],
                chain: [blue, orange],
                doubler: (doubler, doubler_image),
                blank,
                size: UVec2::ZERO,
                seen: 0,
                shown: String::new(),
            });
        }
        (Err(e), _) | (_, Err(e)) => {
            error!("[portal] refused: overlay reason={e}");
            exit.write(AppExit::error());
        }
    }
}

/// The window's projection without a clip plane, the frustum the game culls a level's portals by.
fn plain(p: &PerspectiveProjection) -> PerspectiveProjection {
    PerspectiveProjection {
        near_clip_plane: Vec4::new(0.0, 0.0, -1.0, -p.near),
        ..p.clone()
    }
}

type MainView<'a> = (&'a Transform, &'a Projection, &'a Camera);
type ViewCameras<'a> = (&'a mut Transform, &'a mut Projection, &'a mut Camera);

#[allow(clippy::too_many_arguments)]
fn draw_views(
    time: Res<Time>,
    mut portals: ResMut<Portals>,
    faces: Option<ResMut<Faces>>,
    main: Single<MainView, With<PlayerCamera>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut cameras: Query<ViewCameras, Without<PlayerCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<PortalMaterial>>,
    mut visibility: Query<&mut Visibility>,
) {
    let Some(faces) = faces else { return };
    let faces = faces.into_inner();
    let (eye, projection, main_camera) = *main;
    let Projection::Perspective(perspective) = projection else {
        return;
    };
    portals.think(time.delta_secs());
    let eye_at = from_bevy(eye.translation);
    for c in Colour::ALL {
        if portals.get(c).is_some_and(|p| near(p, eye_at)) {
            portals.fade[c.index()].statics = 0.0;
        }
    }
    let moved = faces.seen != portals.changes;
    faces.seen = portals.changes;
    let size = main_camera
        .viewport
        .as_ref()
        .map_or_else(|| window.physical_size(), |v| v.physical_size)
        .max(UVec2::ONE);
    let resized = size != faces.size;
    if resized {
        faces.size = size;
        info!("[portal] views size={}x{}", size.x, size.y);
    }
    let depth = faces.depth;
    let linked = portals.linked();
    let mut plain = plain(perspective);
    plain.update(size.x as f32, size.y as f32);
    let doubler_projection = plain.get_clip_from_view();
    let mut any_doubler = false;
    let main_frustum = frustum_planes(&(plain.get_clip_from_view() * eye.to_matrix().inverse()));
    let mut report = Vec::new();
    for c in Colour::ALL {
        let i = c.index();
        let fade = portals.fade[i];
        let usable = portals.usable[i];
        let chain = &mut faces.chain[i];
        if moved {
            if let Some(mut mesh) = meshes.get_mut(&faces.mesh[i]) {
                *mesh = face_mesh(&portals, c);
            }
            for (face, _) in &chain.faces {
                if let Ok(mut v) = visibility.get_mut(*face) {
                    *v = match portals.get(c) {
                        Some(_) => Visibility::Inherited,
                        None => Visibility::Hidden,
                    };
                }
            }
        }
        if resized {
            for image in &chain.images {
                if let Some(mut im) = images.get_mut(image) {
                    *im = target(size);
                }
            }
        }
        let mut parent = eye.to_matrix();
        let mut near_units = perspective.near / METRES_PER_UNIT;
        let mut shown = vec![false; depth + 1];
        let mut saved = chain.saved;
        for level in 1..=depth {
            let clip_from_world = plain.get_clip_from_view() * parent.inverse();
            let frustum = frustum_planes(&clip_from_world);
            let seen_from = from_bevy(parent.w_axis.truncate());
            let above = level == 1 || shown[level - 1];
            shown[level] = above
                && linked.is_some_and(|l| {
                    shows_view(
                        &l[i],
                        fade.statics,
                        true,
                        seen_from,
                        level as i32 - 1,
                        &frustum,
                    )
                });
            let Ok((mut t, mut proj, mut camera)) = cameras.get_mut(chain.cameras[level - 1])
            else {
                continue;
            };
            if camera.is_active != shown[level] {
                camera.is_active = shown[level];
            }
            let (Some(l), true) = (linked, shown[level]) else {
                continue;
            };
            let (from, to) = (&l[i], &l[1 - i]);
            let moved_view = through(from, to) * parent;
            *t = Transform::from_matrix(moved_view);
            near_units = near_plane(near_units);
            *proj = Projection::Perspective(PerspectiveProjection {
                near: near_units * METRES_PER_UNIT,
                near_clip_plane: view_clip_plane(clip_plane(to), &t),
                ..plain.clone()
            });
            if usable && level + 1 == depth {
                saved = Some(moved_view);
            }
            parent = moved_view;
        }
        for level in 0..=depth {
            let rem = remaining(depth as i32, level as i32);
            let doubler = draws_doubler(usable, rem, level as i32);
            let hole = level < depth && shown[level + 1];
            let Some(mut m) = materials.get_mut(&chain.faces[level].1) else {
                continue;
            };
            let view = match (doubler, hole) {
                (true, _) => &faces.doubler.1,
                (false, true) => &chain.images[level],
                (false, false) => &faces.blank,
            };
            if m.view != *view {
                m.view = view.clone();
            }
            m.params.open = fade.open;
            m.params.keep = 1.0 - static_amount(fade.statics, linked.is_some(), rem, doubler);
            m.params.time = shader_time(time.elapsed_secs());
            m.params.hole = hole as u8 as f32;
            m.params.doubler = doubler as u8 as f32;
            m.params.doubler_clip_from_world =
                doubler_projection * chain.saved.unwrap_or(Mat4::IDENTITY).inverse();
        }
        chain.saved = saved;
        let fix = match (portals.get(c), shown[1]) {
            (Some(p), true) => render_fix(
                p,
                eye_at,
                from_bevy_dir(*eye.forward()),
                from_bevy_dir(*eye.right()),
                from_bevy_dir(*eye.up()),
                perspective.near / METRES_PER_UNIT,
                &main_frustum,
                0,
            ),
            _ => Vec::new(),
        };
        // kept drawn as a point when unused, so its pipeline is ready the frame it is needed
        let fix = match fix.is_empty() {
            true => vec![[0.0; 3]; 3],
            false => fix,
        };
        if let Some(mut mesh) = meshes.get_mut(&chain.fix.1) {
            *mesh = fix_mesh(&fix);
        }
        let levels: String = shown[1..]
            .iter()
            .map(|&v| if v { '1' } else { '0' })
            .collect();
        report.push(format!("{}={levels}", format!("{c:?}").to_lowercase()));
        any_doubler |= usable && linked.is_some() && 1.0 > fade.statics;
    }
    if let Ok((mut t, mut proj, mut camera)) = cameras.get_mut(faces.doubler.0) {
        *t = *eye;
        *proj = Projection::custom(Kept(plain.clone()));
        if camera.is_active != any_doubler {
            camera.is_active = any_doubler;
        }
    }
    let report = format!("{} doubler={any_doubler}", report.join(" "));
    if report != faces.shown {
        info!("[portal] shown at={:.2} {report}", time.elapsed_secs());
        faces.shown = report;
    }
}
