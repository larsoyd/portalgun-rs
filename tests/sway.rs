use bevy::prelude::*;
use portal::pair::Portal;
use portal::viewbob::{angle_vectors, Angles};
use portalgun::view::eye_relative;
use portalgun::view::ViewMotion;
use portalgun::visual::VisualHistory;
use portalgun::walker::Walker;

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-4
}

#[test]
fn an_unmoved_gun_sits_at_the_eye() {
    let a = Angles {
        pitch: 20.0,
        yaw: 130.0,
        roll: 0.0,
    };
    let m = eye_relative([5.0, 6.0, 7.0], a, [5.0, 6.0, 7.0], a);
    assert!(m.abs_diff_eq(Mat4::IDENTITY, 1e-5), "{m:?}");
}

#[test]
fn the_gun_offset_is_in_view_space_ahead_left_up() {
    let a = Angles {
        pitch: 20.0,
        yaw: 130.0,
        roll: 0.0,
    };
    let (f, r, u) = angle_vectors(a);
    let eye = [5.0, 6.0, 7.0];
    let at = [0, 1, 2].map(|i| eye[i] + f[i] * 2.0 - r[i] * 3.0 + u[i] * 0.5);
    let m = eye_relative(eye, a, at, a);
    assert!(
        close(m.w_axis.truncate(), Vec3::new(2.0, 3.0, 0.5)),
        "{:?}",
        m.w_axis
    );
}

#[test]
fn a_gun_turned_left_turns_about_view_up() {
    let a = Angles {
        pitch: 0.0,
        yaw: 40.0,
        roll: 0.0,
    };
    let turned = Angles { yaw: 50.0, ..a };
    let m = eye_relative([0.0; 3], a, [0.0; 3], turned);
    let ahead = m.transform_vector3(Vec3::X);
    let r = 10f32.to_radians();
    assert!(close(ahead, Vec3::new(r.cos(), r.sin(), 0.0)), "{ahead:?}");
}

fn wall_pair() -> [Portal; 2] {
    [
        Portal::new(
            [0.0, 0.0, 64.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [500.0, 0.0, 64.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
    ]
}

#[test]
fn turning_resumes_as_soon_as_the_hull_clears_the_portal_plane() {
    let mut player = Walker::at([20.0, 0.0, 0.0]);
    let history = VisualHistory::new(&player);
    let mut motion = ViewMotion::default();
    motion.relative(&history.sample(&player, 1.0), 0.0, 0.01);
    player.yaw = 5.0;
    let offset = motion.relative(&history.sample(&player, 1.0), 0.01, 0.01);
    assert!(
        offset.w_axis.truncate().length() > 0.1,
        "turn response is still suppressed beyond the opening"
    );
}

#[test]
fn shaking_stays_continuous_on_entry_and_exit() {
    let pair = wall_pair();
    for hz in [60.0, 180.0] {
        let mut player = Walker::at([20.0, 0.0, 0.0]);
        let mut reference = Walker::at([100.0, 100.0, 0.0]);
        let reference_history = VisualHistory::new(&reference);
        let mut history = VisualHistory::new(&player);
        let mut crossing = ViewMotion::default();
        let mut control = ViewMotion::default();
        for i in 0..120 {
            reference.yaw = (i as f32 * 0.3).sin() * 30.0;
            player.yaw = reference.yaw;
            player.feet = match i {
                0..20 => [20.0, 0.0, 0.0],
                20..50 => [8.0, 0.0, 0.0],
                50..80 => [500.0, 8.0, 0.0],
                _ => [500.0, 20.0, 0.0],
            };
            if i >= 50 {
                let f = portal::pair::through_dir(&pair[0], &pair[1], reference.forward());
                player.yaw = f[1].atan2(f[0]).to_degrees();
            }
            if i == 50 {
                history.teleport(&player, &pair[0], &pair[1]);
            } else {
                history.advance(&player, false);
            }
            let actual = crossing.relative(&history.sample(&player, 1.0), i as f32 / hz, 1.0 / hz);
            let expected = control.relative(
                &reference_history.sample(&reference, 1.0),
                i as f32 / hz,
                1.0 / hz,
            );
            assert!(
                actual.abs_diff_eq(expected, 1e-4),
                "sway snapped at frame {i}, {hz} Hz: actual={actual:?}, expected={expected:?}"
            );
        }
    }
}

#[test]
fn teleporting_rotates_the_existing_turn_response_instead_of_restarting_it() {
    let pair = wall_pair();
    let mut player = Walker::at([100.0, 100.0, 0.0]);
    let mut history = VisualHistory::new(&player);
    let mut carried = ViewMotion::default();
    let mut control = ViewMotion::default();
    for i in 0..10 {
        player.yaw = i as f32 * 2.0;
        let visual = history.sample(&player, 1.0);
        for motion in [&mut carried, &mut control] {
            motion.relative(&visual, i as f32 / 180.0, 1.0 / 180.0);
        }
    }
    let expected = control.relative(&history.sample(&player, 1.0), 10.0 / 180.0, 1.0 / 180.0);
    let f = portal::pair::through_dir(&pair[0], &pair[1], player.forward());
    player.yaw = f[1].atan2(f[0]).to_degrees();
    player.feet = [500.0, 8.0, 0.0];
    history.teleport(&player, &pair[0], &pair[1]);
    let actual = carried.relative(&history.sample(&player, 1.0), 10.0 / 180.0, 1.0 / 180.0);
    assert!(expected.w_axis.truncate().length() > 0.1);
    assert!(
        actual.abs_diff_eq(expected, 1e-4),
        "teleport discarded the in-progress turn response"
    );
}
