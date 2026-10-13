// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::app::PluginsState;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{RenderTarget, ScalingMode};
use bevy::core_pipeline::prepass::DepthPrepass;
use bevy::prelude::*;
use bevy::render::render_resource::{PipelineCache, TextureFormat};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::render::{pipelined_rendering::PipelinedRenderingPlugin, RenderApp};
use portal::install::{root, Install};
use portal::pair::{through_dir, Portal};
use portalgun::body::{BodyPlugin, BODY_LAYER};
use portalgun::bodyanim::BodyAnimation;
use portalgun::bodyportal::{BodyMaterial, GHOST_LAYER};
use portalgun::faces::Portals;
use portalgun::player::Player;
use portalgun::portals::Colour;
use portalgun::sound::Files;
use portalgun::units::{to_bevy, to_bevy_dir, METRES_PER_UNIT};
use portalgun::visual::VisualHistory;
use portalgun::walker::Walker;
use std::time::{Duration, Instant};

const SIZE: u32 = 256;
const VIEW_UNITS: f32 = 112.0;

#[derive(Resource, Default)]
struct Captured(Option<Image>);

fn frame(app: &mut App, deadline: Instant) {
    assert!(
        Instant::now() < deadline,
        "body rendering deadline exceeded"
    );
    app.update();
    std::thread::sleep(Duration::from_millis(5));
}

fn settle(app: &mut App, deadline: Instant) {
    for _ in 0..8 {
        frame(app, deadline);
    }
    while app
        .sub_app(RenderApp)
        .world()
        .resource::<PipelineCache>()
        .waiting_pipelines()
        .next()
        .is_some()
    {
        frame(app, deadline);
    }
}

fn capture(app: &mut App, target: &Handle<Image>, deadline: Instant) -> Image {
    app.world_mut()
        .spawn(Screenshot::image(target.clone()))
        .observe(
            |shot: On<ScreenshotCaptured>, mut result: ResMut<Captured>| {
                result.0 = Some(shot.image.clone());
            },
        );
    while app.world().resource::<Captured>().0.is_none() {
        frame(app, deadline);
    }
    app.world_mut().resource_mut::<Captured>().0.take().unwrap()
}

fn check_pixels(image: &Image, bias: f32, visible: bool, label: &str) {
    let pixels = image.data.as_ref().expect("screenshot pixels");
    if let Some(directory) = std::env::var_os("PORTALGUN_BODY_PIXEL_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        image::save_buffer(
            directory.join(format!("{label}.png")),
            pixels,
            SIZE,
            SIZE,
            image::ColorType::Rgba8,
        )
        .unwrap();
    }
    let mut retained = 0;
    let mut leaked = 0;
    for y in 0..SIZE {
        let distance = (0.5 - (y as f32 + 0.5) / SIZE as f32) * VIEW_UNITS;
        for x in 0..SIZE {
            let at = ((y * SIZE + x) * 4) as usize;
            let lit = pixels[at..at + 3].iter().any(|&v| v > 32);
            retained += usize::from(lit);
            leaked += usize::from(lit && distance < -bias - VIEW_UNITS / SIZE as f32);
        }
    }
    if visible {
        assert!(
            retained > 12,
            "body disappeared label={label} pixels={retained}"
        );
    } else {
        assert_eq!(retained, 0, "phantom body label={label}");
    }
    assert_eq!(
        leaked, 0,
        "body escaped its clip half label={label} retained={retained}"
    );
    eprintln!("[body-pixels] label={label} pixels={retained} leaked={leaked}");
}

fn observer(portal: &Portal) -> Transform {
    let origin = Vec3::from(portal.origin);
    Transform::from_translation(to_bevy(
        (origin + Vec3::from(portal.right) * 160.0).to_array(),
    ))
    .looking_at(to_bevy(portal.origin), to_bevy_dir(portal.forward))
}

fn player(centre: Vec3, crouched: bool, yaw: f32, velocity: Vec3) -> Walker {
    let half = if crouched { 18.0 } else { 36.0 };
    let mut walker = Walker::at((centre - Vec3::Z * half).to_array());
    walker.duck.ducked = crouched;
    walker.duck.eye = if crouched { 28.0 } else { 64.0 };
    walker.yaw = yaw;
    walker.velocity = velocity.to_array();
    walker
}

fn show_pose(app: &mut App, walker: Walker) {
    let visual = VisualHistory::new(&walker).sample(&walker, 0.0);
    app.insert_resource(visual);
    app.insert_resource(Player(walker));
    let mut animation = BodyAnimation::default();
    animation.original_airwalk = true;
    animation.original_feet = true;
    animation.original_gait = true;
    app.insert_resource(animation);
}

