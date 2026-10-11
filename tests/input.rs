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

#[test]
fn visual_motion_advances_every_frame_at_60_120_and_180_hz() {
    use portalgun::player::PlayerCamera;
    use portalgun::visual::{VisualHistory, VisualPlayer};
    for hz in [60.0, 120.0, 180.0] {
        let mut app = app();
        app.add_plugins(bevy::transform::TransformPlugin);
        let camera = app
            .world_mut()
            .spawn((
                PlayerCamera,
                Transform::default(),
                GlobalTransform::default(),
            ))
            .id();
        let gun = app
            .world_mut()
            .spawn((
                Transform::from_xyz(0.1, -0.2, -0.3),
                GlobalTransform::default(),
                ChildOf(camera),
            ))
            .id();
        key(&mut app, KeyCode::KeyW, true);
        for _ in 0..30 {
            frame(&mut app, 1.0 / hz);
        }
        let mut previous = app.world().resource::<VisualPlayer>().pose.eye;
        for _ in 0..30 {
            frame(&mut app, 1.0 / hz);
            let visual = app.world().resource::<VisualPlayer>();
            let current = visual.pose.eye;
            assert!(
                ((current - previous).length() - 150.0 / hz as f32).abs() < 0.002,
                "hz={hz} delta={:?}",
                current - previous
            );
            let authoritative = app.world().resource::<Player>();
            let fraction = app.world().resource::<Time<Fixed>>().overstep_fraction();
            assert_eq!(
                current,
                app.world()
                    .resource::<VisualHistory>()
                    .sample(authoritative, fraction)
                    .pose
                    .eye
            );
            let camera_pose = app.world().get::<Transform>(camera).unwrap();
            assert_eq!(*camera_pose, visual.camera());
            let gun_pose = app.world().get::<GlobalTransform>(gun).unwrap();
            assert!(
                (gun_pose.translation() - camera_pose.transform_point(Vec3::new(0.1, -0.2, -0.3)))
                    .length()
                    < 1e-5
            );
            previous = current;
        }
        assert_eq!(
            app.world().resource::<Time<Fixed>>().timestep(),
            Duration::from_millis(15)
        );
    }
}

#[test]
fn zero_ticks_keep_endpoints_and_multiple_ticks_keep_only_the_latest_pair() {
    use portalgun::visual::VisualPlayer;
    let mut app = app();
    key(&mut app, KeyCode::KeyW, true);
    for _ in 0..20 {
        frame(&mut app, 0.015);
    }
    let before = app.world().resource::<Player>().eye()[0];
    frame(&mut app, 0.0075);
    assert_eq!(app.world().resource::<Player>().eye()[0], before);
    assert!((app.world().resource::<VisualPlayer>().pose.eye.x - (before - 1.125)).abs() < 0.001);
    frame(&mut app, 0.045);
    let after = app.world().resource::<Player>().eye()[0];
    assert!((after - before - 6.75).abs() < 0.001);
    assert!((app.world().resource::<VisualPlayer>().pose.eye.x - (after - 1.125)).abs() < 0.001);
}

#[test]
fn scripted_look_move_jump_and_fire_are_sampled_before_the_fixed_loop() {
    use portalgun::player::{Controls, Scripted};
    use portalgun::script::Script;
    use portalgun::visual::VisualPlayer;
    let mut app = app();
    app.insert_resource(Scripted(
        Script::parse("0 look 90 0\n0 move 1 0\n0 jump\n0 fire1\n").unwrap(),
    ));
    frame(&mut app, 0.005);
    assert_eq!(app.world().resource::<Player>().yaw, 90.0);
    assert_eq!(app.world().resource::<VisualPlayer>().angles.yaw, 90.0);
    assert!(app.world().resource::<Controls>().jump_pressed);
    assert!(app.world().resource::<Controls>().fire1);
    frame(&mut app, 0.010);
    let player = app.world().resource::<Player>();
    assert!(player.jumped);
    assert!(player.velocity[1] > 0.0 && player.velocity[0].abs() < 1e-4);
    assert!(!app.world().resource::<Controls>().jump_pressed);
    assert!(
        app.world().resource::<Controls>().fire1,
        "movement must not consume firing"
    );
}

#[test]
fn mouse_look_affects_this_ticks_movement_and_zero_tick_frames() {
    use bevy::input::mouse::MouseMotion;
    use portalgun::player::Config;
    use portalgun::settings::Settings;
    use portalgun::visual::VisualPlayer;
    let mut app = app();
    app.insert_resource(Config(Settings::parse("").unwrap()));
    key(&mut app, KeyCode::KeyW, true);
    app.world_mut().write_message(MouseMotion {
        delta: Vec2::new(-90.0 / (3.0 * 0.022), 0.0),
    });
    frame(&mut app, 0.015);
    let player = app.world().resource::<Player>();
    assert!(player.velocity[1] > 0.0 && player.velocity[0].abs() < 1e-4);
    let before = player.feet;
    app.world_mut().write_message(MouseMotion {
        delta: Vec2::new(20.0, 10.0),
    });
    frame(&mut app, 0.001);
    let player = app.world().resource::<Player>();
    assert_eq!(player.feet, before);
    assert!((player.yaw - 88.68).abs() < 0.001);
    assert_eq!(
        app.world().resource::<VisualPlayer>().angles.yaw,
        player.yaw
    );
}

