// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::app::PluginsState;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::RenderTarget;
use bevy::core_pipeline::prepass::DepthPrepass;
use bevy::prelude::*;
use bevy::render::render_resource::{
    CachedPipelineState, PipelineCache, PipelineDescriptor, TextureFormat,
};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::render::{pipelined_rendering::PipelinedRenderingPlugin, RenderApp};
use bevy::window::PrimaryWindow;
use portal::install::{root, Install};
use portal::pair::Portal;
use portalgun::body::{BodyPlugin, PlayerBody, BODY_LAYER};
use portalgun::bodyportal::BodyMaterial;
use portalgun::effects::ParticleView;
use portalgun::faces::{FacesPlugin, PortalMaterial, Portals};
use portalgun::player::{Config, Player, PlayerCamera};
use portalgun::portals::Colour;
use portalgun::settings::Settings;
use portalgun::sound::Files;
use portalgun::units::to_bevy;
use portalgun::visual::VisualHistory;
use portalgun::walker::Walker;
use std::time::{Duration, Instant};

#[derive(Resource, Default)]
struct Captured(Option<Image>);

#[derive(Default)]
struct Probe {
    frame: usize,
    known: usize,
}

impl Probe {
    fn frame(&mut self, app: &mut App, phase: &str, deadline: Instant) -> (usize, usize) {
        assert!(Instant::now() < deadline, "probe timeout phase={phase}");
        let started = Instant::now();
        app.update();
        let elapsed = started.elapsed();
        let cache = app.sub_app(RenderApp).world().resource::<PipelineCache>();
        let count = cache.pipelines().count();
        let waiting = cache.waiting_pipelines().count();
        for (id, pipeline) in cache.pipelines().enumerate() {
            let state = match &pipeline.state {
                CachedPipelineState::Queued => "queued",
                CachedPipelineState::Creating(_) => "creating",
                CachedPipelineState::Ok(_) => "ready",
                CachedPipelineState::Err(_) => "waiting-for-shader",
            };
            if id >= self.known || state != "ready" {
                let label = match &pipeline.descriptor {
                    PipelineDescriptor::RenderPipelineDescriptor(p) => p.label.as_deref(),
                    PipelineDescriptor::ComputePipelineDescriptor(p) => p.label.as_deref(),
                };
                println!(
                    "[pipeline] phase={phase} frame={} id={id} label={label:?} state={state}",
                    self.frame
                );
            }
        }
        println!("[connection-frame] phase={phase} frame={} update_ms={:.3} pipelines={count} waiting={waiting}", self.frame, elapsed.as_secs_f64() * 1000.0);
        self.known = count;
        self.frame += 1;
        std::thread::sleep(Duration::from_millis(5));
        (count, waiting)
    }

    fn settle(&mut self, app: &mut App, phase: &str, deadline: Instant) -> usize {
        let mut last = usize::MAX;
        let mut stable = 0;
        for frame in 0..600 {
            let (count, waiting) = self.frame(app, phase, deadline);
            stable = if waiting == 0 && count == last {
                stable + 1
            } else {
                0
            };
            last = count;
            if frame >= 60 && stable >= 12 {
                assert!(
                    app.sub_app(RenderApp)
                        .world()
                        .resource::<PipelineCache>()
                        .pipelines()
                        .all(|p| matches!(p.state, CachedPipelineState::Ok(_))),
                    "failed pipeline phase={phase}"
                );
                return count;
            }
        }
        panic!("pipelines did not settle phase={phase}");
    }

    fn capture(&mut self, app: &mut App, image: Handle<Image>, deadline: Instant) -> Image {
        app.world_mut().spawn(Screenshot::image(image)).observe(
            |shot: On<ScreenshotCaptured>, mut result: ResMut<Captured>| {
                result.0 = Some(shot.image.clone());
            },
        );
        for _ in 0..600 {
            self.frame(app, "capture", deadline);
            if let Some(image) = app.world_mut().resource_mut::<Captured>().0.take() {
                return image;
            }
        }
        panic!("GPU screenshot timeout");
    }
}

