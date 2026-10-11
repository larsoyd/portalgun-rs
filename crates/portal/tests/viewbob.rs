// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::viewbob::{
    add_bob, angle_vectors, look_down, rotate_about, vector_angles, Angles, Bob, Lag,
};

fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

fn close3(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|i| close(a[i], b[i], 1e-5))
}

fn bob_at(speed: f32, last: f32, now: f32) -> Bob {
    let mut b = Bob {
        last,
        ..Bob::default()
    };
    b.calc([speed, 0.0, 0.0], now, 0.01);
    b
}

#[test]
fn bob_time_runs_with_speed_over_320() {
    let b = bob_at(100.0, 0.0, 0.5);
    assert_eq!(b.time, 0.15625);
    assert!(close(b.vertical, 0.4367032, 2e-6), "{}", b.vertical);
    assert!(close(b.lateral, 0.46045378, 2e-6), "{}", b.lateral);
    let later = bob_at(200.0, 1.0, 1.1);
    assert!(close(later.time, 0.062500015, 1e-7), "{}", later.time);
    assert!(
        close(later.vertical, 0.83623123, 2e-6),
        "{}",
        later.vertical
    );
    assert!(close(later.lateral, 0.5958328, 2e-6), "{}", later.lateral);
}

#[test]
fn bob_speed_is_held_to_320() {
    let fast = bob_at(400.0, 0.0, 0.5);
    let cap = bob_at(320.0, 0.0, 0.5);
    assert_eq!(fast.time, cap.time);
    assert_eq!(fast.vertical, cap.vertical);
    assert!(close(cap.vertical, 1.1999222, 2e-6), "{}", cap.vertical);
    assert!(close(cap.lateral, 0.09693752, 2e-6), "{}", cap.lateral);
}

#[test]
fn the_vertical_bob_turns_down_after_a_quarter_cycle() {
    assert!(close(bob_at(320.0, 0.0, 0.224).vertical, 0.4956374, 2e-6));
    assert!(close(bob_at(320.0, 0.0, 0.226).vertical, 0.46436185, 2e-6));
    assert!(close(bob_at(320.0, 0.0, 0.449).lateral, 0.48781887, 2e-6));
    assert!(close(bob_at(320.0, 0.0, 0.451).lateral, 0.47218147, 2e-6));
    assert!(close(bob_at(320.0, 0.0, 0.451).vertical, 0.49563786, 2e-6));
}

#[test]
fn a_paused_frame_leaves_the_bob_alone() {
    let mut b = bob_at(320.0, 0.0, 0.3);
    let before = (b.time, b.last, b.vertical);
    b.calc([100.0, 0.0, 0.0], 0.9, 0.0);
    assert_eq!((b.time, b.last, b.vertical), before);
}

#[test]
fn angle_vectors_follow_source() {
    let (f, r, u) = angle_vectors(Angles {
        pitch: 0.0,
        yaw: 90.0,
        roll: 0.0,
    });
    assert!(close3(f, [0.0, 1.0, 0.0]) && close3(r, [1.0, 0.0, 0.0]) && close3(u, [0.0, 0.0, 1.0]));
    let (f, _, _) = angle_vectors(Angles {
        pitch: 90.0,
        yaw: 0.0,
        roll: 0.0,
    });
    assert!(
        close3(f, [0.0, 0.0, -1.0]),
        "positive pitch looks down {f:?}"
    );
}

#[test]
fn the_bob_moves_and_tilts_the_gun() {
    let (o, a) = add_bob([0.0; 3], Angles::default(), 1.0, 1.0);
    assert!(close3(o, [0.1, -0.8, 0.1]), "{o:?}");
    assert!(
        close(a.pitch, -0.4, 1e-6) && close(a.yaw, -0.3, 1e-6) && close(a.roll, 0.5, 1e-6),
        "{a:?}"
    );
}

#[test]
fn the_gun_lags_behind_a_turn() {
    let mut lag = Lag {
        facing: [0.0, 1.0, 0.0],
    };
    let mut o = [0.0; 3];
    let mut a = Angles::default();
    lag.apply(&mut o, &mut a, Angles::default(), 1.5, 0.1);
    assert!(close3(o, [-5.0, 5.0, 0.0]), "{o:?}");
    let h = std::f32::consts::FRAC_1_SQRT_2;
    assert!(close3(lag.facing, [h, h, 0.0]), "{:?}", lag.facing);
}

#[test]
fn a_turn_past_the_lag_limit_is_caught_up_faster() {
    let mut lag = Lag {
        facing: [0.0, 1.0, 0.0],
    };
    let mut o = [0.0; 3];
    lag.apply(&mut o, &mut Angles::default(), Angles::default(), 0.5, 0.1);
    let d = 2f32.sqrt();
    let moved = [1.0 * d, 1.0 - d, 0.0];
    let n = (moved[0] * moved[0] + moved[1] * moved[1]).sqrt();
    assert!(
        close3(lag.facing, [moved[0] / n, moved[1] / n, 0.0]),
        "{:?}",
        lag.facing
    );
    let mut at_limit = Lag {
        facing: [0.0, 1.0, 0.0],
    };
    lag_once(&mut at_limit, d);
    assert!(
        close3(at_limit.facing, [h2(), h2(), 0.0]),
        "at the limit it chases at 5"
    );
}

