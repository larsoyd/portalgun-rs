use bevy::asset::AssetPlugin;
use bevy::camera::CameraProjection;
use bevy::prelude::*;
use bevy::render::camera::CameraPlugin;
use bevy::render::texture::ManualTextureViews;
use bevy::window::PrimaryWindow;
use portal::install::{root, Install};
use portal::pair::Portal;
use portalgun::faces::{FacesPlugin, Portals};
use portalgun::player::PlayerCamera;
use portalgun::portals::Colour;
use portalgun::sound::Files;
use portalgun::units::{to_bevy, to_bevy_dir};
use std::path::Path;

#[test]
fn linked_camera_matrices_and_targets_update_in_the_moving_and_resizing_frame() {
    let home = std::env::var("HOME").unwrap();
    let install = Install::open(&root(None, None, Path::new(&home))).unwrap();
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        TransformPlugin,
        WindowPlugin::default(),
    ))
    .init_asset::<Image>()
    .init_asset::<Mesh>()
    .init_asset::<Shader>()
    .init_resource::<ManualTextureViews>()
    .insert_resource(Files(install.search))
    .add_plugins((CameraPlugin, FacesPlugin));
    let main = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            PlayerCamera,
            Msaa::Off,
            Projection::Perspective(PerspectiveProjection::default()),
        ))
        .id();
    {
        let mut portals = app.world_mut().resource_mut::<Portals>();
        for (colour, x, forward) in [(Colour::Blue, -384.0, 1.0), (Colour::Orange, 384.0, -1.0)] {
            portals.place(
                colour,
                Portal::new(
                    [x, 0.0, 64.0],
                    [forward, 0.0, 0.0],
                    [0.0, 0.0, 1.0],
                    (32.0, 54.0),
                ),
            );
        }
    }
    for (size, distance, yaw) in [
        ((1280, 720), 3.0, 170.0_f32),
        ((3440, 1440), 0.1, 95.0),
        ((1920, 1080), 20.0, 185.0),
    ] {
        {
            let world = app.world_mut();
            let mut windows = world.query_filtered::<(Entity, &mut Window), With<PrimaryWindow>>();
            let (entity, mut window) = windows.single_mut(world).unwrap();
            window.resolution.set_physical_resolution(size.0, size.1);
            world.write_message(bevy::window::WindowResized {
                window: entity,
                width: size.0 as f32,
                height: size.1 as f32,
            });
            *world.get_mut::<Transform>(main).unwrap() =
                Transform::from_translation(to_bevy([-384.0 + distance, 0.0, 64.0])).looking_to(
                    to_bevy_dir([yaw.to_radians().cos(), yaw.to_radians().sin(), 0.0]),
                    Vec3::Y,
                );
        }
        app.update();
        let world = app.world_mut();
        let mut cameras = world.query_filtered::<(&Camera, &Projection), Without<PlayerCamera>>();
        let mut active = 0;
        for (camera, projection) in cameras.iter(world).filter(|(c, _)| c.is_active) {
            let Projection::Perspective(p) = projection else {
                continue;
            };
            active += 1;
            assert_eq!(
                camera.physical_target_size(),
                Some(UVec2::new(size.0, size.1))
            );
            assert!((p.aspect_ratio - size.0 as f32 / size.1 as f32).abs() < 1e-5);
            assert!(
                camera
                    .clip_from_view()
                    .abs_diff_eq(p.get_clip_from_view(), 1e-5),
                "linked camera used a stale projection"
            );
        }
        assert!(active > 0, "at least one linked view must render");
    }
}

#[test]
fn particle_view_layers_are_isolated_for_main_recursive_and_screen_copy_views() {
    use bevy::camera::visibility::RenderLayers;
    use portalgun::effects::ParticleView;
    let home = std::env::var("HOME").unwrap();
    let install = Install::open(&root(None, None, Path::new(&home))).unwrap();
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        TransformPlugin,
        WindowPlugin::default(),
    ))
    .init_asset::<Image>()
    .init_asset::<Mesh>()
    .init_asset::<Shader>()
    .init_resource::<ManualTextureViews>()
    .insert_resource(Files(install.search))
    .add_plugins((CameraPlugin, FacesPlugin));
    let main = app
        .world_mut()
        .spawn((Camera3d::default(), PlayerCamera, Msaa::Off))
        .id();
    let gun = app
        .world_mut()
        .spawn((Camera3d::default(), RenderLayers::layer(1)))
        .id();
    app.update();
    assert_eq!(app.world().get::<ParticleView>(main).unwrap().0, 32);
    assert!(app.world().get::<ParticleView>(gun).is_none());
    let views: Vec<_> = app
        .world_mut()
        .query::<(&ParticleView, &RenderLayers)>()
        .iter(app.world())
        .map(|(v, l)| (v.0, l.clone()))
        .collect();
    assert_eq!(views.len(), 6);
    for (layer, layers) in &views {
        assert_eq!(
            layers.intersects(&RenderLayers::layer(portalgun::body::BODY_LAYER)),
            *layer != 32 && *layer != 33,
            "only recursive portal cameras render the player body"
        );
        assert!(layers.intersects(&RenderLayers::layer(0)));
        assert!(layers.intersects(&RenderLayers::layer(*layer)));
        for (other, _) in &views {
            if layer != other {
                assert!(!layers.intersects(&RenderLayers::layer(*other)));
            }
        }
    }
}
