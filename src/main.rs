// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::Path;
use std::process::ExitCode;

use bevy::prelude::*;
use portal::install::{root, Install};
use portalgun::args::Args;
use portalgun::chamber::Chamber;
use portalgun::diagnostics::{self, LogFile};
use portalgun::display::{self, DisplayPlugin};
use portalgun::faces::FacesPlugin;
use portalgun::firing::FiringPlugin;
use portalgun::hud::{HudPlugin, InstallRoot};
use portalgun::icon::{self, GameIcon};
use portalgun::player::{Config, PlayerPlugin, Scripted};
use portalgun::run::RunPlugin;
use portalgun::scene::{Level, ScenePlugin};
use portalgun::script::Script;
use portalgun::settings::Settings;
use portalgun::sound::{Files, Sounds};
use portalgun::view::{GunMeshes, ViewPlugin};
use portalgun::viewmodel::read_gun;

fn main() -> ExitCode {
    let args = match Args::parse(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => return refuse(&e),
    };
    let file = match args.log_file.as_deref().map(LogFile::open).transpose() {
        Ok(file) => file,
        Err(reason) => return refuse(&format!("log file: {reason}")),
    };
    let mut app = App::new();
    if let Some(file) = &file {
        app.insert_resource(file.clone());
    }
    app.add_plugins(bevy::log::LogPlugin {
        filter: format!("{},portalgun=debug", bevy::log::DEFAULT_FILTER),
        custom_layer: LogFile::layer,
        ..default()
    });
    diagnostics::panic_log(file.clone());
    info!(
        "[startup] version={} pid={} args={args:?}",
        env!("CARGO_PKG_VERSION"),
        std::process::id()
    );
    let exit = run(args, app);
    info!("[run] exit={exit:?}");
    if let Some(file) = file {
        if let Err(reason) = file.flush() {
            eprintln!("[log] flush=failed reason={reason}");
        }
    }
    exit
}

fn run(args: Args, mut app: App) -> ExitCode {
    let home = std::env::var("HOME").unwrap_or_default();
    let dir = root(
        args.install.as_deref(),
        std::env::var("PORTAL_DIR").ok().as_deref(),
        Path::new(&home),
    );
    let settings = match settings(&args.settings) {
        Ok(s) => s,
        Err(e) => return refuse(&e),
    };
    info!(
        "[settings] file={} effective={settings:?}",
        args.settings.display()
    );
    let script = match &args.script {
        Some(path) => match std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|t| Script::parse(&t))
        {
            Ok(s) => Some(s),
            Err(e) => return refuse(&format!("{}: {e}", path.display())),
        },
        None => None,
    };
    let started = std::time::Instant::now();
    let install = match Install::open(&dir) {
        Ok(i) => i,
        Err(e) => return refuse(&e.to_string()),
    };
    let particles = match portalgun::effects::ParticleEffects::read(&install) {
        Ok(p) => p,
        Err(e) => return refuse(&e),
    };
    let sounds = match Sounds::read(&install.search) {
        Ok(s) => s,
        Err(e) => return refuse(&e.to_string()),
    };
    let chamber = match Chamber::test_chamber(&install.search) {
        Ok(c) => c,
        Err(e) => return refuse(&e.to_string()),
    };
    let (mdl, pieces) = match read_gun(&install.search) {
        Ok(g) => g,
        Err(e) => return refuse(&e),
    };
    let game_icon = match icon::read(&install.root) {
        Ok(i) => i,
        Err(e) => return refuse(&e),
    };
    desktop_entry(&game_icon);
    info!(
        "[startup] install={} checked_ms={}",
        install.root.display(),
        started.elapsed().as_millis()
    );
    if let Some(script) = script {
        app.insert_resource(Scripted(script));
    }
    let exit = app
        .add_plugins(
            DefaultPlugins
                .build()
                .disable::<bevy::log::LogPlugin>()
                .set(WindowPlugin {
                    primary_window: Some(display::window(&settings)),
                    ..default()
                }),
        )
        .add_plugins(DisplayPlugin)
        .add_plugins(RunPlugin {
            shots: args.shots,
            quit_after: args.quit_after,
            shot_dir: args.shot_dir,
        })
        .insert_resource(InstallRoot(install.root.clone()))
        .insert_resource(Files(install.search))
        .insert_resource(sounds)
        .insert_resource(Level(chamber))
        .add_plugins(ScenePlugin)
        .insert_resource(Config(settings))
        .insert_resource(portalgun::menu::SettingsPath(args.settings.clone()))
        .add_plugins(portalgun::menu::MenuPlugin)
        .insert_resource(GameIcon(game_icon))
        .add_systems(Update, icon::set_window_icon)
        .add_plugins(PlayerPlugin)
        .add_plugins(portalgun::objects::ObjectsPlugin)
        .add_plugins(portalgun::body::BodyPlugin)
        .insert_resource(GunMeshes(mdl, pieces))
        .add_plugins(ViewPlugin)
        .add_plugins(FacesPlugin)
        .add_plugins(FiringPlugin)
        .insert_resource(particles)
        .add_plugins(portalgun::effects::EffectsPlugin)
        .add_plugins(portalgun::gun_visual::GunVisualPlugin)
        .add_plugins(portalgun::impacts::PortalImpactsPlugin)
        .add_plugins(HudPlugin)
        .run();
    if exit.is_success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn refuse(reason: &str) -> ExitCode {
    let reason = reason.strip_prefix("refused: ").unwrap_or(reason);
    if bevy::log::tracing::dispatcher::has_been_set() {
        error!("[startup] refused: {reason}");
    } else {
        eprintln!("[startup] refused: {reason}");
    }
    ExitCode::from(2)
}

fn settings(path: &Path) -> Result<Settings, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Settings::parse(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            info!("[settings] file={} missing=defaults", path.display());
            Settings::parse("")
        }
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

#[cfg(target_os = "linux")]
fn desktop_entry(bmp: &portal::bmp::Bmp) {
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".local/share")));
    let (Some(data_home), Ok(exe)) = (data_home, std::env::current_exe()) else {
        info!("[icon] desktop_entry=skipped reason=no data home or executable path");
        return;
    };
    match icon::write_desktop_entry(&data_home, bmp, &exe) {
        Ok(changed) => info!(
            "[icon] desktop_entry={} changed={changed}",
            data_home.display()
        ),
        Err(e) => info!("[icon] desktop_entry=failed reason={e}"),
    }
}

#[cfg(not(target_os = "linux"))]
fn desktop_entry(_bmp: &portal::bmp::Bmp) {}
