// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::pair::Portal;
use portal::placement::{HALF_HEIGHT, HALF_WIDTH};
use portal::teleport::{
    box_meets_portal, entered, exit_velocity, fix_roll, in_front, keeps_leaving, meets_opening,
    nearer, touching, view_through, Mover, Reorient, RollFix, MAX_EXIT_SPEED,
    MIN_FLOOR_EXIT_PLAYER,
};
use portal::viewbob::{angle_vectors, Angles};

const X: [f32; 3] = [1.0, 0.0, 0.0];
const Y: [f32; 3] = [0.0, 1.0, 0.0];
const Z: [f32; 3] = [0.0, 0.0, 1.0];
const HALF: (f32, f32) = (HALF_WIDTH, HALF_HEIGHT);

fn len(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3)
}

fn wall() -> Portal {
    Portal::new([0.0; 3], X, Z, HALF)
}

fn floor() -> Portal {
    Portal::new([500.0, 0.0, 0.0], Z, X, HALF)
}

fn ceiling() -> Portal {
    Portal::new([900.0, 0.0, 400.0], [0.0, 0.0, -1.0], X, HALF)
}

#[test]
fn walking_out_of_a_floor_portal_gets_the_minimum_fling() {
    let v = exit_velocity(&wall(), &floor(), [-50.0, 0.0, 0.0], Mover::Player);
    assert!((v[2] - MIN_FLOOR_EXIT_PLAYER).abs() < 1e-3, "{v:?}");
    assert_eq!(MIN_FLOOR_EXIT_PLAYER, 300.0);
}

#[test]
fn objects_get_less_out_of_a_floor_portal() {
    let from_wall = exit_velocity(&wall(), &floor(), [-10.0, 0.0, 0.0], Mover::Object);
    assert!((from_wall[2] - 50.0).abs() < 1e-3, "{from_wall:?}");
    let from_floor = exit_velocity(&floor(), &floor(), [0.0, 0.0, -10.0], Mover::Object);
    assert!((from_floor[2] - 225.0).abs() < 1e-3, "{from_floor:?}");
}

/// An exit tipped up so its normal's height is exactly z.
fn tipped(z: f32) -> Portal {
    let h = (1.0 - z * z).sqrt();
    Portal::new([0.0; 3], [h, 0.0, z], [-z, 0.0, h], HALF)
}

#[test]
#[allow(clippy::approx_constant)]
fn an_exit_counts_as_a_floor_above_0_7071() {
    let slow = [-10.0, 0.0, 0.0];
    let above = exit_velocity(&wall(), &tipped(0.707103), slow, Mover::Player);
    assert!((above[2] - MIN_FLOOR_EXIT_PLAYER).abs() < 1e-3, "{above:?}");
    let at = exit_velocity(&wall(), &tipped(0.7071), slow, Mover::Player);
    assert!(at[2] < 10.0, "{at:?}");
}

#[test]
fn speed_is_carried_and_capped() {
    let v = exit_velocity(&floor(), &wall(), [0.0, 0.0, -900.0], Mover::Player);
    assert!(close(v, [900.0, 0.0, 0.0]), "{v:?}");
    let fast = exit_velocity(&floor(), &wall(), [0.0, 0.0, -9000.0], Mover::Object);
    assert!((len(fast) - MAX_EXIT_SPEED).abs() < 1e-2, "{fast:?}");
    assert_eq!(MAX_EXIT_SPEED, 1000.0);
    let down = exit_velocity(&floor(), &ceiling(), [0.0, 0.0, -100.0], Mover::Player);
    assert!(
        close(down, [0.0, 0.0, -100.0]),
        "a ceiling exit adds nothing {down:?}"
    );
}

#[test]
fn a_centre_behind_the_plane_inside_the_hole_has_entered() {
    let p = wall();
    let body = [20.0, 20.0, 60.0];
    assert!(entered(&p, [-2.0, 0.0, 0.0], body));
    assert!(!entered(&p, [2.0, 0.0, 0.0], body), "centre still in front");
    assert!(!entered(&p, [0.0, 0.0, 0.0], body), "centre on the plane");
    assert!(!entered(&p, [-2.0, 300.0, 0.0], body), "beside the opening");
    assert!(
        entered(&p, [-2.0, HALF_WIDTH + 10.0, 0.0], body),
        "body reaches into it"
    );
    assert!(
        !entered(&p, [-2.0, 0.0, HALF_HEIGHT + 100.0], body),
        "above it"
    );
    assert!(
        !entered(&p, [-1000.0, 0.0, 0.0], body),
        "far behind the wall"
    );
    assert!(
        !entered(&p, [-0.1, 0.0, 0.0], [0.1, 0.1, 0.1]),
        "only grazing the plane"
    );
}

