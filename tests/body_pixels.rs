// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::app::PluginsState;
use bevy::camera::{RenderTarget, ScalingMode};
use bevy::core_pipeline::prepass::DepthPrepass;
use bevy::prelude::*;
use bevy::render::render_resource::{PipelineCache, TextureFormat};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::render::{pipelined_rendering::PipelinedRenderingPlugin, RenderApp};
use portalgun::bodyportal::{material, BodyMaterial, BodyPortalPlugin};
use portalgun::visual::VisualHistory;
use portalgun::walker::Walker;
use std::time::{Duration, Instant};

#[derive(Resource, Default)]
struct Captured(Option<Image>);

fn frame(app: &mut App) {
    app.update();
    std::thread::sleep(Duration::from_millis(5));
}

#[test]
fn body_fragments_and_depth_are_clipped_on_the_gpu() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: bevy::window::ExitCondition::DontExit,
                ..default()
            })
            .set(TaskPoolPlugin {
                task_pool_options: TaskPoolOptions::with_num_threads(2),
            })
            .disable::<bevy::winit::WinitPlugin>()
            .disable::<bevy::audio::AudioPlugin>()
            .disable::<bevy::log::LogPlugin>()
            .disable::<PipelinedRenderingPlugin>(),
    )
    .insert_resource(ClearColor(Color::BLACK))
    .init_resource::<Captured>()
    .add_plugins(BodyPortalPlugin);
    let player = Walker::at([0.0; 3]);
    app.insert_resource(VisualHistory::new(&player).sample(&player, 0.0));
    app.insert_resource(portalgun::player::Player(player));
    let deadline = Instant::now() + Duration::from_secs(60);
    while app.plugins_state() != PluginsState::Ready {
        assert!(Instant::now() < deadline, "renderer initialization timeout");
        std::thread::sleep(Duration::from_millis(10));
    }
    app.finish();
    app.cleanup();
    let target = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::new_target_texture(
            64,
            64,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
    let cube = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(2.0, 2.0, 1.0));
    let white = app
        .world_mut()
        .resource_mut::<Assets<BodyMaterial>>()
        .add(material(StandardMaterial {
            emissive: LinearRgba::WHITE * 5.0,
            ..default()
        }));
    let red = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            emissive: LinearRgba::new(5.0, 0.0, 0.0, 1.0),
            ..default()
        });
    app.world_mut()
        .spawn((Mesh3d(cube.clone()), MeshMaterial3d(white.clone())));
    app.world_mut().spawn((
        Mesh3d(cube),
        MeshMaterial3d(red),
        Transform::from_xyz(0.0, 0.0, -2.0),
    ));
    app.world_mut().spawn((
        Camera3d::default(),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        Msaa::Off,
        DepthPrepass,
        RenderTarget::Image(target.clone().into()),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 4.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 0.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    for _ in 0..120 {
        frame(&mut app);
    }
    while app
        .sub_app(RenderApp)
        .world()
        .resource::<PipelineCache>()
        .waiting_pipelines()
        .next()
        .is_some()
    {
        assert!(Instant::now() < deadline, "shader compilation timeout");
        frame(&mut app);
    }
    for plane in [Vec4::ZERO, Vec4::X, -Vec4::X] {
        app.world_mut()
            .resource_mut::<Assets<BodyMaterial>>()
            .get_mut(&white)
            .unwrap()
            .extension
            .plane = plane;
        for _ in 0..5 {
            frame(&mut app);
        }
        app.world_mut()
            .spawn(Screenshot::image(target.clone()))
            .observe(
                |shot: On<ScreenshotCaptured>, mut result: ResMut<Captured>| {
                    result.0 = Some(shot.image.clone());
                },
            );
        while app.world().resource::<Captured>().0.is_none() {
            assert!(Instant::now() < deadline, "GPU screenshot timeout");
            frame(&mut app);
        }
        let image = app.world_mut().resource_mut::<Captured>().0.take().unwrap();
        let pixels = image.data.unwrap();
        for (x, white_expected) in [(20, plane.x <= 0.0), (44, plane.x >= 0.0)] {
            let offset = (32 * 64 + x) * 4;
            let p = &pixels[offset..offset + 4];
            assert!(
                p[0] > 80,
                "missing rear geometry or failed shader plane={plane:?} x={x} rgba={p:?}"
            );
            assert_eq!(
                p[1] > 80 && p[2] > 80,
                white_expected,
                "incorrect colour/depth clipping plane={plane:?} x={x} rgba={p:?}"
            );
        }
    }
}

#[test]
fn model_phong_boost_and_normal_alpha_change_rendered_highlights() {
    use bevy::asset::RenderAssetUsages;
    use bevy::camera::Exposure;
    use bevy::light::NotShadowReceiver;
    use bevy::render::render_resource::{Extent3d, TextureDimension};
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: bevy::window::ExitCondition::DontExit,
                ..default()
            })
            .set(TaskPoolPlugin {
                task_pool_options: TaskPoolOptions::with_num_threads(2),
            })
            .disable::<bevy::winit::WinitPlugin>()
            .disable::<bevy::audio::AudioPlugin>()
            .disable::<bevy::log::LogPlugin>()
            .disable::<PipelinedRenderingPlugin>(),
    )
    .insert_resource(ClearColor(Color::BLACK))
    .insert_resource(GlobalAmbientLight {
        brightness: 0.0,
        ..default()
    })
    .init_resource::<Captured>()
    .add_plugins(BodyPortalPlugin);
    let walker = Walker::at([0.0; 3]);
    app.insert_resource(VisualHistory::new(&walker).sample(&walker, 0.0));
    app.insert_resource(portalgun::player::Player(walker));
    let deadline = Instant::now() + Duration::from_secs(60);
    while app.plugins_state() != PluginsState::Ready {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    app.finish();
    app.cleanup();
    let target = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::new_target_texture(
            64,
            64,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
    let mut pixel = |rgba: [u8; 4]| {
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::new(
                Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                rgba.to_vec(),
                TextureFormat::Rgba8Unorm,
                RenderAssetUsages::default(),
            ))
    };
    let normal = pixel([128, 128, 255, 255]);
    let masked = pixel([128, 128, 255, 0]);
    let exponent = pixel([0, 0, 0, 255]);
    let tinted = pixel([0, 255, 0, 255]);
    let mut material = material(StandardMaterial {
        base_color: Color::BLACK,
        normal_map_texture: Some(normal.clone()),
        ..default()
    });
    material.extension.player_owned = false;
    material.extension.model_flags = Vec4::X;
    material.extension.exponent = Some(exponent.clone());
    material.extension.phong = Vec4::ONE;
    let handle = app
        .world_mut()
        .resource_mut::<Assets<BodyMaterial>>()
        .add(material);
    let mesh = app.world_mut().resource_mut::<Assets<Mesh>>().add(
        Rectangle::new(2.0, 2.0)
            .mesh()
            .build()
            .with_generated_tangents()
            .unwrap(),
    );
    app.world_mut().spawn((
        Mesh3d(mesh),
        MeshMaterial3d(handle.clone()),
        NotShadowReceiver,
    ));
    app.world_mut().spawn((
        PointLight {
            intensity: 100.0,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 3.0),
    ));
    app.world_mut().spawn((
        Camera3d::default(),
        Exposure { ev100: 0.0 },
        Msaa::Off,
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        RenderTarget::Image(target.clone().into()),
        Transform::from_xyz(0.0, 0.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    let mut brightness = Vec::new();
    for (boost, normal, exponent) in [
        (0.0, normal.clone(), exponent.clone()),
        (3.0, normal.clone(), exponent.clone()),
        (3.0, masked, exponent),
        (3.0, normal, tinted),
    ] {
        {
            let mut materials = app.world_mut().resource_mut::<Assets<BodyMaterial>>();
            let mut m = materials.get_mut(&handle).unwrap();
            m.extension.phong.w = boost;
            m.base.normal_map_texture = Some(normal);
            m.extension.exponent = Some(exponent);
        }
        for _ in 0..120 {
            assert!(Instant::now() < deadline);
            frame(&mut app);
        }
        app.world_mut()
            .spawn(Screenshot::image(target.clone()))
            .observe(
                |shot: On<ScreenshotCaptured>, mut result: ResMut<Captured>| {
                    result.0 = Some(shot.image.clone())
                },
            );
        while app.world().resource::<Captured>().0.is_none() {
            assert!(Instant::now() < deadline);
            frame(&mut app);
        }
        let image = app.world_mut().resource_mut::<Captured>().0.take().unwrap();
        brightness.push(image.data.unwrap()[(32 * 64 + 32) * 4]);
    }
    assert!(
        brightness[1] > brightness[0] + 20,
        "Phong boost ignored: {brightness:?}"
    );
    assert!(
        brightness[2] < brightness[1] - 20,
        "normal alpha mask ignored: {brightness:?}"
    );
    assert!(
        brightness[3] < brightness[1] - 20,
        "exponent green must tint the highlight with the black base: {brightness:?}"
    );
}

#[test]
fn model_lightwarp_samples_both_coordinates_from_half_lambert() {
    use bevy::asset::RenderAssetUsages;
    use bevy::camera::Exposure;
    use bevy::light::NotShadowReceiver;
    use bevy::render::render_resource::{Extent3d, TextureDimension};
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: bevy::window::ExitCondition::DontExit,
                ..default()
            })
            .set(TaskPoolPlugin {
                task_pool_options: TaskPoolOptions::with_num_threads(2),
            })
            .disable::<bevy::winit::WinitPlugin>()
            .disable::<bevy::audio::AudioPlugin>()
            .disable::<bevy::log::LogPlugin>()
            .disable::<PipelinedRenderingPlugin>(),
    )
    .insert_resource(ClearColor(Color::BLACK))
    .insert_resource(GlobalAmbientLight {
        brightness: 0.0,
        ..default()
    })
    .init_resource::<Captured>()
    .add_plugins(BodyPortalPlugin);
    let walker = Walker::at([0.0; 3]);
    app.insert_resource(VisualHistory::new(&walker).sample(&walker, 0.0));
    app.insert_resource(portalgun::player::Player(walker));
    let deadline = Instant::now() + Duration::from_secs(60);
    while app.plugins_state() != PluginsState::Ready {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    app.finish();
    app.cleanup();
    let target = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::new_target_texture(
            64,
            64,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
    let mut pixel = |rgba: [u8; 4]| {
        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::new(
                Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                rgba.to_vec(),
                TextureFormat::Rgba8Unorm,
                RenderAssetUsages::default(),
            ))
    };
    let normal = pixel([128, 128, 255, 255]);
    let exponent = pixel([0, 0, 0, 255]);
    let white = pixel([255, 255, 255, 255]);
    let mut ramp = Image::new(
        Extent3d {
            width: 1,
            height: 2,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![0, 0, 0, 255, 255, 255, 255, 255],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    ramp.sampler = bevy::image::ImageSampler::linear();
    let ramp = app.world_mut().resource_mut::<Assets<Image>>().add(ramp);
    let mut material = material(StandardMaterial {
        base_color: Color::WHITE,
        normal_map_texture: Some(normal),
        ..default()
    });
    material.extension.player_owned = false;
    material.extension.model_flags = Vec4::new(1.0, 1.0, 0.0, 0.0);
    material.extension.exponent = Some(exponent);
    material.extension.phong = Vec4::new(1.0, 1.0, 1.0, 0.0);
    let handle = app
        .world_mut()
        .resource_mut::<Assets<BodyMaterial>>()
        .add(material);
    let mesh = app.world_mut().resource_mut::<Assets<Mesh>>().add(
        Rectangle::new(2.0, 2.0)
            .mesh()
            .build()
            .with_generated_tangents()
            .unwrap(),
    );
    app.world_mut().spawn((
        Mesh3d(mesh),
        MeshMaterial3d(handle.clone()),
        NotShadowReceiver,
    ));
    app.world_mut().spawn((
        PointLight {
            intensity: 10.0,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 3.0),
    ));
    app.world_mut().spawn((
        Camera3d::default(),
        Exposure { ev100: 0.0 },
        Msaa::Off,
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        RenderTarget::Image(target.clone().into()),
        Transform::from_xyz(0.0, 0.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    let mut brightness = Vec::new();
    for (warp, exposure) in [(white.clone(), 0.0), (ramp, 0.0), (white, 3.0)] {
        for mut value in app
            .world_mut()
            .query::<&mut Exposure>()
            .iter_mut(app.world_mut())
        {
            value.ev100 = exposure;
        }
        app.world_mut()
            .resource_mut::<Assets<BodyMaterial>>()
            .get_mut(&handle)
            .unwrap()
            .extension
            .lightwarp = Some(warp);
        for _ in 0..120 {
            assert!(Instant::now() < deadline);
            frame(&mut app);
        }
        app.world_mut()
            .spawn(Screenshot::image(target.clone()))
            .observe(
                |shot: On<ScreenshotCaptured>, mut result: ResMut<Captured>| {
                    result.0 = Some(shot.image.clone())
                },
            );
        while app.world().resource::<Captured>().0.is_none() {
            assert!(Instant::now() < deadline);
            frame(&mut app);
        }
        let image = app.world_mut().resource_mut::<Captured>().0.take().unwrap();
        brightness.push(image.data.unwrap()[(32 * 64 + 32) * 4]);
    }
    assert!(
        brightness[0] > 20 && brightness[0] < 240,
        "fixture clipped: {brightness:?}"
    );
    assert!(
        brightness[0].abs_diff(brightness[1]) <= 5,
        "front-lit lightwarp must sample its white last row: {brightness:?}"
    );
    assert!(
        brightness[2] < brightness[0] / 2 && brightness[2] > 10,
        "model lighting must follow camera exposure: {brightness:?}"
    );
}
