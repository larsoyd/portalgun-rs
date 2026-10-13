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
    for _ in 0..30 {
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
