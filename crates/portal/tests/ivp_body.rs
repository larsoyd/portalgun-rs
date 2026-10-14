// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::ivp::body::{step, Body, Solid};
use portal::ivp::hull::Convex;
use portal::ivp::TOLERANCE;

const M: f64 = 0.0254;
const G: [f64; 3] = [0.0, 0.0, -600.0 * M];
const DT: f64 = 0.015;
const T: f64 = TOLERANCE as f64;

fn cube() -> Body {
    let mut b = Body::new(
        Convex::cube(20.0 * M),
        40.000004,
        [0.12165254, 0.12122901, 0.12122901],
    );
    b.damp = 0.1;
    b.radius = 0.87988;
    b.friction = 0.8;
    b.elasticity = 0.2;
    b
}

fn slab(id: usize, lo: [f64; 3], hi: [f64; 3]) -> Solid {
    let planes = (0..3)
        .flat_map(|a| {
            let mut n = [0.0; 3];
            n[a] = 1.0;
            [(n, hi[a] * M), (n.map(|v| -v), -lo[a] * M)]
        })
        .collect::<Vec<_>>();
    Solid {
        id,
        hull: Convex::from_planes(&planes),
        friction: 0.8,
        elasticity: 0.2,
    }
}

fn floor() -> Vec<Solid> {
    vec![slab(0, [-512.0, -512.0, -16.0], [512.0, 512.0, 0.0])]
}

fn lowest(b: &Body) -> f64 {
    b.world_hull()
        .points
        .iter()
        .map(|p| p[2])
        .fold(f64::MAX, f64::min)
}

fn run(b: &mut Body, world: &[Solid], seconds: f64) -> (f64, Option<f64>) {
    let mut deepest = f64::MAX;
    let mut slept = None;
    for i in 0..(seconds / DT) as usize {
        step(b, world, G, DT, i as f64 * DT);
        deepest = deepest.min(lowest(b));
        if b.asleep && slept.is_none() {
            slept = Some(i as f64 * DT);
        }
    }
    (deepest, slept)
}

#[test]
fn a_flat_drop_rests_between_one_and_two_tolerances_and_sleeps() {
    let mut b = cube();
    b.position = [0.0, 0.0, 60.0 * M];
    let (deepest, slept) = run(&mut b, &floor(), 4.0);
    let rest = lowest(&b);
    assert!(deepest > -0.5 * M, "sank to {deepest}");
    assert!(
        (T..=2.5 * T).contains(&rest),
        "rests at {rest} outside {T}..{}",
        2.5 * T
    );
    assert!(slept.is_some_and(|t| t < 3.0), "slept at {slept:?}");
}

fn rng(s: &mut u64) -> f64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    (*s >> 11) as f64 / (1u64 << 53) as f64
}

#[test]
fn tumbling_drops_never_sink_and_come_to_sleep() {
    let mut seed = 0x9e3779b97f4a7c15u64;
    for run_index in 0..24 {
        let mut b = cube();
        b.position = [0.0, 0.0, (40.0 + rng(&mut seed) * 150.0) * M];
        b.speed = [
            (rng(&mut seed) - 0.5) * 4.0,
            (rng(&mut seed) - 0.5) * 4.0,
            0.0,
        ];
        b.spin = [0, 1, 2].map(|_| (rng(&mut seed) - 0.5) * 12.0);
        let (deepest, slept) = run(&mut b, &floor(), 12.0);
        assert!(deepest > -M, "run {run_index} sank to {deepest}");
        assert!(
            slept.is_some(),
            "run {run_index} still moving speed={:?} spin={:?}",
            b.speed,
            b.spin
        );
        let rest = lowest(&b);
        assert!(
            rest > 0.0 && rest < 4.5 * T,
            "run {run_index} rests at {rest}"
        );
    }
}

#[test]
fn a_flat_slide_stops_at_the_coulomb_distance() {
    let mut b = cube();
    b.position = [0.0, 0.0, 20.0 * M + 2.0 * T];
    let world = floor();
    run(&mut b, &world, 0.5);
    let start = b.position[0];
    b.wake();
    b.speed = [2.0, 0.0, 0.0];
    run(&mut b, &world, 2.0);
    let slid = b.position[0] - start;
    let want = 4.0 / (2.0 * 0.64 * 600.0 * M);
    assert!((slid - want).abs() < 0.25 * want, "slid {slid} want {want}");
}

#[test]
fn a_flat_landing_barely_bounces() {
    let mut b = cube();
    b.position = [0.0, 0.0, (20.0 + 40.0) * M];
    let mut peak_after = f64::MIN;
    let mut landed = false;
    for i in 0..200 {
        step(&mut b, &floor(), G, DT, i as f64 * DT);
        if b.speed[2] > 0.0 {
            landed = true;
        }
        if landed {
            peak_after = peak_after.max(lowest(&b));
        }
    }
    assert!(landed);
    assert!(peak_after < 2.0 * M, "bounced to {peak_after}");
}

#[test]
fn a_cube_started_inside_a_wall_is_pushed_out_instead_of_freezing() {
    let mut b = cube();
    let world = vec![
        slab(0, [-512.0, -512.0, -16.0], [512.0, 512.0, 0.0]),
        slab(1, [-16.0, -512.0, 0.0], [0.0, 512.0, 512.0]),
    ];
    b.position = [10.0 * M, 0.0, 20.0 * M + 2.0 * T];
    let (_, _) = run(&mut b, &world, 3.0);
    let left = b
        .world_hull()
        .points
        .iter()
        .map(|p| p[0])
        .fold(f64::MAX, f64::min);
    assert!(left > -0.5 * M, "still {left} inside the wall");
}
