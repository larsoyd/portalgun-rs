use portal::placement::Surface;
use portalgun::chamber::{Block, Chamber};
use portalgun::walker::{Input, Walker};
const DT: f32 = 0.015;
fn block(min: [f32; 3], max: [f32; 3]) -> Block {
    Block {
        min,
        max,
        material: "",
        surface: Surface::Portalable,
        surfaceprop: "default".into(),
    }
}
fn floor() -> Chamber {
    Chamber {
        blocks: vec![block([-1000.0, -1000.0, -16.0], [1000.0, 1000.0, 0.0])],
    }
}
#[test]
fn a_fast_hull_hits_a_thin_wall_before_its_endpoint() {
    let c = Chamber {
        blocks: vec![block([0.0, -100.0, 0.0], [1.0, 100.0, 300.0])],
    };
    let mut w = Walker::at([-20.0, 0.0, 100.0]);
    w.velocity = [3500.0, 100.0, 0.0];
    w.step(&Input::default(), DT, &c, None);
    assert!((w.feet[0] + 16.03125).abs() < 0.001, "{:?}", w.feet);
    assert!(
        (w.feet[1] - 1.5).abs() < 0.001,
        "wall tangent survives: {:?}",
        w.feet
    );
    assert_eq!(w.velocity[0], 0.0);
}
#[test]
fn stepping_cannot_push_the_standing_hull_through_a_ceiling() {
    let mut c = floor();
    c.blocks
        .push(block([0.0, -100.0, 0.0], [100.0, 100.0, 18.0]));
    c.blocks
        .push(block([-100.0, -100.0, 80.0], [100.0, 100.0, 100.0]));
    let mut w = Walker::at([-20.0, 0.0, 0.0]);
    w.step(&Input::default(), DT, &c, None);
    for _ in 0..30 {
        w.step(
            &Input {
                forward: 1.0,
                ..Input::default()
            },
            DT,
            &c,
            None,
        );
    }
    assert!(
        w.feet[0] <= -16.0 && (w.feet[2] - 0.03125).abs() < 0.0001,
        "{:?}",
        w.feet
    );
}
#[test]
fn vertical_sequence_matches_the_binary_half_steps_at_fifteen_ms() {
    let c = floor();
    let mut w = Walker::at([0.0; 3]);
    w.step(&Input::default(), DT, &c, None);
    let mut apex: f32 = 0.0;
    let mut landed = 0;
    for tick in 1..=40 {
        w.step(
            &Input {
                jump: true,
                ..Input::default()
            },
            DT,
            &c,
            None,
        );
        if tick == 1 {
            assert!((w.feet[2] - 2.265).abs() < 0.0001);
            assert!((w.velocity[2] - 146.5).abs() < 0.0001);
        }
        apex = apex.max(w.eye()[2] - 64.0);
        if w.grounded {
            landed = tick;
            break;
        }
    }
    assert!((apex - 20.145).abs() < 0.001, "apex {apex}");
    assert_eq!(landed, 35);
}
#[test]
fn falling_in_free_space_matches_constant_acceleration() {
    let c = Chamber { blocks: vec![] };
    let mut w = Walker::at([0.0, 0.0, 500.0]);
    for _ in 0..20 {
        w.step(&Input::default(), DT, &c, None);
    }
    assert!((w.feet[2] - 473.0).abs() < 0.001);
    assert!((w.velocity[2] + 180.0).abs() < 0.001);
}

#[test]
fn ground_surface_scales_acceleration_friction_and_jump() {
    let c = floor();
    let mut w = Walker::at([0.0; 3]);
    w.set_surfaces(vec![portal::surfaces::Surface {
        friction: 0.4,
        jump_factor: 1.5,
        max_speed_factor: 0.5,
        ..Default::default()
    }]);
    w.step(&Input::default(), DT, &c, None);
    w.step(
        &Input {
            forward: 1.0,
            ..Input::default()
        },
        DT,
        &c,
        None,
    );
    assert!((w.velocity[0] - 5.625).abs() < 0.001);
    w.step(&Input::default(), DT, &c, None);
    assert!((w.velocity[0] - 2.625).abs() < 0.001);
    w.step(
        &Input {
            jump: true,
            ..Input::default()
        },
        DT,
        &c,
        None,
    );
    assert!((w.velocity[2] - 226.5).abs() < 0.001);
}

