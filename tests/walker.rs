use std::path::Path;

use portal::install::{root, Install};
use portalgun::chamber::Chamber;
use portalgun::walker::{Input, Walker, EYE, HALF_WIDTH};

fn chamber() -> Chamber {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    let install = Install::open(&dir).unwrap_or_else(|e| panic!("install at {dir:?}: {e}"));
    Chamber::test_chamber(&install.search).unwrap()
}

fn run(w: &mut Walker, c: &Chamber, input: Input, seconds: f32) {
    for _ in 0..(seconds * 100.0) as usize {
        w.step(&input, 0.01, c);
    }
}

#[test]
fn a_walker_dropped_above_the_floor_lands_on_it() {
    let c = chamber();
    let mut w = Walker::at([0.0, 0.0, 100.0]);
    run(&mut w, &c, Input::default(), 2.0);
    assert!(w.feet[2].abs() < 1e-3, "{:?}", w.feet);
    assert!(w.grounded);
    assert_eq!(w.velocity[2], 0.0);
    assert_eq!(w.eye()[2], EYE);
}

#[test]
fn walking_into_a_wall_stops_at_the_hull_and_slides_along_it() {
    let c = chamber();
    let mut w = Walker::at([-300.0, 0.0, 0.0]);
    w.yaw = 180.0;
    let input = Input {
        forward: 1.0,
        side: 0.5,
        ..Input::default()
    };
    run(&mut w, &c, input, 3.0);
    assert!(
        (w.feet[0] - (-384.0 + HALF_WIDTH)).abs() < 1e-3,
        "{:?}",
        w.feet
    );
    assert!(w.feet[1] > 10.0, "slid along the wall to {:?}", w.feet);
}

#[test]
fn the_walker_can_stand_on_the_ledge_but_not_walk_through_it() {
    let c = chamber();
    let mut w = Walker::at([100.0, -160.0, 0.0]);
    let input = Input {
        forward: 1.0,
        ..Input::default()
    };
    run(&mut w, &c, input, 3.0);
    assert!(
        (w.feet[0] - (192.0 - HALF_WIDTH)).abs() < 1e-3,
        "{:?}",
        w.feet
    );
    let mut up = Walker::at([250.0, -160.0, 200.0]);
    run(&mut up, &c, Input::default(), 2.0);
    assert!((up.feet[2] - 128.0).abs() < 1e-3, "{:?}", up.feet);
}

#[test]
fn a_jump_leaves_the_ground_only_from_the_ground() {
    let c = chamber();
    let mut w = Walker::at([0.0, 0.0, 0.0]);
    run(&mut w, &c, Input::default(), 0.1);
    let jump = Input {
        jump: true,
        ..Input::default()
    };
    w.step(&jump, 0.01, &c);
    assert!(w.velocity[2] > 0.0 && !w.grounded);
    let rising = w.velocity[2];
    w.step(&jump, 0.01, &c);
    assert!(w.velocity[2] < rising, "no second jump in the air");
}

#[test]
fn the_mouse_turns_by_sources_yaw_and_pitch_factors() {
    let mut w = Walker::at([0.0, 0.0, 0.0]);
    w.look(100.0, 50.0, 3.0);
    assert!((w.yaw - (-6.6)).abs() < 1e-4, "{}", w.yaw);
    assert!((w.pitch - 3.3).abs() < 1e-4, "{}", w.pitch);
    w.look(0.0, 10000.0, 3.0);
    assert_eq!(w.pitch, 89.0);
    w.look(0.0, -20000.0, 3.0);
    assert_eq!(w.pitch, -89.0);
}

#[test]
fn hitting_a_wall_in_the_air_is_not_standing() {
    let c = chamber();
    let mut w = Walker::at([-360.0, 0.0, 200.0]);
    w.yaw = 180.0;
    let input = Input {
        forward: 1.0,
        ..Input::default()
    };
    for _ in 0..10 {
        w.step(&input, 0.01, &c);
    }
    assert!(
        (w.feet[0] - (-384.0 + HALF_WIDTH)).abs() < 1e-3,
        "{:?}",
        w.feet
    );
    assert!(!w.grounded);
}

#[test]
fn strafing_right_moves_to_the_right_of_the_view() {
    let c = chamber();
    let mut w = Walker::at([0.0, 0.0, 0.0]);
    w.yaw = 90.0;
    let input = Input {
        side: 1.0,
        ..Input::default()
    };
    run(&mut w, &c, input, 0.5);
    assert!(w.feet[0] > 80.0, "{:?}", w.feet);
    assert!(w.feet[1].abs() < 1e-3, "{:?}", w.feet);
}

#[test]
fn the_view_points_along_yaw_and_down_with_positive_pitch() {
    let close = |a: [f32; 3], b: [f32; 3]| {
        assert!((0..3).all(|i| (a[i] - b[i]).abs() < 1e-5), "{a:?} vs {b:?}")
    };
    let mut w = Walker::at([0.0; 3]);
    close(w.forward(), [1.0, 0.0, 0.0]);
    w.yaw = 90.0;
    close(w.forward(), [0.0, 1.0, 0.0]);
    w.pitch = 89.0;
    let f = w.forward();
    assert!(f[2] < -0.99, "{f:?}");
    w.pitch = -45.0;
    close(w.forward(), [0.0, 0.5f32.sqrt(), 0.5f32.sqrt()]);
}
