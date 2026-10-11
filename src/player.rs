use bevy::camera::visibility::RenderLayers;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};

use portal::steps::{Step, Steps};
use portal::surfaces::{Surface, Surfaces};

use crate::faces::{Portals, MAIN_FACES};
use crate::portals::Colour;
use crate::scene::Level;
use crate::script::{Action, Script};
use crate::settings::Settings;
use crate::sound::{Files, Sounds};
use crate::visual::{VisualHistory, VisualPlayer};
use crate::walker::{Input, Walker};

/// Portal's server runs at 0.015 seconds a tick
const TICK_SECONDS: f64 = 0.015;
const SPAWN: [f32; 3] = [-320.0, -120.0, 0.0];
/// seconds between walker position lines
const WALK_LOG: f32 = 0.05;

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
    pub crouch: bool,
    pub jump_pressed: bool,
    pub fire1: bool,
    pub fire2: bool,
}

#[derive(Resource)]
pub struct Scripted(pub Script);

#[derive(Resource, Default)]
pub struct MovementResult(pub Option<usize>);

pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        let mut walker = Walker::at(SPAWN);
        walker.yaw = 20.0;
        let history = VisualHistory::new(&walker);
        app.insert_resource(history.sample(&walker, 0.0))
            .insert_resource(history)
            .insert_resource(Player(walker))
            .insert_resource(Time::<Fixed>::from_seconds(TICK_SECONDS))
            .init_resource::<Controls>()
            .init_resource::<MovementResult>()
            .add_systems(
                RunFixedMainLoop,
                (
                    (
                        read_keys.run_if(not(resource_exists::<Scripted>)),
                        look.run_if(not(resource_exists::<Scripted>)),
                        run_script.run_if(resource_exists::<Scripted>),
                    )
                        .chain()
                        .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
                    (interpolate, place_camera)
                        .chain()
                        .in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
                ),
            )
            .add_systems(FixedUpdate, simulate);
    }
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MovementPlugin)
            .init_resource::<crate::bodyanim::Gait>()
            .add_systems(Startup, (spawn_camera, grab_cursor, read_footing))
            .add_systems(FixedUpdate, walk.after(simulate));
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
        RenderLayers::from_layers(&[0, MAIN_FACES]),
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

fn look(
    motion: Res<AccumulatedMouseMotion>,
    config: Option<Res<Config>>,
    mut player: ResMut<Player>,
) {
    if let Some(config) = config {
        player.look(motion.delta.x, motion.delta.y, config.0.sensitivity);
    }
}

fn read_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut controls: ResMut<Controls>,
) {
    let axis = |plus, minus| keys.pressed(plus) as i8 as f32 - keys.pressed(minus) as i8 as f32;
    controls.forward = axis(KeyCode::KeyW, KeyCode::KeyS);
    controls.side = axis(KeyCode::KeyD, KeyCode::KeyA);
    controls.crouch = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    controls.jump = keys.pressed(KeyCode::Space);
    controls.jump_pressed |= keys.just_pressed(KeyCode::Space);
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
            Action::Jump => controls.jump_pressed = true,
            Action::JumpHeld(held) => {
                controls.jump_pressed |= held && !controls.jump;
                controls.jump = held;
            }
            Action::Crouch(held) => controls.crouch = held,
            Action::Look(yaw, pitch) => {
                player.yaw += yaw;
                player.pitch = (player.pitch + pitch).clamp(-89.0, 89.0);
            }
            Action::Move(forward, side) => (controls.forward, controls.side) = (forward, side),
        }
    }
}

fn simulate(
    level: Res<Level>,
    time: Res<Time<Fixed>>,
    portals: Res<Portals>,
    mut controls: ResMut<Controls>,
    mut player: ResMut<Player>,
    mut result: ResMut<MovementResult>,
    mut history: ResMut<VisualHistory>,
) {
    history.synchronize(&player);
    let pressed = std::mem::take(&mut controls.jump_pressed);
    let input = Input {
        forward: controls.forward,
        side: controls.side,
        jump: controls.jump || pressed,
        jump_pressed: pressed,
        crouch: controls.crouch,
    };
    result.0 = player.step(&input, time.delta_secs(), &level, portals.linked());
    if let (Some(i), Some(pair)) = (result.0, portals.linked()) {
        history.teleport(&player, &pair[i], &pair[1 - i]);
    } else {
        history.advance(&player, false);
    }
}