fn duck_input() -> Input {
    Input {
        crouch: true,
        ..Input::default()
    }
}
#[test]
fn crouch_uses_a_timed_eye_spline_then_switches_the_hull() {
    let c = floor();
    let mut w = Walker::at([0.0; 3]);
    w.step(&Input::default(), DT, &c, None);
    w.step(&duck_input(), DT, &c, None);
    assert_eq!(w.eye()[2], 64.0);
    for _ in 0..10 {
        w.step(&duck_input(), DT, &c, None);
    }
    assert!((w.eye()[2] - 52.609375).abs() < 0.001, "{:?}", w.eye());
    assert_eq!(w.height(), 72.0);
    for _ in 0..17 {
        w.step(&duck_input(), DT, &c, None);
    }
    assert_eq!(w.height(), 36.0);
    assert_eq!(w.eye()[2], 28.0);
    w.step(&Input::default(), DT, &c, None);
    assert_eq!(w.height(), 36.0);
    for _ in 0..14 {
        w.step(&Input::default(), DT, &c, None);
    }
    assert_eq!(w.height(), 72.0);
    assert_eq!(w.eye()[2], 64.0);
}
#[test]
fn releasing_ctrl_under_a_ceiling_waits_for_standing_clearance() {
    let mut c = floor();
    let mut w = Walker::at([0.0; 3]);
    w.step(&Input::default(), DT, &c, None);
    for _ in 0..28 {
        w.step(&duck_input(), DT, &c, None);
    }
    c.blocks
        .push(block([-100.0, -100.0, 40.0], [100.0, 100.0, 80.0]));
    for _ in 0..60 {
        w.step(&Input::default(), DT, &c, None);
    }
    assert_eq!(w.height(), 36.0);
    assert_eq!(w.eye()[2], 28.0);
    c.blocks.pop();
    for _ in 0..20 {
        w.step(&Input::default(), DT, &c, None);
    }
    assert_eq!(w.height(), 72.0);
}
#[test]
fn airborne_duck_raises_feet_by_the_full_hull_difference() {
    let c = Chamber { blocks: vec![] };
    let mut w = Walker::at([0.0, 0.0, 100.0]);
    w.step(&duck_input(), DT, &c, None);
    assert_eq!(w.height(), 36.0);
    assert!((w.feet[2] - 135.9325).abs() < 0.001);
    assert!((w.eye()[2] - 163.9325).abs() < 0.001);
    w.step(&Input::default(), DT, &c, None);
    assert_eq!(w.height(), 72.0);
    assert!((w.feet[2] - 99.73).abs() < 0.001);
}
#[test]
fn a_single_player_jump_automatically_ducks_on_the_following_tick() {
    let c = floor();
    let mut w = Walker::at([0.0; 3]);
    w.step(&Input::default(), DT, &c, None);
    w.step(
        &Input {
            jump: true,
            ..Input::default()
        },
        DT,
        &c,
        None,
    );
    assert_eq!(w.height(), 72.0);
    w.step(&Input::default(), DT, &c, None);
    assert_eq!(w.height(), 36.0);
    assert!((w.feet[2] - 40.395).abs() < 0.001, "{:?}", w.feet);
    assert!((w.eye()[2] - 68.395).abs() < 0.001);
}

