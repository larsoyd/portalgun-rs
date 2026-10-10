use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};

use crate::scene::Level;
use crate::settings::Settings;
use crate::units::{to_bevy, to_bevy_dir};
use crate::walker::{Input, Walker};

/// Portal's server runs at 0.015 seconds a tick
const TICK_HZ: f64 = 1.0 / 0.015;
const SPAWN: [f32; 3] = [-320.0, -120.0, 0.0];

#[derive(Resource, Deref, DerefMut)]
pub struct Player(pub Walker);

#[derive(Resource)]
pub struct Config(pub Settings);

#[derive(Component)]
pub struct PlayerCamera;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        let mut walker = Walker::at(SPAWN);
        walker.yaw = 20.0;
        app.insert_resource(Player(walker))
            .insert_resource(Time::<Fixed>::from_hz(TICK_HZ))
            .add_systems(Startup, (spawn_camera, grab_cursor))
            .add_systems(FixedUpdate, walk)
            .add_systems(Update, (look, place_camera).chain());
    }
}

fn spawn_camera(mut commands: Commands, config: Res<Config>) {
    let msaa = match config.0.msaa {
        1 => Msaa::Off,
        2 => Msaa::Sample2,
        8 => Msaa::Sample8,
        _ => Msaa::Sample4,
    };
    commands.spawn((
        Camera3d::default(),
        PlayerCamera,
        msaa,
        Projection::Perspective(PerspectiveProjection {
            fov: config.0.vertical_fov(),
            ..default()
        }),
    ));
}

fn grab_cursor(mut cursor: Single<&mut CursorOptions>) {
    cursor.visible = false;
    cursor.grab_mode = CursorGrabMode::Locked;
}

fn look(motion: Res<AccumulatedMouseMotion>, config: Res<Config>, mut player: ResMut<Player>) {
    player.look(motion.delta.x, motion.delta.y, config.0.sensitivity);
}

fn walk(
    keys: Res<ButtonInput<KeyCode>>,
    level: Res<Level>,
    time: Res<Time>,
    mut player: ResMut<Player>,
) {
    let axis = |plus, minus| keys.pressed(plus) as i8 as f32 - keys.pressed(minus) as i8 as f32;
    let input = Input {
        forward: axis(KeyCode::KeyW, KeyCode::KeyS),
        side: axis(KeyCode::KeyD, KeyCode::KeyA),
        jump: keys.pressed(KeyCode::Space),
    };
    player.step(&input, time.delta_secs(), &level);
}

fn place_camera(player: Res<Player>, mut camera: Single<&mut Transform, With<PlayerCamera>>) {
    **camera = Transform::from_translation(to_bevy(player.eye()))
        .looking_to(to_bevy_dir(player.forward()), Vec3::Y);
}