#[allow(clippy::too_many_arguments)]
fn walk(
    mut commands: Commands,
    time: Res<Time>,
    player: Res<Player>,
    result: Res<MovementResult>,
    mut sounds: ResMut<Sounds>,
    files: Res<Files>,
    mut audio: ResMut<Assets<AudioSource>>,
    mut logged: Local<f32>,
    footing: Option<ResMut<Footing>>,
    mut gait: ResMut<crate::bodyanim::Gait>,
) {
    let Some(mut footing) = footing else { return };
    if time.elapsed_secs() >= *logged + WALK_LOG {
        *logged = time.elapsed_secs();
        info!(
            "[walk] at={:.2} feet={:?} yaw={:.1} pitch={:.1} velocity={:?} grounded={} ducked={} eye={:.3}",
            *logged,
            player.feet.map(|v| (v * 10.0).round() / 10.0),
            player.yaw,
            player.pitch,
            player.velocity.map(|v| v.round()),
            player.grounded,
            player.duck.ducked,
            player.duck.eye
        );
    }
    if let Some(n) = player.ground {
        footing.last = n;
    }
    let surface = footing.surfaces[footing.last].clone();
    let mut heard = |what: &str, step: Step| {
        let name = match step.left {
            true => &surface.step_left,
            false => &surface.step_right,
        };
        info!("[step] what={what} name={name} volume={:.2}", step.volume);
        sounds.play_as(
            &mut commands,
            &files,
            &mut audio,
            name,
            Some(step.volume),
            None,
        );
    };
    if player.jumped {
        heard("jump", footing.steps.jump());
    }
    let landing = player.landed.and_then(|fall| footing.steps.land(fall));
    if let Some(l) = landing {
        heard("land", l.step);
    }
    let moved = (player.velocity, player.grounded);
    if let Some(s) = footing.steps.update_duck(
        time.delta_secs(),
        moved.0,
        moved.1,
        surface.material,
        player.duck.ducked,
    ) {
        gait.0 = Some(crate::bodyanim::Footfall {
            left: s.left,
            at: time.elapsed_secs(),
            interval: footing.steps.time / 1000.0,
        });
        heard("step", s);
    }
    if let Some((pitch, volume)) = landing.and_then(|l| l.recover) {
        sounds.play_as(
            &mut commands,
            &files,
            &mut audio,
            "PortalPlayer.FallRecover",
            Some(volume),
            Some(pitch as f32),
        );
    }
    let Some(i) = result.0 else {
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

/// What each chamber block sounds like underfoot, and the footstep state.
#[derive(Resource)]
pub struct Footing {
    surfaces: Vec<Surface>,
    steps: Steps,
    /// the block last stood on, the surface a jump pushes off
    last: usize,
}

fn read_footing(
    mut commands: Commands,
    files: Res<Files>,
    level: Res<Level>,
    mut player: ResMut<Player>,
    mut exit: MessageWriter<AppExit>,
) {
    match Surfaces::read(&files) {
        Ok(s) => {
            let surfaces: Vec<Surface> = level
                .blocks
                .iter()
                .map(|b| s.get(&b.surfaceprop).clone())
                .collect();
            info!(
                "[step] surfaces blocks={} floor={}",
                surfaces.len(),
                surfaces[0].step_left
            );
            player.set_surfaces(surfaces.clone());
            commands.insert_resource(Footing {
                surfaces,
                steps: Steps::default(),
                last: 0,
            });
        }
        Err(e) => {
            error!("[step] refused: surfaces reason={e}");
            exit.write(AppExit::error());
        }
    }
}

fn interpolate(
    player: Res<Player>,
    time: Res<Time<Fixed>>,
    mut history: ResMut<VisualHistory>,
    mut visual: ResMut<VisualPlayer>,
) {
    history.synchronize(&player);
    *visual = history.sample(&player, time.overstep_fraction());
}

fn place_camera(
    visual: Res<VisualPlayer>,
    portals: Res<Portals>,
    camera: Option<Single<&mut Transform, With<PlayerCamera>>>,
) {
    if let Some(mut camera) = camera {
        **camera = visual.portal_camera(portals.linked());
    }
}