#[test]
fn collision_skin_selects_the_first_plane_again_at_a_corner() {
    let tr = portalgun::collision::trace(
        [0.0; 3],
        [100.0, 1.0, 0.0],
        [0.0; 3],
        [0.0; 3],
        &[(0, [50.0, 0.51, -1.0], [60.0, 2.0, 1.0])],
    );
    assert!((tr.fraction - 0.4996875).abs() < 0.000001, "{tr:?}");
    assert_eq!(tr.normal, [-1.0, 0.0, 0.0]);
}
#[test]
fn floor_to_wall_teleport_forces_a_duck_and_recovers_when_clear() {
    use portal::pair::Portal;
    let c = floor();
    let pair = [
        Portal::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], (32.0, 54.0)),
        Portal::new(
            [300.0, 0.0, 200.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
    ];
    let mut w = Walker::at([0.0, 0.0, 100.0]);
    let mut crossed = false;
    for _ in 0..100 {
        if w.step(&Input::default(), DT, &c, Some(pair)).is_some() {
            crossed = true;
            break;
        }
    }
    assert!(crossed);
    assert_eq!(w.height(), 36.0);
    assert!(w.duck.in_duck_jump);
    let velocity = w.velocity[0];
    w.step(&Input::default(), DT, &c, Some(pair));
    assert_eq!(w.height(), 72.0);
    assert_eq!(w.velocity[0], velocity);
}

