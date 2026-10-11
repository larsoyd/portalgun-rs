use bevy::prelude::*;
use bevy::window::{Monitor, MonitorSelection, PresentMode, PrimaryWindow, WindowMode};

use crate::icon::APP_ID;
use crate::settings::Settings;

pub fn window(settings: &Settings) -> Window {
    Window {
        title: "portalgun".into(),
        name: Some(APP_ID.into()),
        mode: if settings.fullscreen {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        },
        present_mode: if settings.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        },
        ..default()
    }
}

pub struct DisplayPlugin;

impl Plugin for DisplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (toggle_fullscreen, report_display, report_frames));
    }
}

fn toggle_fullscreen(
    keys: Res<ButtonInput<KeyCode>>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    if keys.just_pressed(KeyCode::F11) {
        window.mode = match window.mode {
            WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            _ => WindowMode::Windowed,
        };
    }
}

fn report_display(
    window: Single<&Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor, Changed<Monitor>>,
    mut previous: Local<Option<(UVec2, WindowMode, PresentMode)>>,
) {
    let state = (window.physical_size(), window.mode, window.present_mode);
    if *previous != Some(state) {
        info!(
            "[display] pixels={}x{} mode={:?} present={:?}",
            state.0.x, state.0.y, state.1, state.2
        );
        *previous = Some(state);
    }
    for monitor in &monitors {
        info!(
            "[display] monitor={:?} pixels={}x{} refresh_millihertz={:?}",
            monitor.name,
            monitor.physical_width,
            monitor.physical_height,
            monitor.refresh_rate_millihertz
        );
    }
}

fn report_frames(time: Res<Time<Real>>, mut sample: Local<(f64, u32, f64)>) {
    let dt = time.delta_secs_f64();
    sample.0 += dt;
    sample.1 += 1;
    sample.2 = sample.2.max(dt);
    if sample.0 >= 5.0 {
        info!(
            "[display] fps={:.1} mean_ms={:.2} longest_ms={:.2}",
            f64::from(sample.1) / sample.0,
            sample.0 * 1000.0 / f64::from(sample.1),
            sample.2 * 1000.0
        );
        *sample = (0.0, 0, 0.0);
    }
}
