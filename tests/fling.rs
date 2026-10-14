// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::Path;

use portal::install::{root, Install};
use portal::pair::Portal;
use portalgun::chamber::{Block, Chamber};
use portalgun::walker::{Input, Walker};

const TICK: f32 = 0.015;

fn chamber() -> Chamber {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    let install = Install::open(&dir).unwrap_or_else(|e| panic!("install at {dir:?}: {e}"));
    Chamber::test_chamber(&install.search).unwrap()
}

fn floor_portal() -> Portal {
    Portal::new(
        [0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        (32.0, 54.0),
    )
}

fn west_wall_high() -> Portal {
    Portal::new(
        [-384.0, 0.0, 250.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    )
}

fn east_wall_high() -> Portal {
    Portal::new(
        [384.0, 100.0, 250.0],
        [-1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    )
}

fn south_wall_high() -> Portal {
    Portal::new(
        [-150.0, -256.0, 250.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    )
}

fn horizontal(v: [f32; 3]) -> f32 {
    v[0].hypot(v[1])
}

/// Steps until the walker crosses, then returns the velocity it came out with.
fn cross(w: &mut Walker, c: &Chamber, pair: [Portal; 2], input: &Input) -> [f32; 3] {
    for tick in 0..400 {
        if w.step(input, TICK, c, Some(pair)).is_some() {
            return w.velocity;
        }
        assert!(
            tick < 399,
            "never crossed, feet {:?} velocity {:?}",
            w.feet,
            w.velocity
        );
    }
    unreachable!()
}

fn drop_into_floor_portal(c: &Chamber) -> (Walker, [f32; 3]) {
    let mut w = Walker::at([0.0, 0.0, 300.0]);
    w.pitch = 89.0;
    let out = cross(
        &mut w,
        c,
        [floor_portal(), west_wall_high()],
        &Input::default(),
    );
    (w, out)
}

fn forward() -> Input {
    Input {
        forward: 1.0,
        ..Input::default()
    }
}

fn back() -> Input {
    Input {
        forward: -1.0,
        ..Input::default()
    }
}

/// The horizontal speed over the next ticks, while the walker is still in the air.
fn air_speeds(
    w: &mut Walker,
    c: &Chamber,
    pair: [Portal; 2],
    input: &Input,
    ticks: usize,
) -> Vec<f32> {
    (0..ticks)
        .map(|_| {
            w.step(input, TICK, c, Some(pair));
            horizontal(w.velocity)
        })
        .collect()
}

#[test]
fn falling_into_a_floor_portal_comes_out_of_a_wall_portal_with_the_fall_speed() {
    let c = chamber();
    let (w, out) = drop_into_floor_portal(&c);
    assert!(out[0] > 500.0, "flung out along the wall normal: {out:?}");
    assert!(out[1].abs() < 1e-3 && out[2].abs() < 60.0, "{out:?}");
    assert!(!w.grounded);
}

#[test]
fn a_floor_to_wall_fling_keeps_its_speed_in_the_air_with_no_input() {
    let c = chamber();
    let (mut w, out) = drop_into_floor_portal(&c);
    let pair = [floor_portal(), west_wall_high()];
    for (tick, speed) in air_speeds(&mut w, &c, pair, &Input::default(), 30)
        .into_iter()
        .enumerate()
    {
        assert!(
            (speed - horizontal(out)).abs() < 1e-2,
            "tick {tick}: {speed} after leaving at {}",
            horizontal(out)
        );
    }
}

#[test]
fn holding_forward_through_a_floor_to_wall_fling_neither_adds_nor_replaces_speed() {
    let c = chamber();
    let (mut w, out) = drop_into_floor_portal(&c);
    let pair = [floor_portal(), west_wall_high()];
    assert!(w.forward()[0] > 0.99, "looking out of the wall portal");
    for (tick, speed) in air_speeds(&mut w, &c, pair, &forward(), 30)
        .into_iter()
        .enumerate()
    {
        assert!(
            (speed - horizontal(out)).abs() < 1e-2,
            "tick {tick}: {speed} after leaving at {}",
            horizontal(out)
        );
    }
}

#[test]
fn pulling_back_against_a_floor_to_wall_fling_does_not_slow_it() {
    let c = chamber();
    let (mut w, out) = drop_into_floor_portal(&c);
    let pair = [floor_portal(), west_wall_high()];
    for (tick, speed) in air_speeds(&mut w, &c, pair, &back(), 30)
        .into_iter()
        .enumerate()
    {
        assert!(
            (speed - horizontal(out)).abs() < 1e-2,
            "tick {tick}: {speed} after leaving at {}",
            horizontal(out)
        );
    }
}

#[test]
fn releasing_forward_after_crossing_does_not_halt_the_fling() {
    let c = chamber();
    let (mut w, out) = drop_into_floor_portal(&c);
    let pair = [floor_portal(), west_wall_high()];
    let mut speeds = air_speeds(&mut w, &c, pair, &forward(), 10);
    speeds.extend(air_speeds(&mut w, &c, pair, &Input::default(), 10));
    assert!(
        speeds.iter().all(|s| (s - horizontal(out)).abs() < 1e-2),
        "{speeds:?} after leaving at {}",
        horizontal(out)
    );
}

#[test]
fn strafing_across_a_fling_adds_at_most_the_air_speed_cap() {
    let c = chamber();
    let (mut w, out) = drop_into_floor_portal(&c);
    let pair = [floor_portal(), west_wall_high()];
    let strafe = Input {
        side: 1.0,
        ..Input::default()
    };
    air_speeds(&mut w, &c, pair, &strafe, 30);
    assert!(!w.grounded, "still flying at {:?}", w.feet);
    assert!((w.velocity[0] - out[0]).abs() < 1e-2, "{:?}", w.velocity);
    assert!(
        (w.velocity[1].abs() - 60.0).abs() < 1e-2,
        "sideways speed settles at the cap: {:?}",
        w.velocity
    );
}

#[test]
fn a_fling_that_lands_slides_to_a_stop_by_friction() {
    let c = chamber();
    let (mut w, _) = drop_into_floor_portal(&c);
    let pair = [floor_portal(), west_wall_high()];
    let mut before = 0.0;
    for _ in 0..200 {
        before = horizontal(w.velocity);
        w.step(&Input::default(), TICK, &c, Some(pair));
        if w.grounded {
            break;
        }
    }
    assert!(w.grounded, "landed at {:?}", w.feet);
    let landed = horizontal(w.velocity);
    assert!(
        landed > before * 0.9,
        "the landing tick keeps most of {before}: {landed}"
    );
    w.step(&Input::default(), TICK, &c, Some(pair));
    let next = horizontal(w.velocity);
    assert!(next > 0.5 * landed && next < landed, "{landed} then {next}");
}

fn moving_east(at: [f32; 3], speed: f32) -> Walker {
    let mut w = Walker::at(at);
    w.velocity = [speed, 0.0, 0.0];
    w
}

#[test]
fn a_wall_to_floor_fling_climbs_on_gravity_alone_whatever_the_input() {
    let c = chamber();
    let pair = [east_wall_high(), floor_portal()];
    for input in [Input::default(), forward(), back()] {
        let mut w = moving_east([300.0, 100.0, 214.0], 600.0);
        let out = cross(&mut w, &c, pair, &input);
        assert!(out[2] > 550.0, "out of the floor going up: {out:?}");
        let still = input.forward == 0.0;
        for tick in 0..20 {
            let v = w.velocity;
            w.step(&input, TICK, &c, Some(pair));
            assert!(
                (w.velocity[2] - (v[2] - 600.0 * TICK)).abs() < 1e-2,
                "tick {tick}: {} after {}",
                w.velocity[2],
                v[2]
            );
            let change = (w.velocity[0] - v[0]).hypot(w.velocity[1] - v[1]);
            let control = if still { 0.0 } else { 15.0 * 150.0 * TICK };
            assert!(
                change <= control + 1e-3,
                "tick {tick}: only air control moves it sideways, {v:?} to {:?}",
                w.velocity
            );
        }
    }
}

#[test]
fn a_wall_to_wall_fling_turns_with_the_portals_and_keeps_its_speed() {
    let c = chamber();
    let pair = [east_wall_high(), south_wall_high()];
    for input in [Input::default(), forward(), back()] {
        let mut w = moving_east([300.0, 100.0, 214.0], 600.0);
        let out = cross(&mut w, &c, pair, &input);
        assert!(
            out[1] > 590.0,
            "out of the south wall heading north: {out:?}"
        );
        let speeds = air_speeds(&mut w, &c, pair, &input, 20);
        assert!(
            speeds.iter().all(|s| (s - horizontal(out)).abs() < 1e-2),
            "{speeds:?} after leaving at {}",
            horizontal(out)
        );
    }
}

#[test]
fn walking_tops_out_at_the_walk_speed() {
    let c = chamber();
    let mut w = Walker::at([-300.0, 0.0, 0.0]);
    w.step(&Input::default(), TICK, &c, None);
    w.step(&forward(), TICK, &c, None);
    assert!((w.velocity[0] - 22.5).abs() < 1e-3, "{:?}", w.velocity);
    w.step(&forward(), TICK, &c, None);
    assert!(
        (w.velocity[0] - 39.0).abs() < 1e-3,
        "less 6 of friction: {:?}",
        w.velocity
    );
    for _ in 0..60 {
        w.step(&forward(), TICK, &c, None);
    }
    assert!((w.velocity[0] - 150.0).abs() < 1e-3, "{:?}", w.velocity);
}

#[test]
fn letting_go_of_forward_on_the_ground_slows_by_friction() {
    let c = chamber();
    let mut w = Walker::at([-300.0, 0.0, 0.0]);
    for _ in 0..60 {
        w.step(&forward(), TICK, &c, None);
    }
    w.step(&Input::default(), TICK, &c, None);
    assert!((w.velocity[0] - 141.0).abs() < 1e-3, "{:?}", w.velocity);
    for _ in 0..40 {
        w.step(&Input::default(), TICK, &c, None);
    }
    assert_eq!(w.velocity, [0.0; 3]);
}

fn hold_jump() -> Input {
    Input {
        jump: true,
        ..Input::default()
    }
}

#[test]
fn a_jump_leaves_at_160_less_a_tick_and_a_half_of_gravity() {
    let c = chamber();
    let mut w = Walker::at([0.0, 0.0, 0.0]);
    w.step(&Input::default(), TICK, &c, None);
    assert!(w.grounded);
    w.step(&hold_jump(), TICK, &c, None);
    assert!(w.jumped && !w.grounded);
    assert!(
        (w.velocity[2] - (160.0 - 900.0 * TICK)).abs() < 1e-3,
        "{:?}",
        w.velocity
    );
}

#[test]
fn holding_jump_does_not_jump_again_on_landing() {
    let c = chamber();
    let mut w = Walker::at([0.0, 0.0, 0.0]);
    w.step(&Input::default(), TICK, &c, None);
    let mut jumps = 0;
    for _ in 0..200 {
        w.step(&hold_jump(), TICK, &c, None);
        jumps += w.jumped as usize;
    }
    assert_eq!(jumps, 1);
    w.step(&Input::default(), TICK, &c, None);
    w.step(&hold_jump(), TICK, &c, None);
    assert!(w.jumped, "jumps again once released");
}

#[test]
fn walking_into_a_wall_portal_pops_out_of_its_floor_partner_at_300() {
    let c = chamber();
    let wall = Portal::new(
        [-384.0, 0.0, 54.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    );
    let pair = [wall, floor_portal()];
    let mut w = Walker::at([-340.0, 0.0, 0.0]);
    w.yaw = 180.0;
    let out = cross(&mut w, &c, pair, &forward());
    assert!((out[2] - 300.0).abs() < 1e-3, "{out:?}");
    w.step(&forward(), TICK, &c, Some(pair));
    assert!(
        (w.velocity[2] - (300.0 - 600.0 * TICK)).abs() < 1e-3,
        "rises out of the floor: {:?}",
        w.velocity
    );
}

#[test]
fn the_ground_is_found_up_to_2_units_below_the_feet() {
    let c = chamber();
    let mut near = Walker::at([0.0, 0.0, 2.06]);
    near.step(&Input::default(), TICK, &c, None);
    assert!(near.grounded && near.feet[2] == 0.03125, "{:?}", near.feet);
    let mut far = Walker::at([0.0, 0.0, 2.1]);
    far.step(&Input::default(), TICK, &c, None);
    assert!(!far.grounded, "{:?}", far.feet);
}

fn with_platform(height: f32) -> Chamber {
    let mut c = chamber();
    let floor = &c.blocks[0];
    let block = Block::cuboid(
        [-200.0, -100.0, 0.0],
        [0.0, 100.0, height],
        &floor.sides[0].material,
        floor.sides[0].surface,
        &floor.surfaceprop,
    );
    c.blocks.push(block);
    c
}

/// Whether a walker leaving the platform at walking speed ever leaves the ground.
fn walks_off(height: f32) -> bool {
    let c = with_platform(height);
    let mut w = Walker::at([-100.0, 0.0, height]);
    w.step(&Input::default(), TICK, &c, None);
    assert!(w.grounded, "starts on the platform: {:?}", w.feet);
    let mut airborne = false;
    for _ in 0..100 {
        w.step(&forward(), TICK, &c, None);
        airborne |= !w.grounded;
    }
    assert!(w.feet[0] > 50.0 && w.feet[2] == 0.03125, "{:?}", w.feet);
    airborne
}

#[test]
fn walking_off_a_drop_of_a_step_keeps_to_the_ground() {
    assert!(!walks_off(18.0 - 0.03125));
    assert!(walks_off(18.0));
    assert!(walks_off(18.5));
}