#[test]
fn a_fast_crossing_sweeps_the_remaining_motion_against_exit_geometry() {
    use portal::pair::Portal;
    let c = Chamber {
        blocks: vec![block([225.0, -100.0, 0.0], [226.0, 100.0, 300.0])],
    };
    let pair = [
        Portal::new(
            [0.0, 0.0, 100.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [200.0, 0.0, 100.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
    ];
    let mut w = Walker::at([-20.0, 0.0, 64.0]);
    w.velocity = [3500.0, 0.0, 0.0];
    assert_eq!(w.step(&Input::default(), DT, &c, Some(pair)), Some(0));
    assert!((w.feet[0] - 208.96875).abs() < 0.001, "{:?}", w.feet);
    assert_eq!(w.velocity[0], 0.0);
    w.step(&Input::default(), DT, &c, Some(pair));
    assert!(w.feet[0] < 209.0);
}

#[test]
fn low_ceiling_clips_a_jump_without_penetration() {
    let mut c = floor();
    c.blocks
        .push(block([-100.0, -100.0, 74.0], [100.0, 100.0, 90.0]));
    let mut w = Walker::at([0.0; 3]);
    w.step(&Input::default(), DT, &c, None);
    for _ in 0..80 {
        w.step(
            &Input {
                jump: true,
                ..Default::default()
            },
            DT,
            &c,
            None,
        );
        assert!(
            w.feet[2] + w.height() <= 74.0,
            "{:?} height {}",
            w.feet,
            w.height()
        );
    }
    assert!(w.grounded);
    assert_eq!(w.height(), 72.0);
}

#[test]
fn two_walls_stop_both_horizontal_components_without_stopping_fall() {
    let c = Chamber {
        blocks: vec![
            block([0.0, -100.0, 0.0], [1.0, 100.0, 300.0]),
            block([-100.0, 0.0, 0.0], [100.0, 1.0, 300.0]),
        ],
    };
    let mut w = Walker::at([-20.0, -20.0, 100.0]);
    w.velocity = [1000.0, 1000.0, -100.0];
    w.step(&Input::default(), DT, &c, None);
    assert!((w.feet[0] + 16.03125).abs() < 0.001);
    assert!((w.feet[1] + 16.03125).abs() < 0.001);
    assert_eq!(&w.velocity[..2], &[0.0, 0.0]);
    assert_eq!(w.velocity[2], -109.0);
    assert!(!w.grounded);
}

#[test]
fn start_solid_and_all_solid_are_distinct_and_contact_can_move_away() {
    use portalgun::collision::trace;
    let solids = [(0, [0.0; 3], [10.0; 3])];
    let leaving = trace([5.0; 3], [-5.0; 3], [0.0; 3], [0.0; 3], &solids);
    assert!(leaving.start_solid && !leaving.all_solid);
    let trapped = trace([5.0; 3], [6.0; 3], [0.0; 3], [0.0; 3], &solids);
    assert!(trapped.start_solid && trapped.all_solid);
    let away = trace(
        [0.0, 5.0, 5.0],
        [-10.0, 5.0, 5.0],
        [0.0; 3],
        [0.0; 3],
        &solids,
    );
    assert_eq!(away.fraction, 1.0);
    assert!(!away.start_solid);
}

#[test]
fn reversing_an_unfinished_duck_does_not_jump_the_eye_height() {
    let c = floor();
    let mut w = Walker::at([0.0; 3]);
    for _ in 0..11 {
        w.step(&duck_input(), DT, &c, None);
    }
    let before = w.eye()[2];
    w.step(&Input::default(), DT, &c, None);
    assert!((w.eye()[2] - before).abs() < 3.0);
    for _ in 0..20 {
        w.step(&Input::default(), DT, &c, None);
    }
    assert_eq!(w.height(), 72.0);
    assert_eq!(w.duck.eye, 64.0);
}

#[test]
fn an_obstructed_exit_cannot_place_the_player_inside_a_solid() {
    use portal::pair::Portal;
    let c = Chamber {
        blocks: vec![block([210.0, -100.0, 0.0], [220.0, 100.0, 300.0])],
    };
    let pair = [
        Portal::new(
            [0.0, 0.0, 100.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [200.0, 0.0, 100.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
    ];
    let mut w = Walker::at([-20.0, 0.0, 64.0]);
    w.velocity = [3500.0, 0.0, 0.0];
    assert_eq!(w.step(&Input::default(), DT, &c, Some(pair)), None);
    assert!(w.feet[0] < 0.0, "{:?}", w.feet);
    assert_eq!(w.velocity[0], 0.0);
}

#[test]
fn rotated_floor_portals_keep_their_full_length_for_jump_entries() {
    use portal::pair::Portal;
    for degrees in [0f32, 20.0, 45.0, 70.0, 90.0] {
        let (s, c) = degrees.to_radians().sin_cos();
        let pair = [
            Portal::new([0.0; 3], [0.0, 0.0, 1.0], [c, s, 0.0], (32.0, 54.0)),
            Portal::new(
                [300.0, 0.0, 200.0],
                [-1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
                (32.0, 54.0),
            ),
        ];
        // The entire 32-unit hull fits at this offset along the portal's long axis.
        let mut w = Walker::at([c * 25.0, s * 25.0, 0.0]);
        w.step(&Input::default(), DT, &floor(), None);
        w.step(
            &Input {
                jump: true,
                jump_pressed: true,
                ..Default::default()
            },
            DT,
            &floor(),
            None,
        );
        let mut crossed = false;
        for _ in 0..100 {
            if w.step(&Input::default(), DT, &floor(), Some(pair))
                .is_some()
            {
                crossed = true;
                break;
            }
        }
        assert!(crossed, "rotation={degrees} feet={:?}", w.feet);
    }
}

#[test]
fn rotated_openings_still_support_hulls_that_overlap_the_rim() {
    use portal::pair::Portal;
    for degrees in [20f32, 45.0, 70.0] {
        let (s, c) = degrees.to_radians().sin_cos();
        let pair = [
            Portal::new([0.0; 3], [0.0, 0.0, 1.0], [c, s, 0.0], (32.0, 54.0)),
            Portal::new(
                [300.0, 0.0, 200.0],
                [-1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
                (32.0, 54.0),
            ),
        ];
        let mut w = Walker::at([c * 50.0, s * 50.0, 50.0]);
        for _ in 0..100 {
            assert_eq!(w.step(&Input::default(), DT, &floor(), Some(pair)), None);
        }
        assert!(
            w.grounded && w.feet[2] >= 0.0,
            "rotation={degrees} feet={:?}",
            w.feet
        );
    }
}
