use portal::movement::{
    accelerate, air_accelerate, check_velocity, command, finish_gravity, fling_guard, flinging,
    friction, funnel, jump, leaves_ground, start_gravity, wish, Funnel, ACCELERATE, AIR_ACCELERATE,
    AIR_SPEED_CAP, GRAVITY, JUMP_SPEED, MAX_SPEED, MAX_VELOCITY, WALK_SPEED,
};
use portal::pair::Portal;

const DT: f32 = 0.015;

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn the_servers_movement_settings_are_portals_defaults() {
    assert_eq!(GRAVITY, 600.0);
    assert_eq!(ACCELERATE, 10.0);
    assert_eq!(AIR_ACCELERATE, 15.0);
    assert_eq!(AIR_SPEED_CAP, 60.0);
    assert_eq!(MAX_SPEED, 320.0);
    assert_eq!(WALK_SPEED, 150.0);
    assert_eq!(JUMP_SPEED, 160.0);
    assert_eq!(MAX_VELOCITY, 3500.0);
}

#[test]
fn keys_ask_for_450_and_are_cut_to_the_walk_speed() {
    let (f, s) = command(1.0, 0.0, WALK_SPEED);
    assert!(close(f, 150.0) && s == 0.0, "{f} {s}");
    let (f, s) = command(1.0, 1.0, WALK_SPEED);
    assert!(close(f.hypot(s), 150.0) && close(f, s), "{f} {s}");
    let (f, s) = command(0.0, 0.0, WALK_SPEED);
    assert_eq!((f, s), (0.0, 0.0));
    let (f, _) = command(0.2, 0.0, 1000.0);
    assert!(close(f, 90.0), "under the cap the 450 is kept: {f}");
}

#[test]
fn the_wish_turns_with_yaw_alone() {
    let w = wish(90.0, 150.0, 0.0);
    assert!(close(w[0], 0.0) && close(w[1], 150.0), "{w:?}");
    let w = wish(0.0, 0.0, 150.0);
    assert!(
        close(w[0], 0.0) && close(w[1], -150.0),
        "right of east is south: {w:?}"
    );
}

#[test]
fn friction_takes_four_times_the_speed_a_second() {
    let v = friction([300.0, 0.0, 0.0], DT);
    assert!(close(v[0], 300.0 * (1.0 - 4.0 * DT)), "{v:?}");
}

#[test]
fn friction_below_the_stop_speed_drops_by_the_stop_speed() {
    let v = friction([100.0, 0.0, 0.0], DT);
    assert!(close(v[0], 100.0 - 400.0 * DT), "at 100: {v:?}");
    let v = friction([50.0, 0.0, 0.0], DT);
    assert!(close(v[0], 50.0 - 400.0 * DT), "{v:?}");
    let v = friction([101.0, 0.0, 0.0], DT);
    assert!(close(v[0], 101.0 * (1.0 - 4.0 * DT)), "{v:?}");
}

#[test]
fn friction_stops_dead_rather_than_reversing() {
    assert_eq!(friction([5.0, 0.0, 0.0], DT), [0.0, 0.0, 0.0]);
}

#[test]
fn friction_leaves_a_crawl_under_a_tenth_alone() {
    assert_eq!(friction([0.09, 0.0, 0.0], DT), [0.09, 0.0, 0.0]);
    assert_eq!(friction([0.1, 0.0, 0.0], DT), [0.0, 0.0, 0.0]);
}

#[test]
fn friction_counts_the_vertical_speed() {
    let v = friction([80.0, 0.0, 60.0], DT);
    assert!(
        close(v[0], 80.0 * (1.0 - 4.0 * DT)),
        "speed 100 in 3d: {v:?}"
    );
}

#[test]
fn ground_acceleration_adds_ten_times_the_wish_a_second_up_to_it() {
    let v = accelerate([0.0; 3], [1.0, 0.0, 0.0], 150.0, ACCELERATE, DT);
    assert!(close(v[0], 22.5), "{v:?}");
    let v = accelerate([140.0, 0.0, 0.0], [1.0, 0.0, 0.0], 150.0, ACCELERATE, DT);
    assert!(close(v[0], 150.0), "{v:?}");
    let v = accelerate([200.0, 0.0, 0.0], [1.0, 0.0, 0.0], 150.0, ACCELERATE, DT);
    assert_eq!(v, [200.0, 0.0, 0.0], "never slows");
}