#[test]
fn rendered_player_halves_survive_crouched_corners_and_every_floor_exit() {
    let home = std::env::var("HOME").unwrap();
    let install = Install::open(&root(None, None, std::path::Path::new(&home))).unwrap();
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
    .insert_resource(Files(install.search))
    .init_resource::<Portals>()
    .init_resource::<Captured>()
    .add_plugins(BodyPlugin);
    show_pose(&mut app, player(Vec3::Z * 36.0, false, 0.0, Vec3::ZERO));
    let deadline = Instant::now() + Duration::from_secs(180);
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
            SIZE,
            SIZE,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
    let camera = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            bevy::core_pipeline::tonemapping::Tonemapping::None,
            Msaa::Off,
            DepthPrepass,
            RenderLayers::layer(GHOST_LAYER),
            RenderTarget::Image(target.clone().into()),
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::FixedVertical {
                    viewport_height: VIEW_UNITS * METRES_PER_UNIT,
                },
                ..OrthographicProjection::default_3d()
            }),
            Transform::default(),
        ))
        .id();
    frame(&mut app, deadline);
    for (_, material) in app
        .world_mut()
        .resource_mut::<Assets<BodyMaterial>>()
        .iter_mut()
    {
        material.base.base_color = Color::WHITE;
        material.base.base_color_texture = None;
        material.base.emissive = LinearRgba::WHITE * 5.0;
    }

    let floor = Portal::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], (32.0, 54.0));
    let exits = [
        Portal::new(
            [200.0, 0.0, 54.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [200.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [200.0, 0.0, 384.0],
            [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0],
            (32.0, 54.0),
        ),
    ];
    for (exit_kind, exit) in exits.into_iter().enumerate() {
        {
            let mut portals = app.world_mut().resource_mut::<Portals>();
            portals.place(Colour::Blue, floor);
            portals.place(Colour::Orange, exit);
        }
        for crouched in [false, true] {
            for after in [false, true] {
                let (source, linked) = if after { (exit, floor) } else { (floor, exit) };
                let centre = Vec3::from(source.origin) + Vec3::from(source.forward);
                show_pose(&mut app, player(centre, crouched, 0.0, Vec3::ZERO));
                for (layer, plane, bias) in [(BODY_LAYER, source, 0.3), (GHOST_LAYER, linked, 0.75)]
                {
                    *app.world_mut().get_mut::<RenderLayers>(camera).unwrap() =
                        RenderLayers::layer(layer);
                    *app.world_mut().get_mut::<Transform>(camera).unwrap() = observer(&plane);
                    settle(&mut app, deadline);
                    let image = capture(&mut app, &target, deadline);
                    check_pixels(
                        &image,
                        bias,
                        true,
                        &format!("floor-{exit_kind}-crouch-{crouched}-after-{after}-layer-{layer}"),
                    );
                }
            }
        }
    }

    let corner = [
        Portal::new(
            [0.0, 0.0, 54.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [64.0, 64.0, 54.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
    ];
    {
        let mut portals = app.world_mut().resource_mut::<Portals>();
        portals.place(Colour::Blue, corner[0]);
        portals.place(Colour::Orange, corner[1]);
    }
    for after in [false, true] {
        let source = corner[usize::from(after)];
        let linked = corner[usize::from(!after)];
        let direction = if after {
            Vec3::from(through_dir(&corner[0], &corner[1], [-1.0, 0.0, 0.0]))
        } else {
            Vec3::NEG_X
        };
        let centre = Vec3::from(source.origin) + Vec3::from(source.forward) - Vec3::Z * 36.0;
        show_pose(
            &mut app,
            player(
                centre,
                true,
                direction.y.atan2(direction.x).to_degrees(),
                direction * 60.0,
            ),
        );
        *app.world_mut().get_mut::<RenderLayers>(camera).unwrap() =
            RenderLayers::layer(GHOST_LAYER);
        *app.world_mut().get_mut::<Transform>(camera).unwrap() = observer(&linked);
        settle(&mut app, deadline);
        let image = capture(&mut app, &target, deadline);
        check_pixels(&image, 0.75, true, &format!("corner-crouch-after-{after}"));
    }

    {
        let mut portals = app.world_mut().resource_mut::<Portals>();
        portals.place(Colour::Blue, floor);
        portals.place(Colour::Orange, exits[2]);
    }
    show_pose(&mut app, player(Vec3::Z * 36.0, false, 0.0, Vec3::ZERO));
    *app.world_mut().get_mut::<RenderLayers>(camera).unwrap() = RenderLayers::layer(GHOST_LAYER);
    *app.world_mut().get_mut::<Transform>(camera).unwrap() = observer(&exits[2]);
    *app.world_mut().get_mut::<Projection>(camera).unwrap() =
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 16.0 * METRES_PER_UNIT,
            },
            ..OrthographicProjection::default_3d()
        });
    settle(&mut app, deadline);
    let image = capture(&mut app, &target, deadline);
    check_pixels(&image, 0.75, false, "standing-near-floor-no-ceiling-legs");

    for crouched in [false, true] {
        let mut walker = player(
            Vec3::new(60.0, 0.0, if crouched { 18.0 } else { 36.0 }),
            crouched,
            180.0,
            Vec3::NEG_X * 50.0,
        );
        walker.grounded = true;
        show_pose(&mut app, walker);
        let mut view = observer(&exits[2]);
        view.translation += to_bevy_dir(exits[2].up) * (60.0 * METRES_PER_UNIT);
        *app.world_mut().get_mut::<Transform>(camera).unwrap() = view;
        settle(&mut app, deadline);
        let image = capture(&mut app, &target, deadline);
        check_pixels(
            &image,
            0.75,
            false,
            &format!("moving-rim-crouched-{crouched}"),
        );
    }
}