#[test]
fn a_mover_that_skips_past_the_hole_in_one_step_has_not_entered() {
    let p = wall();
    assert!(!entered(&p, [-900.0, 0.0, 0.0], [5.0; 3]));
    assert!(entered(&p, [-400.0, 0.0, 0.0], [5.0; 3]));
}

#[test]
fn a_portal_claims_only_movers_whose_centre_is_strictly_in_front() {
    let p = wall();
    assert!(in_front(&p, [0.001, 0.0, 0.0]));
    assert!(!in_front(&p, [0.0, 0.0, 0.0]));
    assert!(!in_front(&p, [-0.001, 0.0, 0.0]));
}

#[test]
fn the_view_turns_with_the_portal() {
    let (f, u) = view_through(
        &wall(),
        &Portal::new([0.0, 500.0, 0.0], Y, Z, HALF),
        [-1.0, 0.0, 0.0],
        Z,
    );
    assert!(close(f, Y) && close(u, Z), "{f:?} {u:?}");
    let (f, u) = view_through(&floor(), &wall(), [0.0, 0.0, -1.0], X);
    assert!(close(f, X) && close(u, Z), "{f:?} {u:?}");
}

fn air(dt: f32) -> RollFix {
    RollFix {
        grounded: false,
        in_air: true,
        pitch_reorientation: false,
        coin: false,
        frametime: dt,
    }
}

fn tipped_view(pitch: f32, roll: f32) -> Angles {
    Angles {
        pitch,
        yaw: 30.0,
        roll,
    }
}

#[test]
fn a_level_view_clears_the_rate_and_is_not_upside_down() {
    let mut state = Reorient {
        rate: 77.0,
        upside_down: true,
    };
    let a = fix_roll(&mut state, tipped_view(10.0, 0.0), air(0.01));
    assert_eq!(a, tipped_view(10.0, 0.0));
    assert_eq!(state, Reorient::default());
}

#[test]
fn a_steep_roll_ramps_by_400_to_120_in_the_air_and_240_on_the_ground() {
    for (grounded, cap) in [(false, 120.0), (true, 240.0)] {
        let mut state = Reorient::default();
        let mut a = tipped_view(0.0, 90.0);
        a = fix_roll(
            &mut state,
            a,
            RollFix {
                grounded,
                ..air(0.1)
            },
        );
        assert!((state.rate - 40.0).abs() < 1e-3, "{}", state.rate);
        assert!((a.roll - 86.0).abs() < 1e-3, "{}", a.roll);
        for _ in 0..3 {
            a = fix_roll(
                &mut state,
                a,
                RollFix {
                    grounded,
                    ..air(0.1)
                },
            );
        }
        let ramp = (40.0_f32 + 120.0).min(cap);
        assert!(
            (state.rate - ramp).abs() < 1e-3,
            "grounded={grounded} {}",
            state.rate
        );
        for _ in 0..20 {
            a = fix_roll(
                &mut state,
                a,
                RollFix {
                    grounded,
                    ..air(0.1)
                },
            );
        }
        assert!(state.rate <= cap + 1e-3);
        assert_eq!(a.roll, 0.0);
        assert!(!state.upside_down);
    }
}

#[test]
fn a_mild_roll_settles_the_rate_toward_60() {
    let mut state = Reorient::default();
    fix_roll(&mut state, tipped_view(0.0, 30.0), air(0.1));
    assert!((state.rate - 40.0).abs() < 1e-3, "{}", state.rate);
    fix_roll(&mut state, tipped_view(0.0, 30.0), air(0.1));
    assert!((state.rate - 60.0).abs() < 1e-3, "{}", state.rate);
    state.rate = 200.0;
    let a = fix_roll(&mut state, tipped_view(0.0, -30.0), air(0.1));
    assert!((state.rate - 160.0).abs() < 1e-3, "{}", state.rate);
    assert!(
        (a.roll + 14.0).abs() < 1e-3,
        "negative roll comes back up {}",
        a.roll
    );
    let mut state = Reorient::default();
    for _ in 0..2 {
        fix_roll(&mut state, tipped_view(0.0, 45.0), air(0.1));
    }
    assert!(
        (state.rate - 80.0).abs() < 1e-3,
        "up.z 0.71 still ramps {}",
        state.rate
    );
}