#[test]
fn air_acceleration_caps_the_wish_at_60_but_scales_by_the_full_wish() {
    let v = air_accelerate([0.0; 3], [1.0, 0.0, 0.0], 150.0, AIR_ACCELERATE, DT);
    assert!(close(v[0], 15.0 * 150.0 * DT), "{v:?}");
    let v = air_accelerate([50.0, 0.0, 0.0], [1.0, 0.0, 0.0], 150.0, AIR_ACCELERATE, DT);
    assert!(close(v[0], 60.0), "{v:?}");
    let v = air_accelerate([60.0, 0.0, 0.0], [1.0, 0.0, 0.0], 150.0, AIR_ACCELERATE, DT);
    assert_eq!(v[0], 60.0);
    let v = air_accelerate([0.0; 3], [1.0, 0.0, 0.0], 40.0, AIR_ACCELERATE, DT);
    assert!(close(v[0], 15.0 * 40.0 * DT), "{v:?}");
    let v = air_accelerate([35.0, 0.0, 0.0], [1.0, 0.0, 0.0], 40.0, AIR_ACCELERATE, DT);
    assert!(close(v[0], 40.0), "a small wish caps itself: {v:?}");
}

#[test]
fn a_fling_ignores_steering_against_it_on_each_axis() {
    assert_eq!(fling_guard([400.0, 0.0], [-150.0, 0.0]), [0.0, 0.0]);
    assert_eq!(fling_guard([-400.0, 0.0], [150.0, 0.0]), [0.0, 0.0]);
    assert_eq!(fling_guard([0.0, 400.0], [10.0, -150.0]), [10.0, 0.0]);
    assert_eq!(fling_guard([0.0, -400.0], [10.0, 150.0]), [10.0, 0.0]);
    assert_eq!(fling_guard([400.0, 0.0], [150.0, 20.0]), [150.0, 20.0]);
}

#[test]
fn the_fling_guard_starts_above_300() {
    assert_eq!(fling_guard([300.0, 0.0], [-150.0, 0.0]), [-150.0, 0.0]);
    assert_eq!(fling_guard([300.01, 0.0], [-150.0, 0.0]), [0.0, 0.0]);
}

#[test]
fn a_fling_is_over_300_across_the_ground() {
    assert!(!flinging([300.0, 0.0]));
    assert!(flinging([300.01, 0.0]));
    assert!(flinging([0.0, -300.01]));
    assert!(!flinging([212.0, 212.0]));
    assert!(flinging([212.2, 212.2]));
}

#[test]
fn the_fling_guard_needs_150_along_the_axis() {
    assert_eq!(fling_guard([150.0, 300.0], [-1.0, 0.0]), [-1.0, 0.0]);
    assert_eq!(fling_guard([150.01, 300.0], [-1.0, 0.0]), [0.0, 0.0]);
    assert_eq!(fling_guard([-150.0, 300.0], [1.0, 0.0]), [1.0, 0.0]);
    assert_eq!(fling_guard([-150.01, 300.0], [1.0, 0.0]), [0.0, 0.0]);
    assert_eq!(fling_guard([300.0, 150.0], [0.0, -1.0]), [0.0, -1.0]);
    assert_eq!(fling_guard([300.0, -150.01], [0.0, 1.0]), [0.0, 0.0]);
}

fn floor_portal() -> Portal {
    Portal::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        (32.0, 54.0),
    )
}

const DOWN: [f32; 3] = [0.0, 0.0, -1.0];

#[test]
fn falling_onto_a_floor_portal_steers_toward_its_centre() {
    let got = funnel(
        [10.0, 0.0, -300.0],
        [0.0, 0.0],
        [20.0, -5.0, 100.0],
        DOWN,
        &floor_portal(),
    );
    assert_eq!(got, Funnel::Wish([-130.0, 30.0]));
}

#[test]
fn the_funnel_holds_still_right_over_the_portal() {
    let got = funnel(
        [10.0, 5.0, -300.0],
        [0.0, 0.0],
        [0.0, 0.0, 7.0],
        DOWN,
        &floor_portal(),
    );
    assert_eq!(got, Funnel::Stop);
    let got = funnel(
        [10.0, 5.0, -300.0],
        [0.0, 0.0],
        [0.0, 0.0, 8.0],
        DOWN,
        &floor_portal(),
    );
    assert_ne!(got, Funnel::Stop, "8 above is not close enough");
}

