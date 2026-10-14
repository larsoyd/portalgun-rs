// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::ivp::{
    combine, damping, normal_impulse, rotation_steps, Motion, Rest, SleepCheck, TOLERANCE,
};

#[test]
fn collision_tolerance_is_a_quarter_unit_less_epsilon_in_metres() {
    assert_eq!(TOLERANCE, (0.25f32 - 1e-4) * 0.0254);
}

#[test]
fn linear_damping_is_linear_below_a_quarter_and_exponential_from_it() {
    let (_, cube) = damping([0.0; 3], 0.1, 1.0 / 66.0);
    assert!((cube - (1.0 - 0.1 / 66.0)).abs() < 1e-9, "{cube}");
    let (_, below) = damping([0.0; 3], 0.2499, 1.0);
    assert!((below - 0.7501).abs() < 1e-7, "{below}");
    let (_, at) = damping([0.0; 3], 0.25, 1.0);
    assert!((at - (-0.25f64).exp()).abs() < 1e-7, "{at}");
}

#[test]
fn angular_damping_switches_on_the_squared_length_of_all_three_axes() {
    let (below, _) = damping([0.4, 0.4, 0.4], 0.0, 1.0);
    assert!((below[0] - 0.6).abs() < 1e-6, "{below:?}");
    let (at, _) = damping([0.5, 0.5, 0.0], 0.0, 1.0);
    assert!((at[0] - (-0.5f32).exp()).abs() < 1e-6, "{at:?}");
    assert_eq!(at[2], 1.0);
}

#[test]
fn materials_combine_as_a_product_clamped_to_one() {
    assert!((combine(0.2, 0.2) - 0.04).abs() < 1e-7);
    assert_eq!(combine(2.0, 0.8), 1.0);
    assert_eq!(combine(-1.0, 0.5), 0.0);
}

#[test]
fn rotation_splits_into_substeps_past_a_sixth_of_a_radian() {
    assert_eq!(rotation_steps(10.0, 0.0166), 1);
    assert_eq!(rotation_steps(10.0, 0.0167), 3);
    assert_eq!(rotation_steps(100.0, 0.015), 18);
}

#[test]
fn normal_spring_pushes_only_while_the_gap_is_under_twice_the_tolerance() {
    let gap = 2.0 * TOLERANCE;
    assert_eq!(normal_impulse(0.0, gap, 40.0), 0.0);
    let close = normal_impulse(0.0, gap - 0.001, 40.0);
    assert!((close - 0.001 * 40.0).abs() < 1e-6, "{close}");
    let approaching = normal_impulse(0.5, gap, 40.0);
    assert!((approaching - 20.0).abs() < 1e-6, "{approaching}");
}

#[test]
fn normal_spring_pulls_twenty_times_harder_against_a_gap_past_twice_the_tolerance() {
    let gap = 2.0 * TOLERANCE + 0.01;
    assert_eq!(normal_impulse(0.19, gap, 1.0), 0.0);
    let held = normal_impulse(0.3, gap, 1.0);
    assert!((held - 0.1).abs() < 1e-5, "{held}");
}

fn still() -> Motion {
    Motion {
        position: [0.0; 3],
        rotation: [0.0, 0.0, 0.0, 1.0],
        previous_rotation: [0.0, 0.0, 0.0, 1.0],
        spin: [0.0; 3],
        radius: 0.88,
    }
}

#[test]
fn sleep_waits_out_the_window_before_calling_a_body_calm() {
    let mut check = SleepCheck::new(&still(), 0.0);
    assert_eq!(check.test(&still(), 0.3), Rest::Settling);
    assert_eq!(check.test(&still(), 0.31), Rest::Calm);
}

#[test]
fn sleep_restarts_the_window_after_a_centimetre_of_travel() {
    let mut check = SleepCheck::new(&still(), 0.0);
    let mut moved = still();
    moved.position[0] = 0.0101;
    assert_eq!(check.test(&moved, 0.5), Rest::Moving);
    assert_eq!(check.test(&moved, 0.6), Rest::Settling);
    let mut barely = still();
    barely.position[0] = 0.0099;
    let mut check = SleepCheck::new(&still(), 0.0);
    assert_eq!(check.test(&barely, 0.5), Rest::Calm);
}

#[test]
fn sleep_accepts_spin_under_three_quarter_pi_per_window() {
    let mut spinning = still();
    spinning.spin = [7.85, 0.0, 0.0];
    let turned = [0.0081f64.sin(), 0.0, 0.0, 0.0081f64.cos()];
    spinning.previous_rotation = turned;
    let mut check = SleepCheck::new(&still(), 0.0);
    assert_eq!(check.test(&spinning, 0.5), Rest::Calm);
    spinning.spin = [7.86, 0.0, 0.0];
    let mut check = SleepCheck::new(&still(), 0.0);
    assert_eq!(check.test(&spinning, 0.5), Rest::Moving);
}

#[test]
fn sleep_accepts_fast_spin_when_the_last_step_barely_turned() {
    let mut spinning = still();
    spinning.spin = [20.0, 0.0, 0.0];
    let half = 0.0040f64;
    spinning.previous_rotation = [half.sin(), 0.0, 0.0, half.cos()];
    let mut check = SleepCheck::new(&still(), 0.0);
    assert_eq!(check.test(&spinning, 0.5), Rest::Calm);
}

#[test]
fn sleep_restarts_the_window_after_the_orientation_turns() {
    let mut turned = still();
    let half = 0.0041f64;
    turned.rotation = [half.sin(), 0.0, 0.0, half.cos()];
    turned.previous_rotation = turned.rotation;
    let mut check = SleepCheck::new(&still(), 0.0);
    assert_eq!(check.test(&turned, 0.5), Rest::Moving);
    let mut check = SleepCheck::new(&still(), 0.0);
    turned.rotation = [0.0039f64.sin(), 0.0, 0.0, 0.0039f64.cos()];
    turned.previous_rotation = turned.rotation;
    assert_eq!(check.test(&turned, 0.5), Rest::Calm);
}

#[test]
fn sleep_accepts_a_slow_drift_held_inside_ten_centimetres_for_four_seconds() {
    let mut check = SleepCheck::new(&still(), 0.0);
    let mut drifting = still();
    let mut last = Rest::Moving;
    for i in 1..=41 {
        drifting.position[0] = if i % 2 == 0 { -0.011 } else { 0.011 };
        last = check.test(&drifting, 0.1 * i as f64);
        if i < 41 {
            assert_eq!(last, Rest::Moving, "step {i}");
        }
    }
    assert_eq!(last, Rest::Calm);
}
