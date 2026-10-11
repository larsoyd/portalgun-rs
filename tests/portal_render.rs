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

#[test]
fn crossing_the_camera_plane_hides_only_the_first_recursive_body_view() {
    use bevy::camera::visibility::RenderLayers;
    use portalgun::body::BODY_LAYER;
    use portalgun::effects::ParticleView;
    use portalgun::visual::VisualHistory;
    use portalgun::walker::Walker;
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
    .add_plugins((
        CameraPlugin,
        FacesPlugin,
        portalgun::bodyportal::BodyPortalPlugin,
    ));
    let main = app
        .world_mut()
        .spawn((Camera3d::default(), PlayerCamera, Msaa::Off))
        .id();
    for normal in [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]] {
        let up = if normal[0] == 1.0 {
            [0.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let from = Portal::new([0.0; 3], normal, up, (32.0, 54.0));
        let to = Portal::new(
            [384.0, 0.0, 64.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        );
        for ducked in [false, true] {
            for distance in [2.0, 0.001, 0.0, -0.001, -2.0, 2.0] {
                let mut player = Walker::at([0.0; 3]);
                player.duck.ducked = ducked;
                player.duck.eye = if ducked { 28.0 } else { 64.0 };
                let mut visual = VisualHistory::new(&player).sample(&player, 0.0);
                visual.pose.eye = Vec3::from(normal) * distance;
                visual.pose.centre =
                    visual.pose.eye - Vec3::Z * (player.duck.eye - visual.pose.half.z);
                {
                    let mut portals = app.world_mut().resource_mut::<Portals>();
                    portals.place(Colour::Blue, from);
                    portals.place(Colour::Orange, to);
                }
                *app.world_mut().get_mut::<Transform>(main).unwrap() =
                    visual.portal_camera(Some([from, to]));
                app.insert_resource(visual);
                app.update();
                let world = app.world_mut();
                for (view, layers) in world.query::<(&ParticleView, &RenderLayers)>().iter(world) {
                    let expected = match view.0 {
                        35 | 36 => distance >= 0.0,
                        37 | 38 => true,
                        _ => false,
                    };
                    assert_eq!(
                        layers.intersects(&RenderLayers::layer(BODY_LAYER)),
                        expected,
                        "normal={normal:?} ducked={ducked} distance={distance} view={}",
                        view.0
                    );
                    let ghost = match view.0 {
                        32 | 33 => distance >= 0.0,
                        35 | 36 => distance < 0.0,
                        _ => true,
                    };
                    assert_eq!(
                        layers.intersects(&RenderLayers::layer(portalgun::bodyportal::GHOST_LAYER)),
                        ghost
                    );
                }
            }
        }
    }
}

#[test]
fn falling_through_floor_portals_updates_body_visibility_in_the_same_frame() {
    use bevy::camera::visibility::RenderLayers;
    use bevy::input::InputPlugin;
    use bevy::time::TimeUpdateStrategy;
    use portalgun::body::BODY_LAYER;
    use portalgun::chamber::Chamber;
    use portalgun::effects::ParticleView;
    use portalgun::player::{MovementPlugin, Player};
    use portalgun::scene::Level;
    use portalgun::visual::{VisualHistory, VisualPlayer};
    use portalgun::walker::Walker;
    use std::time::Duration;
    let home = std::env::var("HOME").unwrap();
    let install = Install::open(&root(None, None, Path::new(&home))).unwrap();
    let chamber = Chamber::test_chamber(&install.search).unwrap();
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        TransformPlugin,
        WindowPlugin::default(),
        InputPlugin,
    ))
    .init_asset::<Image>()
    .init_asset::<Mesh>()
    .init_asset::<Shader>()
    .init_resource::<ManualTextureViews>()
    .insert_resource(Files(install.search))
    .insert_resource(Level(chamber))
    .add_plugins((
        CameraPlugin,
        FacesPlugin,
        MovementPlugin,
        portalgun::bodyportal::BodyPortalPlugin,
    ));
    let main = app
        .world_mut()
        .spawn((Camera3d::default(), PlayerCamera, Msaa::Off))
        .id();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    app.update();
    let floor = Portal::new(
        [0.0, -120.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        (32.0, 54.0),
    );
    let exits = [
        Portal::new(
            [-384.0, -120.0, 54.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [-250.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [-250.0, 0.0, 384.0],
            [0.0, 0.0, -1.0],
            [0.0, 1.0, 0.0],
            (32.0, 54.0),
        ),
    ];
    let mut hidden_frames = [0; 3];
    let mut teleports = [0; 3];
    for (exit, to) in exits.into_iter().enumerate() {
        for crouch in [false, true] {
            for fall in [150.0, 700.0] {
                for dt in [1.0 / 240.0, 1.0 / 60.0, 0.045] {
                    let mut player = Walker::at([0.0, -120.0, 80.0]);
                    player.velocity[2] = -fall;
                    player.pitch = 85.0;
                    player.duck.ducked = crouch;
                    player.duck.eye = if crouch { 28.0 } else { 64.0 };
                    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
                    if crouch {
                        keys.press(KeyCode::ControlLeft);
                    } else {
                        keys.release(KeyCode::ControlLeft);
                    }
                    app.insert_resource(VisualHistory::new(&player));
                    app.insert_resource(Player(player));
                    {
                        let mut portals = app.world_mut().resource_mut::<Portals>();
                        portals.place(Colour::Blue, floor);
                        portals.place(Colour::Orange, to);
                    }
                    let mut saw_crossing = false;
                    for _ in 0..(0.8 / dt) as usize {
                        app.insert_resource(TimeUpdateStrategy::ManualDuration(
                            Duration::from_secs_f64(dt),
                        ));
                        app.update();
                        let world = app.world_mut();
                        let visual = *world.resource::<VisualPlayer>();
                        saw_crossing |= visual.revision > 0;
                        let camera = world.get::<Transform>(main).unwrap();
                        let transformed = !camera
                            .translation
                            .abs_diff_eq(visual.camera().translation, 0.0001);
                        hidden_frames[exit] += usize::from(transformed);
                        for (view, layers) in
                            world.query::<(&ParticleView, &RenderLayers)>().iter(world)
                        {
                            let visible = layers.intersects(&RenderLayers::layer(BODY_LAYER));
                            let ghost = layers.intersects(&RenderLayers::layer(
                                portalgun::bodyportal::GHOST_LAYER,
                            ));
                            assert_eq!(
                                ghost,
                                match view.0 {
                                    32 | 33 => !transformed,
                                    35 | 36 => transformed,
                                    _ => true,
                                }
                            );
                            match view.0 {
                                35 | 36 => assert_eq!(
                                    visible, !transformed,
                                    "exit={exit} crouch={crouch} fall={fall} dt={dt}"
                                ),
                                37 | 38 => assert!(visible),
                                _ => assert!(!visible),
                            }
                        }
                    }
                    assert!(
                        saw_crossing,
                        "no teleport exit={exit} crouch={crouch} fall={fall} dt={dt}"
                    );
                    teleports[exit] += 1;
                }
            }
        }
    }
    assert!(
        hidden_frames[2] > 0,
        "ceiling exit never transformed the camera: {hidden_frames:?}"
    );
    eprintln!("floor crossings={teleports:?} transformed camera frames={hidden_frames:?}");
}
