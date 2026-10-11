use std::path::Path;

use portal::install::{root, Install};
use portal::pair::Portal;
use portalgun::chamber::Chamber;
use portalgun::walker::{Input, Walker, EYE, HALF_WIDTH};

fn chamber() -> Chamber {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    let install = Install::open(&dir).unwrap_or_else(|e| panic!("install at {dir:?}: {e}"));
    Chamber::test_chamber(&install.search).unwrap()
}

fn run(w: &mut Walker, c: &Chamber, input: Input, seconds: f32) {
    for _ in 0..(seconds * 100.0) as usize {
        w.step(&input, 0.01, c, None);
    }
}

#[test]
fn a_walker_dropped_above_the_floor_lands_on_it() {
    let c = chamber();
    let mut w = Walker::at([0.0, 0.0, 100.0]);
    run(&mut w, &c, Input::default(), 2.0);
    assert!((w.feet[2] - 0.03125).abs() < 1e-3, "{:?}", w.feet);
    assert!(w.grounded);
    assert_eq!(w.velocity[2], 0.0);
    assert_eq!(w.eye()[2], EYE + 0.03125);
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
        (w.feet[0] - (-384.0 + HALF_WIDTH + 0.03125)).abs() < 1e-3,
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
        (w.feet[0] - (192.0 - HALF_WIDTH - 0.03125)).abs() < 1e-3,
        "{:?}",
        w.feet
    );
    let mut up = Walker::at([250.0, -160.0, 200.0]);
    run(&mut up, &c, Input::default(), 2.0);
    assert!((up.feet[2] - 128.03125).abs() < 1e-3, "{:?}", up.feet);
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
    w.step(&jump, 0.01, &c, None);
    assert!(w.velocity[2] > 0.0 && !w.grounded);
    let rising = w.velocity[2];
    w.step(&jump, 0.01, &c, None);
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
    for _ in 0..40 {
        w.step(&input, 0.01, &c, None);
    }
    assert!(
        (w.feet[0] - (-384.0 + HALF_WIDTH + 0.03125)).abs() < 1e-3,
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
    assert!(w.feet[0] > 60.0, "{:?}", w.feet);
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

fn pair() -> [Portal; 2] {
    [
        Portal::new(
            [-384.0, 0.0, 54.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [0.0, -256.0, 54.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
    ]
}

fn walk_west(w: &mut Walker, c: &Chamber, linked: Option<[Portal; 2]>) -> Option<usize> {
    let input = Input {
        forward: 1.0,
        ..Input::default()
    };
    let mut crossed = None;
    for _ in 0..200 {
        crossed = crossed.or(w.step(&input, 0.01, c, linked));
    }
    crossed
}

#[test]
fn walking_into_a_linked_wall_portal_comes_out_of_its_partner() {
    let c = chamber();
    let mut w = Walker::at([-340.0, 0.0, 0.0]);
    w.yaw = 180.0;
    assert_eq!(walk_west(&mut w, &c, Some(pair())), Some(0));
    assert!(
        w.feet[1] > -256.0 + HALF_WIDTH - 1e-3,
        "out in front of orange: {:?}",
        w.feet
    );
    assert!(w.feet[0].abs() < 20.0, "{:?}", w.feet);
    assert!(
        (w.yaw.rem_euclid(360.0) - 90.0).abs() < 1e-3,
        "faces out of orange: {}",
        w.yaw
    );
    assert!((w.feet[2] - 0.03125).abs() < 1e-3, "{:?}", w.feet);
}

#[test]
fn the_wall_beside_a_portal_still_stops_the_walker() {
    let c = chamber();
    let mut w = Walker::at([-340.0, 150.0, 0.0]);
    w.yaw = 180.0;
    assert_eq!(walk_west(&mut w, &c, Some(pair())), None);
    assert!(
        (w.feet[0] - (-384.0 + HALF_WIDTH + 0.03125)).abs() < 1e-3,
        "{:?}",
        w.feet
    );
}

#[test]
fn an_unlinked_portal_is_just_wall() {
    let c = chamber();
    let mut w = Walker::at([-340.0, 0.0, 0.0]);
    w.yaw = 180.0;
    assert_eq!(walk_west(&mut w, &c, None), None);
    assert!(
        (w.feet[0] - (-384.0 + HALF_WIDTH + 0.03125)).abs() < 1e-3,
        "{:?}",
        w.feet
    );
}

#[test]
fn the_crossing_keeps_height_and_pitch_between_upright_portals() {
    let c = chamber();
    let mut w = Walker::at([-340.0, 0.0, 0.0]);
    w.yaw = 180.0;
    w.pitch = 30.0;
    let input = Input {
        forward: 1.0,
        ..Input::default()
    };
    for _ in 0..200 {
        let z = w.feet[2];
        if w.step(&input, 0.01, &c, Some(pair())).is_some() {
            assert!((w.feet[2] - z).abs() < 1.0, "feet {} after {z}", w.feet[2]);
            assert!((w.pitch - 30.0).abs() < 1e-3, "pitch {}", w.pitch);
            return;
        }
    }
    panic!("never crossed, feet {:?}", w.feet);
}

#[test]
fn a_walker_in_the_opening_cannot_slide_into_the_wall_beside_it() {
    let c = chamber();
    let mut w = Walker::at([-380.0, 0.0, 0.0]);
    w.yaw = 180.0;
    let strafe = Input {
        side: 1.0,
        ..Input::default()
    };
    for _ in 0..300 {
        assert_eq!(w.step(&strafe, 0.01, &c, Some(pair())), None);
    }
    assert!(
        (w.feet[1] - (32.0 - HALF_WIDTH - 0.03125)).abs() < 1e-3,
        "stopped by the side of the opening: {:?}",
        w.feet
    );
    assert!((w.feet[0] - -380.0).abs() < 1e-3, "{:?}", w.feet);
}

#[test]
fn the_walker_steps_over_the_sill_under_a_raised_portal() {
    let c = chamber();
    let [mut blue, orange] = pair();
    blue.origin[2] = 64.0;
    let mut w = Walker::at([-340.0, 0.0, 0.0]);
    w.yaw = 180.0;
    assert_eq!(walk_west(&mut w, &c, Some([blue, orange])), Some(0));
}

#[test]
fn a_block_higher_than_a_step_still_stops_the_walker() {
    let c = chamber();
    let mut w = Walker::at([-200.0, 112.0, 0.0]);
    let input = Input {
        forward: 1.0,
        ..Input::default()
    };
    run(&mut w, &c, input, 1.0);
    assert!(
        (-160.0 - HALF_WIDTH - 0.03125..=-160.0 - HALF_WIDTH).contains(&w.feet[0]),
        "the 32 unit block stops it: {:?}",
        w.feet
    );
}

#[test]
fn a_walker_dropped_into_the_ledge_top_leaves_by_the_ledge_side() {
    let c = chamber();
    let top = Portal::new(
        [240.0, -194.0, 128.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        (32.0, 54.0),
    );
    let side = Portal::new(
        [192.0, -160.0, 74.0],
        [-1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    );
    let mut w = Walker::at([240.0, -194.0, 200.0]);
    let mut crossed = None;
    for _ in 0..200 {
        crossed = crossed.or(w.step(&Input::default(), 0.01, &c, Some([top, side])));
    }
    assert_eq!(
        crossed,
        Some(0),
        "taken by the portal it fell into, at {:?}",
        w.feet
    );
    assert!(
        w.feet[0] < 192.0,
        "out in front of the side portal: {:?}",
        w.feet
    );
}

#[test]
fn unducking_across_a_floor_portal_plane_still_teleports() {
    let c = chamber();
    let floor = Portal::new(
        [-250.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        (32.0, 54.0),
    );
    let ceiling = Portal::new(
        [0.0, -120.0, 384.0],
        [0.0, 0.0, -1.0],
        [0.0, 1.0, 0.0],
        (32.0, 54.0),
    );
    let mut w = Walker::at([-250.0, 0.0, -15.0]);
    w.duck.ducked = true;
    w.duck.eye = 28.0;
    assert!(w.centre()[2] > 0.0);
    assert_eq!(
        w.step(&Input::default(), 0.015, &c, Some([floor, ceiling])),
        Some(0)
    );
    assert!(!w.duck.ducked);
    assert!(w.feet[2] > 300.0 && w.centre()[2] < 384.0);
    assert!((w.feet[1] + 120.0).abs() < 0.01);
}

#[test]
fn the_walker_reports_its_jump_landing_speed_and_ground() {
    let c = chamber();
    let mut w = Walker::at([0.0, 0.0, 100.0]);
    let mut landed = None;
    for _ in 0..200 {
        w.step(&Input::default(), 0.015, &c, None);
        landed = landed.or(w.landed);
    }
    let fall = landed.expect("a landing after the drop");
    assert!(
        fall > 300.0 && fall < 400.0,
        "a 100 unit drop lands near 346: {fall}"
    );
    assert_eq!(w.landed, None, "only on the landing tick");
    let floor = w.ground.expect("standing on a block");
    assert_eq!(c.blocks[floor].min[2], -16.0, "the floor block");
    w.step(
        &Input {
            jump: true,
            ..Input::default()
        },
        0.015,
        &c,
        None,
    );
    assert!(w.jumped);
    w.step(
        &Input {
            jump: true,
            ..Input::default()
        },
        0.015,
        &c,
        None,
    );
    assert!(!w.jumped, "no jump from the air");
    assert_eq!(w.ground, None);
}
