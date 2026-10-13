// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::carry::{
    carry_pitch, hold_distance, pickup_eligible, throw_impulse, Carry, RigidCube, IDENTITY,
};
use portal::pair::{through_dir, through_point, Portal};

fn pair() -> [Portal; 2] {
    [
        Portal::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], (32.0, 54.0)),
        Portal::new(
            [100.0, 0.0, 100.0],
            [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0],
            (32.0, 54.0),
        ),
    ]
}

#[test]
fn pickup_checks_original_mass_and_size_boundaries() {
    assert!(pickup_eligible(85.0, [128.0; 3], true));
    assert!(!pickup_eligible(85.001, [40.0; 3], true));
    assert!(!pickup_eligible(40.0, [128.001, 40.0, 40.0], true));
    assert!(!pickup_eligible(40.0, [40.0; 3], false));
}

#[test]
fn carrying_clamps_pitch_to_seventy_five_degrees() {
    assert_eq!(carry_pitch(-90.0), -75.0);
    assert_eq!(carry_pitch(90.0), 75.0);
    assert_eq!(carry_pitch(74.9), 74.9);
}

#[test]
fn hold_target_retreats_at_the_original_half_trace_boundary() {
    assert_eq!(hold_distance(40.0, 1.0), 64.0);
    assert_eq!(hold_distance(40.0, 0.5), 64.0);
    assert_eq!(hold_distance(40.0, 0.4999), 20.0);
}

#[test]
fn throw_force_uses_clamped_mass_and_original_remap() {
    assert_eq!(throw_impulse(0.25), 500.0);
    assert_eq!(throw_impulse(0.5), 500.0);
    assert!((throw_impulse(15.0) - 4000.0).abs() < 0.001);
    assert_eq!(throw_impulse(40.0), throw_impulse(15.0));
}

#[test]
fn remote_hold_transforms_position_and_all_orientation_axes() {
    let p = pair();
    let c = Carry::new(IDENTITY, 0.0, Some(0));
    let (at, basis) = c
        .target([20.0, 0.0, 20.0], [-1.0, 0.0, 0.0], 60.0, 0.0, Some(&p))
        .unwrap();
    assert_eq!(at, through_point(&p[0], &p[1], [-40.0, 0.0, 20.0]));
    assert_eq!(basis, IDENTITY.map(|v| through_dir(&p[0], &p[1], v)));
    assert!(c
        .target([0.0; 3], [1.0, 0.0, 0.0], 60.0, 0.0, None)
        .is_none());
}

#[test]
fn separate_player_and_object_crossings_keep_the_held_side() {
    let mut c = Carry::new(IDENTITY, 0.0, None);
    c.object_crossed(0);
    assert_eq!(c.portal, Some(0));
    c.player_crossed(0);
    assert_eq!(c.portal, None);
    c.player_crossed(1);
    assert_eq!(c.portal, Some(0));
    c.object_crossed(1);
    assert_eq!(c.portal, None);
}

#[test]
fn remote_error_is_delayed_then_grows_when_looking_away() {
    let mut c = Carry::new(IDENTITY, 0.0, Some(0));
    assert!(!c.update_error(100.0, 0.5, false));
    assert!(!c.update_error(100.0, 0.5, false));
    assert!(c.update_error(100.0, 0.1, false));
}

#[test]
fn rigid_crossing_rotates_orientation_and_angular_velocity() {
    let p = pair();
    let mut cube = RigidCube::at([-1.0, 0.0, 0.0]);
    cube.angular_velocity = [0.0, 0.0, 90.0];
    cube.teleport(&p[0], &p[1], 1);
    assert_eq!(cube.owner, Some(1));
    assert_eq!(
        cube.rotation,
        IDENTITY.map(|v| through_dir(&p[0], &p[1], v))
    );
    assert_eq!(
        cube.angular_velocity,
        through_dir(&p[0], &p[1], [0.0, 0.0, 90.0])
    );
}

#[test]
fn pickup_aligns_cube_axes_within_thirty_degrees() {
    let basis = IDENTITY.map(|v| portal::carry::yaw_rotate(v, 29.0));
    let carry = Carry::new(basis, 0.0, None);
    assert_eq!(carry.relative_rotation, IDENTITY);
    let basis = IDENTITY.map(|v| portal::carry::yaw_rotate(v, 31.0));
    let carry = Carry::new(basis, 0.0, None);
    for (actual, expected) in carry
        .relative_rotation
        .iter()
        .flatten()
        .zip(basis.iter().flatten())
    {
        assert!((actual - expected).abs() < 0.0001);
    }
}

#[test]
fn held_orientation_keeps_roll_but_ignores_look_pitch() {
    let carry = Carry::new(IDENTITY, 0.0, None);
    let a = portal::viewbob::Angles {
        yaw: 0.0,
        pitch: 60.0,
        roll: 90.0,
    };
    let (_, basis) = carry
        .target_angles([0.0; 3], [1.0, 0.0, 0.0], 60.0, a, None)
        .unwrap();
    assert!((basis[1][2] - 1.0).abs() < 0.0001);
    assert_eq!(basis[0], [1.0, 0.0, 0.0]);
    let velocity = portal::carry::rotation_velocity(IDENTITY, basis, 1.0);
    assert!((velocity[0] - 90.0).abs() < 0.001);
}

#[test]
fn diagonal_collision_creates_rotation_without_adding_energy() {
    let mut cube = RigidCube::at([0.0; 3]);
    cube.rotation = IDENTITY.map(|v| portal::carry::yaw_rotate(v, 30.0));
    cube.velocity = [-100.0, 0.0, 0.0];
    let before = portal::carry::dot(cube.velocity, cube.velocity) * cube.mass * 0.5;
    cube.contact([1.0, 0.0, 0.0], 0.25, 0.0);
    assert!(cube.angular_velocity[2].abs() > 0.0);
    let spin = cube.angular_velocity.map(f32::to_radians);
    let inertia = 2.0 / 3.0 * cube.mass * cube.half_size * cube.half_size;
    let after = portal::carry::dot(cube.velocity, cube.velocity) * cube.mass * 0.5
        + portal::carry::dot(spin, spin) * inertia * 0.5;
    assert!(after < before);
}

#[test]
fn carry_error_halves_samples_above_the_shadow_speed_limit() {
    let mut carry = Carry::new(IDENTITY, 0.0, None);
    carry.elapsed = 0.0;
    assert!(!carry.update_error(100.0, 0.05, true));
    assert_eq!(carry.error, 2.5);
    carry.error = 0.0;
    assert!(!carry.update_error(100.0, 0.1, true));
    assert_eq!(carry.error, 10.0);
    carry.error = 0.0;
    assert!(!carry.update_error(12.0, 1.0, true));
    assert_eq!(carry.error, 12.0);
    assert!(carry.update_error(12.001, 1.0, true));
}

#[test]
fn throw_torque_scales_three_original_random_axes_by_mass() {
    assert_eq!(
        portal::carry::throw_torque(0.5, [-10.0, 0.0, 10.0]),
        [-5.0, 0.0, 5.0]
    );
    let torque = portal::carry::throw_torque(40.0, [-10.0, 5.0, 10.0]);
    for (actual, expected) in torque.into_iter().zip([-40.0, 20.0, 40.0]) {
        assert!((actual - expected).abs() < 0.0001);
    }
}
