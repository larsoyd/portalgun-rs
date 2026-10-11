// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::Path;

use bevy::audio::Volume;
use bevy::prelude::*;
use portal::install::{root, Install};
use portalgun::sound::{Files, Sounds};

fn app() -> App {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    let install = Install::open(&dir).unwrap_or_else(|e| panic!("install at {dir:?}: {e}"));
    let sounds = Sounds::read(&install.search).unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<AudioSource>()
        .insert_resource(Files(install.search))
        .insert_resource(sounds);
    app
}

fn play(app: &mut App, name: &'static str) -> Option<Entity> {
    app.world_mut()
        .run_system_cached_with(
            |In(name): In<&str>,
             mut commands: Commands,
             mut sounds: ResMut<Sounds>,
             files: Res<Files>,
             mut audio: ResMut<Assets<AudioSource>>| {
                sounds.play(&mut commands, &files, &mut audio, name)
            },
            name,
        )
        .unwrap()
}

#[test]
fn a_scripted_sound_plays_its_wave_at_the_scripts_volume_and_pitch() {
    let mut app = app();
    let played = play(&mut app, "PortalPlayer.EnterPortal").expect("enter sound plays");
    app.update();
    let world = app.world();
    let settings = world.get::<PlaybackSettings>(played).unwrap();
    assert_eq!(settings.volume, Volume::Linear(0.5));
    assert!(
        (0.95..=1.05).contains(&settings.speed),
        "speed {}",
        settings.speed
    );
    let player = world.get::<AudioPlayer>(played).unwrap();
    let source = world
        .resource::<Assets<AudioSource>>()
        .get(&player.0)
        .unwrap();
    assert_eq!(&source.bytes[..4], b"RIFF");
}

#[test]
fn an_unknown_sound_plays_nothing() {
    let mut app = app();
    assert_eq!(play(&mut app, "Portal.NoSuchSound"), None);
    app.update();
    let mut players = app.world_mut().query::<&AudioPlayer>();
    assert_eq!(players.iter(app.world()).count(), 0);
}

#[test]
fn each_wave_is_decoded_once() {
    let mut app = app();
    for _ in 0..8 {
        play(&mut app, "PortalPlayer.ExitPortal").unwrap();
    }
    app.update();
    assert!(app.world().resource::<Assets<AudioSource>>().len() <= 2);
    assert_eq!(
        app.world().resource::<Sounds>().decoded(),
        app.world().resource::<Assets<AudioSource>>().len()
    );
}

#[test]
fn the_woosh_loops_from_silence() {
    let mut app = app();
    let played = app
        .world_mut()
        .run_system_cached(
            |mut commands: Commands,
             mut sounds: ResMut<Sounds>,
             files: Res<Files>,
             mut audio: ResMut<Assets<AudioSource>>| {
                sounds.looped(&mut commands, &files, &mut audio, "PortalPlayer.Woosh", 0.0)
            },
        )
        .unwrap()
        .expect("the woosh script resolves");
    app.update();
    let settings = app.world().get::<PlaybackSettings>(played).unwrap();
    assert!(matches!(settings.mode, bevy::audio::PlaybackMode::Loop));
    assert_eq!(settings.volume, Volume::Linear(0.0));
}
