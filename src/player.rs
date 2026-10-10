use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};

use crate::faces::Portals;
use crate::portals::Colour;
use crate::scene::Level;
use crate::script::{Action, Script};
use crate::settings::Settings;
use crate::sound::{Files, Sounds};
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

/// What the player asked for, from the keys or a script. Presses stay set until used.
#[derive(Resource, Default)]
pub struct Controls {
    pub forward: f32,
    pub side: f32,
    pub jump: bool,
    pub fire1: bool,
    pub fire2: bool,
}

#[derive(Resource)]
pub struct Scripted(pub Script);

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        let mut walker = Walker::at(SPAWN);
        walker.yaw = 20.0;
        app.insert_resource(Player(walker))
            .insert_resource(Time::<Fixed>::from_hz(TICK_HZ))
            .add_systems(Startup, (spawn_camera, grab_cursor))
            .init_resource::<Controls>()
            .add_systems(FixedUpdate, walk)
            .add_systems(
                Update,
                (
                    (read_keys, look).run_if(not(resource_exists::<Scripted>)),
                    run_script.run_if(resource_exists::<Scripted>),
                    place_camera,
                )
                    .chain(),
            );
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

fn read_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut controls: ResMut<Controls>,
) {
    let axis = |plus, minus| keys.pressed(plus) as i8 as f32 - keys.pressed(minus) as i8 as f32;
    controls.forward = axis(KeyCode::KeyW, KeyCode::KeyS);
    controls.side = axis(KeyCode::KeyD, KeyCode::KeyA);
    controls.jump |= keys.pressed(KeyCode::Space);
    controls.fire1 |= mouse.just_pressed(MouseButton::Left);
    controls.fire2 |= mouse.just_pressed(MouseButton::Right);
}

fn run_script(
    mut script: ResMut<Scripted>,
    time: Res<Time<Real>>,
    mut controls: ResMut<Controls>,
    mut player: ResMut<Player>,
) {
    for action in script.0.due(time.elapsed_secs()) {
        info!("[script] at={:.2} action={action:?}", time.elapsed_secs());
        match action {
            Action::Fire1 => controls.fire1 = true,
            Action::Fire2 => controls.fire2 = true,
            Action::Jump => controls.jump = true,
            Action::Look(yaw, pitch) => {
                player.yaw += yaw;
                player.pitch = (player.pitch + pitch).clamp(-89.0, 89.0);
            }
            Action::Move(forward, side) => (controls.forward, controls.side) = (forward, side),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn walk(
    mut commands: Commands,
    level: Res<Level>,
    time: Res<Time>,
    portals: Res<Portals>,
    mut controls: ResMut<Controls>,
    mut player: ResMut<Player>,
    mut sounds: ResMut<Sounds>,
    files: Res<Files>,
    mut audio: ResMut<Assets<AudioSource>>,
) {
    let input = Input {
        forward: controls.forward,
        side: controls.side,
        jump: std::mem::take(&mut controls.jump),
    };
    let Some(i) = player.step(&input, time.delta_secs(), &level, portals.linked()) else {
        return;
    };
    let from = Colour::ALL[i];
    info!(
        "[cross] from={from:?} to={:?} feet={:?} yaw={:.1} velocity={:?}",
        from.other(),
        player.feet.map(|v| v.round()),
        player.yaw,
        player.velocity.map(|v| v.round())
    );
    for name in ["PortalPlayer.EnterPortal", "PortalPlayer.ExitPortal"] {
        sounds.play(&mut commands, &files, &mut audio, name);
    }
}

fn place_camera(player: Res<Player>, mut camera: Single<&mut Transform, With<PlayerCamera>>) {
    **camera = Transform::from_translation(to_bevy(player.eye()))
        .looking_to(to_bevy_dir(player.forward()), Vec3::Y);
}
