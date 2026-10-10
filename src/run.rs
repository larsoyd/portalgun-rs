use std::path::PathBuf;

use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Capturing, Screenshot};

/// how long a quit waits on screenshots still being written
const CAPTURE_GRACE: f32 = 3.0;

#[derive(Resource)]
pub struct Timeline {
    shots: Vec<f32>,
    quit_after: Option<f32>,
}

impl Timeline {
    pub fn new(mut shots: Vec<f32>, quit_after: Option<f32>) -> Self {
        shots.sort_by(|a, b| b.total_cmp(a));
        Timeline { shots, quit_after }
    }

    pub fn shots_due(&mut self, now: f32) -> Vec<f32> {
        let mut due = Vec::new();
        while self.shots.last().is_some_and(|&t| t <= now) {
            due.extend(self.shots.pop());
        }
        due
    }

    pub fn should_quit(&self, now: f32) -> bool {
        self.quit_after.is_some_and(|t| now >= t)
    }
}

#[derive(Resource)]
struct ShotDir(PathBuf);

pub struct RunPlugin {
    pub shots: Vec<f32>,
    pub quit_after: Option<f32>,
    pub shot_dir: PathBuf,
}

impl Plugin for RunPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Timeline::new(self.shots.clone(), self.quit_after))
            .insert_resource(ShotDir(self.shot_dir.clone()))
            .add_systems(Update, (take_shots, quit_when_due));
    }
}

fn take_shots(
    mut commands: Commands,
    mut timeline: ResMut<Timeline>,
    dir: Res<ShotDir>,
    time: Res<Time<Real>>,
) {
    for at in timeline.shots_due(time.elapsed_secs()) {
        let path = dir.0.join(format!("shot-{:06.2}.png", at));
        info!(
            "[shot] at={at} now={:.2} path={}",
            time.elapsed_secs(),
            path.display()
        );
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
}

fn quit_when_due(
    timeline: Res<Timeline>,
    capturing: Query<(), With<Capturing>>,
    time: Res<Time<Real>>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    if !timeline.should_quit(now) {
        return;
    }
    match capturing.iter().count() {
        0 => {
            info!("[run] quit now={now:.2}");
            exit.write(AppExit::Success);
        }
        pending if timeline.should_quit(now - CAPTURE_GRACE) => {
            error!("[run] quit now={now:.2} shots_unwritten={pending} reason=capture still running after {CAPTURE_GRACE}s");
            exit.write(AppExit::error());
        }
        _ => {}
    }
}