#[test]
fn without_cl_reorient_in_air_an_airborne_roll_stays() {
    let mut state = Reorient::default();
    let how = RollFix {
        in_air: false,
        ..air(0.1)
    };
    let a = fix_roll(&mut state, tipped_view(0.0, 90.0), how);
    assert_eq!(a.roll, 90.0);
    assert_eq!(state.rate, 0.0);
    let a = fix_roll(
        &mut state,
        tipped_view(0.0, 90.0),
        RollFix {
            grounded: true,
            ..how
        },
    );
    assert!(a.roll < 90.0);
}

#[test]
fn exactly_180_takes_its_direction_from_the_coin() {
    for (coin, sign) in [(false, 1.0), (true, -1.0)] {
        let mut state = Reorient {
            rate: 100.0,
            upside_down: false,
        };
        let a = fix_roll(
            &mut state,
            tipped_view(0.0, 180.0),
            RollFix { coin, ..air(0.1) },
        );
        assert!(
            (a.roll - sign * 168.0).abs() < 1e-3,
            "coin={coin} {}",
            a.roll
        );
    }
}

#[test]
fn upside_down_and_looking_up_pitches_about_the_view_right_axis() {
    let mut state = Reorient::default();
    let before = tipped_view(-85.0, 178.0);
    let (_, right, _) = angle_vectors(before);
    let a = fix_roll(&mut state, before, air(0.1));
    assert_eq!(state.rate, 120.0);
    let (forward, after_right, up) = angle_vectors(a);
    let along = (0..3).map(|i| right[i] * after_right[i]).sum::<f32>();
    assert!(along > 0.999, "the right axis is kept {along}");
    let (old_forward, _, _) = angle_vectors(before);
    let expected = portal::viewbob::rotate_about(right, -12.0, old_forward);
    assert!(
        (0..3).all(|i| (forward[i] - expected[i]).abs() < 1e-3),
        "{forward:?} vs {expected:?}"
    );
    assert_eq!(state.upside_down, up[2] < 0.0);
}

#[test]
fn a_networked_pitch_reorientation_pitches_when_the_up_is_below_0_2() {
    let mut state = Reorient {
        rate: 100.0,
        upside_down: false,
    };
    let how = RollFix {
        pitch_reorientation: true,
        ..air(0.1)
    };
    let a = fix_roll(&mut state, tipped_view(0.0, 82.0), how);
    let (_, _, up) = angle_vectors(tipped_view(0.0, 82.0));
    assert!(up[2] > 0.1 && up[2] < 0.2);
    assert!(a.pitch != 0.0, "{a:?}");
    let b = fix_roll(&mut state, tipped_view(0.0, 40.0), how);
    assert!(b.roll < 40.0 && b.pitch.abs() < 1e-3, "{b:?}");
}

#[test]
fn only_movers_whose_box_meets_the_portal_box_are_touching() {
    let p = wall();
    let body = [20.0, 20.0, 60.0];
    assert!(touching(&p, [30.0, 0.0, 0.0], body));
    assert!(
        touching(&p, [64.0 + 10.0, 0.0, 0.0], body),
        "box reaches the trigger"
    );
    assert!(
        !touching(&p, [64.0 + 30.0, 0.0, 0.0], body),
        "beyond the trigger"
    );
    assert!(
        touching(&p, [-5.0, 0.0, 0.0], body),
        "the trigger box alone decides, a centre behind the plane still touches"
    );
    assert!(
        !touching(&p, [-25.0, 0.0, 0.0], body),
        "box behind the plane"
    );
    assert!(
        !touching(&p, [30.0, HALF_WIDTH + 30.0, 0.0], body),
        "beside it"
    );
    assert!(
        !touching(&p, [30.0, 0.0, HALF_HEIGHT + 70.0], body),
        "above it"
    );
}

