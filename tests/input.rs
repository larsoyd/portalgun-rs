use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonState, InputPlugin};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use portalgun::chamber::{Block, Chamber};
use portalgun::faces::Portals;
use portalgun::player::{MovementPlugin, Player};
use portalgun::scene::Level;
use portalgun::walker::Walker;
use std::time::Duration;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, InputPlugin))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<Portals>()
        .insert_resource(Level(Chamber {
            blocks: vec![Block {
                min: [-1000.0, -1000.0, -16.0],
                max: [1000.0, 1000.0, 0.0],
                material: "",
                surface: portal::placement::Surface::Portalable,
                surfaceprop: "default".into(),
            }],
        }))
        .add_plugins(MovementPlugin);
    app.world_mut().resource_mut::<Player>().0 = Walker::at([0.0; 3]);
    frame(&mut app, 0.0);
    frame(&mut app, 0.015);
    app
}

fn frame(app: &mut App, dt: f64) {
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        dt,
    )));
    app.update();
}

#[derive(Resource, Default)]
struct Jumps(usize);
fn count(player: Res<Player>, mut jumps: ResMut<Jumps>) {
    jumps.0 += usize::from(player.jumped);
}

#[test]
fn held_space_never_repeats_across_zero_one_or_multiple_fixed_ticks() {
    for dt in [0.005, 0.015, 0.045] {
        let mut app = app();
        app.init_resource::<Jumps>()
            .add_systems(FixedPostUpdate, count);
        key(&mut app, KeyCode::Space, true);
        for _ in 0..(3.0 / dt) as usize {
            frame(&mut app, dt);
        }
        assert_eq!(app.world().resource::<Jumps>().0, 1, "render dt={dt}");
        assert!(app.world().resource::<Player>().grounded);
    }
}

#[test]
fn a_press_and_release_between_fixed_ticks_is_delivered_once() {
    let mut app = app();
    app.init_resource::<Jumps>()
        .add_systems(FixedPostUpdate, count);
    key(&mut app, KeyCode::Space, true);
    frame(&mut app, 0.002);
    key(&mut app, KeyCode::Space, false);
    frame(&mut app, 0.002);
    for _ in 0..100 {
        frame(&mut app, 0.015);
    }
    assert_eq!(app.world().resource::<Jumps>().0, 1);
}

fn key(app: &mut App, code: KeyCode, pressed: bool) {
    app.world_mut().write_message(KeyboardInput {
        key_code: code,
        logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
        state: if pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        },
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
}

#[test]
fn ctrl_changes_the_hull_through_the_real_keyboard_schedule() {
    for dt in [0.005, 0.015, 0.045] {
        let mut app = app();
        key(&mut app, KeyCode::ControlLeft, true);
        for _ in 0..(0.6 / dt) as usize {
            frame(&mut app, dt);
        }
        let player = app.world().resource::<Player>();
        assert_eq!(player.height(), 36.0);
        assert_eq!(player.eye()[2] - player.feet[2], 28.0);
        key(&mut app, KeyCode::ControlLeft, false);
        for _ in 0..(0.3 / dt) as usize {
            frame(&mut app, dt);
        }
        assert_eq!(app.world().resource::<Player>().height(), 72.0);
    }
}

#[derive(Resource, Default)]
struct Tape(Vec<([f32; 3], [f32; 3], bool)>);
fn record(player: Res<Player>, mut tape: ResMut<Tape>) {
    tape.0
        .push((player.feet, player.velocity, player.duck.ducked));
}
#[test]
fn the_fixed_trajectory_is_identical_at_thirty_sixty_and_144_render_fps() {
    let mut reference = None;
    for dt in [1.0 / 30.0, 1.0 / 60.0, 1.0 / 144.0] {
        let mut app = app();
        app.init_resource::<Tape>()
            .add_systems(FixedPostUpdate, record);
        key(&mut app, KeyCode::Space, true);
        key(&mut app, KeyCode::KeyW, true);
        while app.world().resource::<Tape>().0.len() < 120 {
            frame(&mut app, dt);
        }
        let tape = app.world().resource::<Tape>().0[..120].to_vec();
        if let Some(previous) = &reference {
            assert_eq!(&tape, previous, "render dt={dt}");
        } else {
            reference = Some(tape);
        }
    }
}

#[test]
fn release_and_repress_between_ticks_is_a_real_new_jump() {
    let mut app = app();
    app.init_resource::<Jumps>()
        .add_systems(FixedPostUpdate, count);
    key(&mut app, KeyCode::Space, true);
    for _ in 0..100 {
        frame(&mut app, 0.015);
    }
    assert!(app.world().resource::<Player>().grounded);
    key(&mut app, KeyCode::Space, false);
    frame(&mut app, 0.002);
    key(&mut app, KeyCode::Space, true);
    frame(&mut app, 0.002);
    frame(&mut app, 0.015);
    assert_eq!(app.world().resource::<Jumps>().0, 2);
}

#[test]
fn held_jump_waits_until_uncrouching_allows_the_jump() {
    let mut app = app();
    app.init_resource::<Jumps>()
        .add_systems(FixedPostUpdate, count);
    key(&mut app, KeyCode::ControlLeft, true);
    for _ in 0..40 {
        frame(&mut app, 0.015);
    }
    assert_eq!(app.world().resource::<Player>().height(), 36.0);
    key(&mut app, KeyCode::ControlLeft, false);
    key(&mut app, KeyCode::Space, true);
    frame(&mut app, 0.015);
    assert_eq!(app.world().resource::<Jumps>().0, 0);
    for _ in 0..30 {
        frame(&mut app, 0.015);
    }
    assert_eq!(app.world().resource::<Jumps>().0, 1);
}
