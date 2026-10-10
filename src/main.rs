use std::path::Path;
use std::process::ExitCode;

use bevy::prelude::*;
use portal::install::{root, Install};
use portalgun::args::Args;
use portalgun::chamber::Chamber;
use portalgun::run::RunPlugin;
use portalgun::scene::{Level, ScenePlugin};
use portalgun::sound::{Files, Sounds};

fn main() -> ExitCode {
    let args = match Args::parse(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => return refuse(&e),
    };
    let home = std::env::var("HOME").unwrap_or_default();
    let dir = root(
        args.install.as_deref(),
        std::env::var("PORTAL_DIR").ok().as_deref(),
        Path::new(&home),
    );
    let started = std::time::Instant::now();
    let install = match Install::open(&dir) {
        Ok(i) => i,
        Err(e) => return refuse(&e.to_string()),
    };
    let sounds = match Sounds::read(&install.search) {
        Ok(s) => s,
        Err(e) => return refuse(&e.to_string()),
    };
    let chamber = match Chamber::test_chamber(&install.search) {
        Ok(c) => c,
        Err(e) => return refuse(&e.to_string()),
    };
    println!(
        "[startup] install={} checked_ms={}",
        install.root.display(),
        started.elapsed().as_millis()
    );
    let exit = App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "portalgun".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(RunPlugin {
            shots: args.shots,
            quit_after: args.quit_after,
            shot_dir: args.shot_dir,
        })
        .insert_resource(Files(install.search))
        .insert_resource(sounds)
        .insert_resource(Level(chamber))
        .add_plugins(ScenePlugin)
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn((
                Camera3d::default(),
                Transform::from_translation(portalgun::units::to_bevy([-320.0, -120.0, 64.0]))
                    .looking_at(portalgun::units::to_bevy([200.0, 100.0, 40.0]), Vec3::Y),
            ));
        })
        .run();
    if exit.is_success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn refuse(reason: &str) -> ExitCode {
    let reason = reason.strip_prefix("refused: ").unwrap_or(reason);
    eprintln!("[startup] refused: {reason}");
    ExitCode::from(2)
}