#[test]
fn a_box_meets_the_opening_only_across_its_plane_inside_its_edges() {
    let p = wall();
    let ear = [10.0, 10.0, 32.0];
    assert!(meets_opening(&p, [5.0, 0.0, 0.0], ear));
    assert!(meets_opening(&p, [-5.0, 0.0, 0.0], ear));
    assert!(
        !meets_opening(&p, [15.0, 0.0, 0.0], ear),
        "in front, clear of the plane"
    );
    assert!(
        meets_opening(&p, [0.0, HALF_WIDTH + 5.0, 0.0], ear),
        "straddling the side edge"
    );
    assert!(
        !meets_opening(&p, [0.0, HALF_WIDTH + 15.0, 0.0], ear),
        "beside it"
    );
    assert!(
        !meets_opening(&p, [0.0, 0.0, HALF_HEIGHT + 40.0], ear),
        "above it"
    );
}

#[test]
fn a_box_meets_a_portal_only_where_it_straddles_its_two_triangles() {
    let p = wall();
    let half = [16.0, 16.0, 36.0];
    let at = |x: f32, y: f32, z: f32| [x, y, z];
    assert!(
        box_meets_portal(&p, at(0.0, 0.0, 0.0), half),
        "centred on the opening"
    );
    assert!(
        box_meets_portal(&p, at(15.9, 0.0, 0.0), half),
        "just reaching back through the plane"
    );
    assert!(
        !box_meets_portal(&p, at(16.1, 0.0, 0.0), half),
        "in front, not touching the plane"
    );
    assert!(
        !box_meets_portal(&p, at(-16.1, 0.0, 0.0), half),
        "behind it"
    );
    assert!(
        box_meets_portal(&p, at(0.0, 47.9, 0.0), half),
        "overlapping the side edge"
    );
    assert!(
        !box_meets_portal(&p, at(0.0, 48.1, 0.0), half),
        "beside the opening"
    );
    assert!(
        !box_meets_portal(&p, at(0.0, 0.0, 90.1), half),
        "above the opening"
    );
    assert!(
        box_meets_portal(&p, at(0.0, 0.0, 89.9), half),
        "overlapping the top edge"
    );
    const S: f32 = std::f32::consts::FRAC_1_SQRT_2;
    let tipped = Portal::new([0.0; 3], [S, 0.0, S], [-S, 0.0, S], HALF);
    assert!(box_meets_portal(&tipped, at(0.0, 0.0, 0.0), half));
    assert!(
        !box_meets_portal(&tipped, at(30.0, 0.0, 30.0), half),
        "in front of a tilted portal"
    );
    let (f, u) = (1.0 / 3.0_f32.sqrt(), 1.0 / 6.0_f32.sqrt());
    let skew = Portal::new([0.0; 3], [f, f, f], [-u, -u, 2.0 * u], HALF);
    let reach = (16.0 + 16.0 + 36.0) * f;
    let near = [0, 1, 2]
        .map(|i| skew.right[i] * -19.25 + skew.up[i] * 6.3 + skew.forward[i] * (reach + 0.25));
    assert!(
        !box_meets_portal(&skew, near, half),
        "a quarter unit in front of a skewed portal"
    );
    assert!(
        !box_meets_portal(&skew, at(47.72, 47.64, -58.44), half),
        "beside a skewed portal, separated only along an edge axis"
    );
}

#[test]
fn a_portal_takes_a_held_mover_only_when_strictly_nearer() {
    let a = wall();
    let mut b = wall();
    b.origin = [0.0, 100.0, 0.0];
    assert!(nearer([0.0, 49.0, 0.0], &a, &b));
    assert!(!nearer([0.0, 50.0, 0.0], &a, &b), "a tie keeps the holder");
    assert!(!nearer([0.0, 51.0, 0.0], &a, &b));
}

#[test]
#[allow(clippy::approx_constant)]
fn only_a_ceiling_portal_keeps_a_ducked_mover_behind_its_plane() {
    let ceiling = |z: f32| Portal::new([0.0; 3], [0.0, (1.0 - z * z).sqrt(), z], X, (32.0, 54.0));
    let below = |p: &Portal| p.world([-1.0, 0.0, 0.0]);
    let p = ceiling(-0.7072);
    assert!(keeps_leaving(&p, below(&p), true));
    assert!(!keeps_leaving(&p, below(&p), false), "not in a duck jump");
    assert!(
        !keeps_leaving(&p, p.world([0.0, 0.0, 0.0]), true),
        "on the plane"
    );
    let p = ceiling(-0.7071);
    assert!(!keeps_leaving(&p, below(&p), true), "not steep enough");
}
