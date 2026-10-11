// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::movement::Punch;

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4)
}

#[test]
fn a_view_punch_adds_twenty_times_the_angle_to_the_punch_speed() {
    let mut p = Punch::default();
    p.kick([-1.0, 0.5, 0.0]);
    assert_eq!(p.velocity, [-20.0, 10.0, 0.0]);
    assert_eq!(p.angle, [0.0; 3]);
}

#[test]
fn a_tick_moves_damps_and_springs_the_punch_back() {
    let mut p = Punch::default();
    p.kick([-1.0, 0.0, 0.0]);
    p.decay(0.015);
    let damped = -20.0 * (1.0 - 9.0 * 0.015);
    assert!(close(p.angle, [-0.3, 0.0, 0.0]), "{:?}", p.angle);
    assert!(
        close(p.velocity, [damped + 65.0 * 0.015 * 0.3, 0.0, 0.0]),
        "{:?}",
        p.velocity
    );
    for _ in 0..400 {
        p.decay(0.015);
    }
    assert_eq!(p, Punch::default(), "settles to exactly zero");
}

#[test]
fn a_long_frame_stops_damping_and_caps_the_spring() {
    let mut p = Punch {
        angle: [1.0, 0.0, 0.0],
        velocity: [0.0; 3],
    };
    p.decay(0.2);
    assert!(close(p.velocity, [-2.0, 0.0, 0.0]), "{:?}", p.velocity);
    let mut q = Punch {
        angle: [0.0; 3],
        velocity: [10.0, 0.0, 0.0],
    };
    q.decay(0.2);
    assert!(
        close(q.velocity, [0.0 - 2.0 * 2.0, 0.0, 0.0]),
        "{:?}",
        q.velocity
    );
}

#[test]
fn punches_are_held_to_89_179_89() {
    let mut p = Punch {
        angle: [0.0; 3],
        velocity: [1.0e5, -1.0e5, 1.0e5],
    };
    p.decay(0.015);
    assert_eq!(p.angle, [89.0, -179.0, 89.0]);
}

#[test]
fn tiny_punches_snap_to_zero() {
    let mut p = Punch {
        angle: [0.02, 0.0, 0.0],
        velocity: [0.0, 0.02, 0.0],
    };
    p.decay(0.015);
    assert_eq!(p, Punch::default());
}

#[test]
fn a_rough_landing_sets_roll_and_holds_pitch_under_8() {
    let mut p = Punch {
        angle: [12.0, 3.0, 0.0],
        velocity: [1.0; 3],
    };
    p.land(400.0);
    assert!(close(p.angle, [8.0, 3.0, 5.2]), "{:?}", p.angle);
    assert_eq!(p.velocity, [1.0; 3]);
}