#[test]
fn a_teleport_in_an_earlier_tick_of_the_frame_resets_visual_history() {
    use portal::pair::Portal;
    use portalgun::portals::Colour;
    use portalgun::visual::VisualPlayer;
    let mut app = app();
    let pair = [
        Portal::new(
            [0.0, 0.0, 100.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [500.0, 0.0, 100.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
    ];
    for (colour, portal) in Colour::ALL.into_iter().zip(pair) {
        app.world_mut()
            .resource_mut::<Portals>()
            .place(colour, portal);
    }
    app.world_mut().resource_mut::<Player>().0 = Walker::at([-5.0, 0.0, 64.0]);
    app.world_mut().resource_mut::<Player>().velocity = [1000.0, 0.0, 0.0];
    frame(&mut app, 0.045);
    let revision = app.world().resource::<VisualPlayer>().revision;
    assert!(revision > 0);
    assert!(app.world().resource::<VisualPlayer>().pose.eye.x >= 500.0);
    assert!(app.world().resource::<Player>().feet[0] > 500.0);
    frame(&mut app, 0.002);
    assert_eq!(app.world().resource::<VisualPlayer>().revision, revision);
    assert!(app.world().resource::<VisualPlayer>().pose.eye.x >= 500.0);
    app.world_mut().resource_mut::<Player>().0 = Walker::at([-100.0, 0.0, 0.0]);
    frame(&mut app, 0.001);
    assert_eq!(app.world().resource::<VisualPlayer>().pose.eye.x, -100.0);
    assert!(app.world().resource::<VisualPlayer>().revision > revision);
}

#[test]
fn floor_ceiling_loops_keep_the_rendered_eye_inside_the_room() {
    use portal::pair::Portal;
    use portalgun::player::PlayerCamera;
    use portalgun::portals::Colour;
    use portalgun::units::from_bevy;
    let mut app = app();
    let camera = app
        .world_mut()
        .spawn((PlayerCamera, Transform::default()))
        .id();
    for (colour, z, normal) in [(Colour::Blue, 0.0, 1.0), (Colour::Orange, 384.0, -1.0)] {
        app.world_mut().resource_mut::<Portals>().place(
            colour,
            Portal::new(
                [0.0, 0.0, z],
                [0.0, 0.0, normal],
                [1.0, 0.0, 0.0],
                (32.0, 54.0),
            ),
        );
    }
    app.world_mut().resource_mut::<Player>().0 = Walker::at([0.0, 0.0, 250.0]);
    let mut crossings = 0;
    let mut previous_z = 250.0;
    for i in 0..900 {
        frame(&mut app, 1.0 / 180.0);
        let player = app.world().resource::<Player>();
        crossings += usize::from(player.feet[2] > previous_z + 100.0);
        previous_z = player.feet[2];
        let eye = from_bevy(app.world().get::<Transform>(camera).unwrap().translation);
        assert!(
            (0.0..=384.0).contains(&eye[2]),
            "frame {i}: eye={eye:?}, feet={:?}",
            player.feet
        );
    }
    assert!(crossings > 3, "exercise repeated crossings");
}

#[test]
fn firing_on_a_zero_tick_frame_uses_authoritative_eye_and_current_look_once() {
    use portal::install::{root, Install};
    use portalgun::firing::{Firing, FiringPlugin};
    use portalgun::gun::tracer_origin;
    use portalgun::player::Controls;
    use portalgun::sound::{Files, Sounds};
    use portalgun::visual::VisualPlayer;
    let mut app = app();
    let dir = root(
        None,
        None,
        std::path::Path::new(&std::env::var("HOME").unwrap()),
    );
    let install = Install::open(&dir).unwrap();
    app.insert_resource(Sounds::read(&install.search).unwrap())
        .insert_resource(Files(install.search))
        .init_resource::<Assets<AudioSource>>()
        .add_plugins(FiringPlugin);
    key(&mut app, KeyCode::KeyW, true);
    for _ in 0..20 {
        frame(&mut app, 0.015);
    }
    app.world_mut().resource_mut::<Player>().yaw = 35.0;
    app.world_mut().resource_mut::<Controls>().fire1 = true;
    let expected = tracer_origin(app.world().resource::<Player>());
    let feet = app.world().resource::<Player>().feet;
    frame(&mut app, 0.002);
    assert_eq!(app.world().resource::<Player>().feet, feet);
    let eye = app.world().resource::<Player>().eye();
    assert_ne!(
        app.world().resource::<VisualPlayer>().pose.eye.to_array(),
        eye
    );
    assert_eq!(app.world().resource::<Firing>().flying()[0].from, expected);
    assert!(!app.world().resource::<Controls>().fire1);
    frame(&mut app, 0.045);
    assert_eq!(app.world().resource::<Firing>().flying().len(), 1);
}
