use bevy::prelude::*;
use portal::pair::Portal;
use portalgun::units::from_bevy;
use portalgun::visual::VisualHistory;
use portalgun::walker::Walker;

#[test]
fn interpolation_samples_eye_hull_and_velocity_at_endpoints_and_midpoint() {
    let mut player = Walker::at([10.0, 20.0, 30.0]);
    let mut history = VisualHistory::new(&player);
    let before = player.eye();
    player.feet = [30.0, 40.0, 50.0];
    player.velocity = [100.0, 20.0, -40.0];
    player.duck.eye = 28.0;
    player.duck.ducked = true;
    history.advance(&player, false);
    for fraction in [0.0, 0.5, 1.0] {
        let visual = history.sample(&player, fraction);
        assert_eq!(
            visual.pose.eye,
            Vec3::from(before).lerp(player.eye().into(), fraction)
        );
        assert_eq!(visual.pose.velocity, Vec3::from(player.velocity) * fraction);
        assert_eq!(visual.pose.half.z, 36.0 - 18.0 * fraction);
    }
    assert_eq!(player.feet, [30.0, 40.0, 50.0]);
}

#[test]
fn teleport_and_reset_discard_both_old_endpoints() {
    let mut player = Walker::at([0.0; 3]);
    let mut history = VisualHistory::new(&player);
    player.feet[0] = 10.0;
    history.advance(&player, false);
    player.feet = [1000.0, -500.0, 200.0];
    player.yaw = 179.0;
    history.advance(&player, true);
    for fraction in [0.0, 0.5, 1.0] {
        assert_eq!(
            history.sample(&player, fraction).pose.eye.to_array(),
            player.eye()
        );
    }
    let revision = history.revision;
    player = Walker::at([-300.0, 40.0, 0.0]);
    history.synchronize(&player);
    assert_eq!(history.revision, revision + 1);
    assert_eq!(
        history.sample(&player, 0.5).pose.eye.to_array(),
        player.eye()
    );
    history.synchronize(&player);
    assert_eq!(history.revision, revision + 1);
    player.feet[0] += 3.0;
    history.advance(&player, false);
    assert_eq!(history.sample(&player, 0.5).pose.eye.x, -298.5);
}

#[test]
fn look_is_immediate_and_wraps_without_a_long_turn() {
    let mut player = Walker::at([0.0; 3]);
    player.yaw = 179.0;
    let history = VisualHistory::new(&player);
    let before = history.sample(&player, 0.0).camera();
    player.yaw = -179.0;
    player.pitch = 20.0;
    for fraction in [0.0, 0.5, 1.0] {
        let visual = history.sample(&player, fraction);
        assert_eq!(visual.angles.yaw, -179.0);
        assert_eq!(visual.angles.pitch, 20.0);
        assert!(visual.camera().rotation.angle_between(before.rotation) < 21f32.to_radians());
    }
}

#[test]
fn ceiling_view_tracks_the_eye_even_after_the_crouched_hull_has_cleared() {
    let pair = [
        Portal::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], (32.0, 54.0)),
        Portal::new(
            [0.0, 0.0, 384.0],
            [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0],
            (32.0, 54.0),
        ),
    ];
    let mut player = Walker::at([0.0, 0.0, 340.0]);
    player.duck.ducked = true;
    player.duck.eye = 64.0;
    let history = VisualHistory::new(&player);
    let visual = history.sample(&player, 1.0);
    assert!(visual.pose.centre.z + visual.pose.half.z < 384.0);
    assert!(visual.pose.eye.z > 384.0);
    let camera = visual.portal_camera(Some(pair));
    assert!((from_bevy(camera.translation)[2] - 20.0).abs() < 1e-4);
    assert!(camera.rotation.abs_diff_eq(visual.camera().rotation, 1e-5));
    assert_eq!(visual.portal_camera(None), visual.camera());
    for x in [-100.0, 100.0] {
        player.feet[0] = x;
        let visual = VisualHistory::new(&player).sample(&player, 1.0);
        assert_eq!(visual.portal_camera(Some(pair)), visual.camera());
    }
}

#[test]
fn teleport_rotations_accumulate_across_fixed_ticks_and_external_resets_clear_them() {
    let pair = [
        Portal::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], (32.0, 54.0)),
        Portal::new(
            [500.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
    ];
    let player = Walker::at([0.0; 3]);
    let mut history = VisualHistory::new(&player);
    let mut forward = [1.0, 0.0, 0.0];
    for _ in 0..3 {
        history.teleport(&player, &pair[0], &pair[1]);
        history.advance(&player, false);
        forward = portal::pair::through_dir(&pair[0], &pair[1], forward);
    }
    let visual = history.sample(&player, 0.5);
    assert!((visual.teleport_rotation * Vec3::X).abs_diff_eq(forward.into(), 1e-5));
    assert_eq!(visual.reset_revision, 0);
    history.reset(&player);
    let visual = history.sample(&player, 0.5);
    assert_eq!(visual.reset_revision, 1);
    assert_eq!(visual.teleport_rotation, Quat::IDENTITY);
}