fn h2() -> f32 {
    std::f32::consts::FRAC_1_SQRT_2
}

fn lag_once(lag: &mut Lag, max: f32) {
    lag.apply(
        &mut [0.0; 3],
        &mut Angles::default(),
        Angles::default(),
        max,
        0.1,
    );
}

#[test]
fn no_lag_while_touching_a_portal_keeps_only_the_pitch_push() {
    let mut lag = Lag {
        facing: [0.0, 1.0, 0.0],
    };
    let mut o = [1.0, 2.0, 3.0];
    let original = Angles {
        pitch: 350.0,
        yaw: 0.0,
        roll: 0.0,
    };
    let mut a = original;
    lag.apply(&mut o, &mut a, original, 0.0, 0.1);
    let (f, r, u) = angle_vectors(original);
    let p = -10.0f32;
    let want = [0, 1, 2].map(|i| {
        [1.0, 2.0, 3.0][i] + f[i] * (-p * 0.035) + r[i] * (-p * 0.03) + u[i] * (-p * 0.02)
    });
    assert!(close3(o, want), "{o:?} vs {want:?}");
    assert_eq!(a, original);
}

#[test]
fn a_paused_frame_does_not_chase_the_view() {
    let mut lag = Lag {
        facing: [0.0, 1.0, 0.0],
    };
    let mut o = [0.0; 3];
    lag.apply(&mut o, &mut Angles::default(), Angles::default(), 1.5, 0.0);
    assert_eq!(lag.facing, [0.0, 1.0, 0.0]);
    assert_eq!(o, [0.0; 3]);
}

#[test]
fn looking_down_pushes_the_gun_out() {
    let down = Angles {
        pitch: 30.0,
        yaw: 0.0,
        roll: 0.0,
    };
    let (f, r, u) = angle_vectors(down);
    let o = look_down([0.0; 3], down);
    let k = 0.25;
    let want = [0, 1, 2].map(|i| f[i] * 5.0 * k + r[i] * 4.7 * k + u[i] * 4.0 * k);
    assert!(close3(o, want), "{o:?} vs {want:?}");
    let level = look_down([1.0; 3], Angles::default());
    assert_eq!(level, [1.0; 3]);
    let up = look_down(
        [1.0; 3],
        Angles {
            pitch: -30.0,
            ..Angles::default()
        },
    );
    assert_eq!(up, [1.0; 3]);
}

#[test]
fn resetting_lag_does_not_pull_toward_the_preteleport_facing() {
    let mut lag = Lag {
        facing: [-1.0, 0.0, 0.0],
    };
    let angles = Angles {
        yaw: 90.0,
        ..Default::default()
    };
    lag.reset(angles);
    let mut origin = [0.0; 3];
    let mut turned = angles;
    lag.apply(&mut origin, &mut turned, angles, 1.5, 1.0 / 180.0);
    assert!(close3(origin, [0.0; 3]));
    assert!(close3(lag.facing, angle_vectors(angles).0));
}

#[test]
fn vector_angles_recovers_pitch_yaw_and_roll() {
    for pitch in [-80.0, -30.0, 0.0, 45.0, 85.0] {
        for yaw in [-170.0, -45.0, 0.0, 90.0, 135.0] {
            for roll in [-179.0, -90.0, -10.0, 0.0, 30.0, 120.0, 179.0] {
                let a = Angles { pitch, yaw, roll };
                let (f, _, u) = angle_vectors(a);
                let b = vector_angles(f, u);
                assert!(
                    close(a.pitch, b.pitch, 1e-2)
                        && close(a.yaw, b.yaw, 1e-2)
                        && close(a.roll, b.roll, 1e-2),
                    "{a:?} {b:?}"
                );
            }
        }
    }
    let straight_up = vector_angles([0.0, 0.0, 1.0], [-1.0, 0.0, 0.0]);
    assert!(
        close(straight_up.pitch, -90.0, 1e-3) && straight_up.roll == 0.0,
        "{straight_up:?}"
    );
    assert!(close(straight_up.yaw, 0.0, 1e-3), "{straight_up:?}");
}

#[test]
fn rotation_about_an_axis_is_right_handed() {
    assert!(close3(
        rotate_about([0.0, 0.0, 1.0], 90.0, [1.0, 0.0, 0.0]),
        [0.0, 1.0, 0.0]
    ));
    assert!(close3(
        rotate_about([1.0, 0.0, 0.0], -90.0, [0.0, 1.0, 0.0]),
        [0.0, 0.0, -1.0]
    ));
}