fn blue() -> Portal {
    Portal::new(
        [0.0, 0.0, 64.0],
        [-1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    )
}

fn orange() -> Portal {
    Portal::new(
        [-200.0, 0.0, 64.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    )
}

fn link(app: &mut App) {
    let mut pair = app.world_mut().resource_mut::<Portals>();
    pair.place(Colour::Orange, orange());
    for fade in &mut pair.fade {
        fade.open = 1.0;
        fade.statics = 0.0;
    }
}

#[test]
fn first_portal_connection_uses_prepared_render_pipelines() {
    let deadline = Instant::now() + Duration::from_secs(90);
    let home = std::env::var("HOME").unwrap();
    let install = Install::open(&root(None, None, std::path::Path::new(&home))).unwrap();
    let mut walker = Walker::at([-100.0, 0.0, 0.0]);
    walker.grounded = true;
    let visual = VisualHistory::new(&walker).sample(&walker, 0.0);
    let camera_transform = visual.camera();
    let config = Settings::parse("").unwrap();
    let fov = config.vertical_fov();
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
    .insert_resource(Player(walker))
    .insert_resource(visual)
    .insert_resource(Config(config))
    .insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    })
    .init_resource::<Captured>()
    .add_plugins((BodyPlugin, FacesPlugin));
    while app.plugins_state() != PluginsState::Ready {
        assert!(Instant::now() < deadline, "renderer initialization timeout");
        std::thread::sleep(Duration::from_millis(10));
    }
    app.finish();
    app.cleanup();
    app.world_mut().spawn((
        Window {
            resolution: (640, 360).into(),
            ..default()
        },
        PrimaryWindow,
    ));
    let target = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::new_target_texture(
            640,
            360,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
    let main = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            PlayerCamera,
            Msaa::Sample4,
            RenderTarget::Image(target.into()),
            Projection::Perspective(PerspectiveProjection { fov, ..default() }),
            camera_transform,
        ))
        .id();
    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(0.5, 0.5, 0.5));
    let material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.1, 0.1),
            ..default()
        });
    app.world_mut().spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(to_bevy([-80.0, 60.0, 32.0])),
    ));

    let mut probe = Probe::default();
    let startup = probe.settle(&mut app, "startup", deadline);
    assert!(
        app.world_mut()
            .query::<(&Camera, &ParticleView)>()
            .iter(app.world())
            .all(|(camera, view)| view.0 == 32 || !camera.is_active),
        "unused portal views stayed active after preparation"
    );
    app.world_mut()
        .resource_mut::<Portals>()
        .place(Colour::Blue, blue());
    let before_link = probe.settle(&mut app, "one-portal", deadline);
    link(&mut app);
    let after_link = probe.settle(&mut app, "first-link", deadline);
    app.world_mut()
        .resource_mut::<Portals>()
        .deactivate(Colour::Orange);
    probe.settle(&mut app, "unlinked", deadline);
    assert!(
        app.world_mut()
            .query::<(&Camera, &ParticleView)>()
            .iter(app.world())
            .all(|(camera, view)| view.0 == 32 || !camera.is_active),
        "unlinked views must stop rendering"
    );
    link(&mut app);
    let after_relink = probe.settle(&mut app, "relink", deadline);
    println!("[connection-summary] startup={startup} before_link={before_link} first_link={after_link} relink={after_relink}");

    assert!(!app
        .world()
        .get::<RenderLayers>(main)
        .unwrap()
        .intersects(&RenderLayers::layer(BODY_LAYER)));
    let portal_target = app
        .world_mut()
        .query::<(
            &Camera,
            &ParticleView,
            &RenderLayers,
            &RenderTarget,
            &DepthPrepass,
        )>()
        .iter(app.world())
        .find_map(|(camera, view, layers, target, _)| {
            if !camera.is_active
                || view.0 != 35
                || !layers.intersects(&RenderLayers::layer(BODY_LAYER))
            {
                return None;
            }
            match target {
                RenderTarget::Image(target) => Some(target.handle.clone()),
                _ => None,
            }
        })
        .expect("first blue portal must render the real player with a depth prepass");
    let face_entities: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<MeshMaterial3d<PortalMaterial>>>()
        .iter(app.world())
        .collect();
    for entity in face_entities {
        *app.world_mut().get_mut::<Visibility>(entity).unwrap() = Visibility::Hidden;
    }
    for (_, material) in app
        .world_mut()
        .resource_mut::<Assets<BodyMaterial>>()
        .iter_mut()
    {
        material.base.emissive = LinearRgba::WHITE * 2.0;
    }
    for _ in 0..8 {
        probe.frame(&mut app, "body-visible", deadline);
    }
    let shown = probe.capture(&mut app, portal_target.clone(), deadline);
    let body = app
        .world_mut()
        .query_filtered::<Entity, With<PlayerBody>>()
        .single(app.world())
        .unwrap();
    *app.world_mut().get_mut::<Visibility>(body).unwrap() = Visibility::Hidden;
    for _ in 0..8 {
        probe.frame(&mut app, "body-hidden", deadline);
    }
    let hidden = probe.capture(&mut app, portal_target, deadline);
    assert_eq!(
        shown.texture_descriptor.format,
        hidden.texture_descriptor.format
    );
    let shown = shown.data.unwrap();
    let hidden = hidden.data.unwrap();
    assert_eq!(shown.len(), hidden.len());
    let changed_bytes = shown.iter().zip(&hidden).filter(|(a, b)| a != b).count();
    println!("[connection-body] changed_bytes={changed_bytes}");
    assert!(
        changed_bytes > 128,
        "player body produced no visible portal pixels"
    );
    assert_eq!(
        after_relink, after_link,
        "relink queued additional pipelines"
    );
    assert_eq!(
        after_link, before_link,
        "first connection queued new pipelines after startup"
    );
}
