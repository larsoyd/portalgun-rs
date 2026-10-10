use portal::pair::Portal;
use portal::placement::{HALF_HEIGHT, HALF_WIDTH};
use portal::teleport::{
    entered, exit_velocity, meets_opening, recover_roll, touching, view_through, Mover,
    MAX_EXIT_SPEED, MIN_FLOOR_EXIT_PLAYER,
};

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
    assert!(entered(
        &p,
        [10.0, 0.0, 0.0],
        [-2.0, 0.0, 0.0],
        [20.0, 20.0, 60.0]
    ));
    assert!(
        !entered(&p, [10.0, 0.0, 0.0], [2.0, 0.0, 0.0], [20.0, 20.0, 60.0]),
        "centre still in front"
    );
    assert!(
        !entered(
            &p,
            [-3.0, 300.0, 0.0],
            [-2.0, 300.0, 0.0],
            [20.0, 20.0, 60.0]
        ),
        "beside the opening"
    );
    assert!(
        entered(
            &p,
            [-3.0, HALF_WIDTH + 10.0, 0.0],
            [-2.0, HALF_WIDTH + 10.0, 0.0],
            [20.0, 20.0, 60.0]
        ),
        "body reaches into it"
    );
    assert!(
        !entered(
            &p,
            [-3.0, 0.0, HALF_HEIGHT + 100.0],
            [-2.0, 0.0, HALF_HEIGHT + 100.0],
            [20.0, 20.0, 60.0]
        ),
        "above it"
    );
    assert!(
        !entered(
            &p,
            [-1200.0, 0.0, 0.0],
            [-1000.0, 0.0, 0.0],
            [20.0, 20.0, 60.0]
        ),
        "far behind the wall"
    );
    assert!(
        !entered(&p, [-0.05, 0.0, 0.0], [-0.1, 0.0, 0.0], [0.1, 0.1, 0.1]),
        "only grazing the plane"
    );
}

#[test]
fn a_fast_mover_that_skips_the_hole_still_enters() {
    let p = wall();
    assert!(entered(&p, [400.0, 0.0, 0.0], [-900.0, 0.0, 0.0], [5.0; 3]));
    assert!(!entered(
        &p,
        [400.0, 200.0, 0.0],
        [-900.0, 200.0, 0.0],
        [5.0; 3]
    ));
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

#[test]
fn roll_recovers_at_up_to_120_degrees_a_second() {
    let (mut roll, mut rate) = (180.0, 0.0);
    for _ in 0..60 {
        (roll, rate) = recover_roll(roll, rate, false, 0.5, 1.0 / 60.0);
    }
    assert!(roll > 0.0 && roll < 180.0, "{roll}");
    assert!(rate <= 120.0 && rate > 0.0, "{rate}");
    for _ in 0..600 {
        (roll, rate) = recover_roll(roll, rate, false, 0.5, 1.0 / 60.0);
    }
    assert_eq!((roll, rate), (0.0, 0.0));
    let (r, _) = recover_roll(-30.0, 120.0, true, 0.5, 0.1);
    assert!(r > -30.0, "negative roll comes back up {r}");
    let (_, ground) = recover_roll(30.0, 200.0, true, 0.5, 0.1);
    assert!(
        ground > 120.0 && ground <= 240.0,
        "on the ground the cap doubles {ground}"
    );
}

#[test]
fn only_movers_in_front_within_the_portal_box_are_touching() {
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
        !touching(&p, [-5.0, 0.0, 0.0], body),
        "centre behind the plane"
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