#[test]
fn the_funnel_needs_a_floor_portal_a_downward_look_and_a_fall() {
    let feet = [10.0, 0.0, 100.0];
    let v = [0.0, 0.0, -300.0];
    let tilted = Portal::new([0.0; 3], [0.0, 0.6, 0.8], [1.0, 0.0, 0.0], (32.0, 54.0));
    assert_ne!(
        funnel(v, [0.0; 2], feet, DOWN, &tilted),
        Funnel::Off,
        "0.8 counts"
    );
    let steep = Portal::new([0.0; 3], [0.0, 0.61, 0.79], [1.0, 0.0, 0.0], (32.0, 54.0));
    assert_eq!(funnel(v, [0.0; 2], feet, DOWN, &steep), Funnel::Off);
    let level = [0.995, 0.0, -0.0999];
    assert_eq!(
        funnel(v, [0.0; 2], feet, level, &floor_portal()),
        Funnel::Off
    );
    let exact = [0.994, 0.0, -0.1];
    assert_ne!(
        funnel(v, [0.0; 2], feet, exact, &floor_portal()),
        Funnel::Off
    );
    let barely = [0.994, 0.0, -0.1001];
    assert_ne!(
        funnel(v, [0.0; 2], feet, barely, &floor_portal()),
        Funnel::Off
    );
    assert_eq!(
        funnel([0.0, 0.0, -164.9], [0.0; 2], feet, DOWN, &floor_portal()),
        Funnel::Off
    );
    assert_ne!(
        funnel([0.0, 0.0, -165.0], [0.0; 2], feet, DOWN, &floor_portal()),
        Funnel::Off
    );
}

#[test]
fn the_funnel_lets_go_of_a_player_steering_hard() {
    let feet = [10.0, 0.0, 100.0];
    let v = [0.0, 0.0, -300.0];
    assert_ne!(
        funnel(v, [64.0, -64.0], feet, DOWN, &floor_portal()),
        Funnel::Off
    );
    assert_eq!(
        funnel(v, [64.1, 0.0], feet, DOWN, &floor_portal()),
        Funnel::Off
    );
    assert_eq!(
        funnel(v, [0.0, -64.1], feet, DOWN, &floor_portal()),
        Funnel::Off
    );
}

#[test]
fn the_funnel_reaches_512_down_48_across_and_81_along() {
    let v = [0.0, 0.0, -300.0];
    let p = floor_portal();
    assert_ne!(
        funnel(v, [0.0; 2], [0.0, 0.0, 512.0], DOWN, &p),
        Funnel::Off
    );
    assert_eq!(
        funnel(v, [0.0; 2], [0.0, 0.0, 512.1], DOWN, &p),
        Funnel::Off
    );
    assert_ne!(
        funnel(v, [0.0; 2], [0.0, 48.0, 100.0], DOWN, &p),
        Funnel::Off
    );
    assert_eq!(
        funnel(v, [0.0; 2], [0.0, 48.1, 100.0], DOWN, &p),
        Funnel::Off
    );
    assert_ne!(
        funnel(v, [0.0; 2], [81.0, 0.0, 100.0], DOWN, &p),
        Funnel::Off
    );
    assert_eq!(
        funnel(v, [0.0; 2], [81.1, 0.0, 100.0], DOWN, &p),
        Funnel::Off
    );
}

#[test]
fn gravity_comes_in_two_halves_around_the_move() {
    assert!(close(start_gravity(0.0, DT), -4.5));
    assert!(close(finish_gravity(-4.5, DT), -9.0));
}

#[test]
fn a_jump_adds_160_and_a_forward_boost_up_to_one_and_a_half_times_the_walk_speed() {
    let v = jump([0.0, 0.0, -4.5], 0.0, 150.0, WALK_SPEED);
    assert!(close(v[2], 155.5), "{v:?}");
    assert!(close(v[0], 75.0), "half the forward move: {v:?}");
    let v = jump([200.0, 0.0, 0.0], 0.0, 150.0, WALK_SPEED);
    assert!(close(v[0], 225.0), "boost stops at 225: {v:?}");
    let v = jump([300.0, 0.0, 0.0], 0.0, 150.0, WALK_SPEED);
    assert!(close(v[0], 225.0), "faster than that it brakes: {v:?}");
    let v = jump([300.0, 0.0, 0.0], 0.0, -150.0, WALK_SPEED);
    assert!(close(v[0], 375.0), "backwards it speeds up: {v:?}");
    let v = jump([0.0; 3], 0.0, -150.0, WALK_SPEED);
    assert!(
        close(v[0], -75.0),
        "a standing jump backwards goes back: {v:?}"
    );
    let v = jump([0.0; 3], 90.0, 0.0, WALK_SPEED);
    assert!(close(v[0], 0.0) && close(v[1], 0.0), "{v:?}");
}

#[test]
fn the_ground_lets_go_above_140_up() {
    assert!(!leaves_ground(140.0));
    assert!(leaves_ground(140.01));
}

#[test]
fn velocity_is_clamped_to_3500_on_each_axis() {
    assert_eq!(
        check_velocity([4000.0, -3600.0, 3500.0]),
        [3500.0, -3500.0, 3500.0]
    );
    assert_eq!(check_velocity([f32::NAN, 1.0, 2.0]), [0.0, 1.0, 2.0]);
    assert_eq!(check_velocity([1.0, f32::INFINITY, 2.0]), [1.0, 0.0, 2.0]);
}
