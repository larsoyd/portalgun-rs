// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

mod support;

use portal::dmx::Value;
use portal::particles::collection::Vec3;
use portal::particles::forces::{Constraint, Force, Movement, Trace, World};
use portal::particles::random::SimdRandom;
use support::*;

fn force(name: &str, params: Vec<(&str, Value)>) -> Force {
    Force::from_function(&function(name, params)).unwrap()
}

fn constraint(name: &str, params: Vec<(&str, Value)>) -> Constraint {
    Constraint::from_function(&function(name, params)).unwrap()
}

/// a floor at z = 0
struct Floor;

impl World for Floor {
    fn trace(&self, start: Vec3, end: Vec3, _brush_only: bool) -> Trace {
        if start[2] >= 0.0 && end[2] < 0.0 {
            Trace {
                fraction: start[2] / (start[2] - end[2]),
                normal: [0.0, 0.0, 1.0],
                start_solid: false,
            }
        } else {
            Trace {
                fraction: 1.0,
                normal: [0.0; 3],
                start_solid: false,
            }
        }
    }
}

struct Open;

impl World for Open {
    fn trace(&self, _: Vec3, _: Vec3, _: bool) -> Trace {
        Trace {
            fraction: 1.0,
            normal: [0.0; 3],
            start_solid: false,
        }
    }
}

fn close3(a: Vec3, b: Vec3) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < 1e-4)
}

#[test]
fn pull_falls_off_in_whole_quarters() {
    let table = flat(0.5);
    let mut p = particle(0, 0.0, 1.0);
    p.xyz = [4.0, 0.0, 0.0];
    let mut c = collection(&table, SimdRandom::new(), vec![p], 0.0);
    for (falloff, want) in [
        (2.0, -100.0 / 16.0),
        (1.0, -100.0 / 4.0),
        (0.45, -100.0 / 4f32.powf(0.25)),
        (0.0, -100.0),
    ] {
        let mut acc = vec![[0.0; 3]];
        let f = force(
            "Pull towards control point",
            vec![
                ("amount of force", Value::Float(100.0)),
                ("falloff power", Value::Float(falloff)),
            ],
        );
        f.add(&mut c, &mut acc, 1.0);
        assert!(
            (acc[0][0] - want).abs() < 1e-3,
            "falloff {falloff}: {:?}",
            acc[0]
        );
    }
}

#[test]
fn twist_turns_clockwise_seen_from_its_axis() {
    let table = flat(0.5);
    let mut p = particle(0, 0.0, 1.0);
    p.xyz = [5.0, 0.0, 3.0];
    let mut c = collection(&table, SimdRandom::new(), vec![p], 0.0);
    let mut acc = vec![[0.0; 3]];
    force(
        "twist around axis",
        vec![("amount of force", Value::Float(10.0))],
    )
    .add(&mut c, &mut acc, 0.5);
    let s = 5.0 / 34f32.sqrt();
    assert!(close3(acc[0], [0.0, -5.0 * s, 0.0]), "{:?}", acc[0]);
}

#[test]
fn random_force_spans_min_to_max_scaled_by_strength() {
    let table = flat(0.5);
    let ps = (0..5).map(|i| particle(i, 0.0, 1.0)).collect();
    let mut c = collection(&table, SimdRandom::new(), ps, 0.0);
    let mut acc = vec![[0.0; 3]; 5];
    force(
        "random force",
        vec![
            ("min force", Value::Vec3([10.0; 3])),
            ("max force", Value::Vec3([20.0; 3])),
        ],
    )
    .add(&mut c, &mut acc, 0.5);
    let mut check = SimdRandom::new();
    let r = check.draw();
    assert!(
        (acc[0][0] - (5.0 + r[0] * 5.0)).abs() < 1e-4,
        "{:?}",
        acc[0]
    );
    assert!(
        acc.iter().flatten().all(|v| (5.0..10.0).contains(v)),
        "{acc:?}"
    );
}

#[test]
fn movement_is_verlet_with_drag_per_thirtieth() {
    let table = flat(0.5);
    let mut p = particle(0, 0.0, 1.0);
    p.xyz = [10.0, 0.0, 0.0];
    p.prev = [9.0, 0.0, 0.0];
    let mut c = collection(&table, SimdRandom::new(), vec![p], 0.0);
    c.dt = 1.0 / 30.0;
    c.prev_dt = 1.0 / 30.0;
    Movement {
        gravity: [0.0, 0.0, -900.0],
        drag: 0.5,
        passes: 3,
    }
    .integrate(&mut c, &[]);
    let q = c.particles[0];
    assert!(close3(q.xyz, [10.5, 0.0, -1.0]), "{:?}", q.xyz);
    assert_eq!(q.prev, [10.0, 0.0, 0.0]);
    c.prev_dt = 2.0 / 30.0;
    Movement {
        gravity: [0.0; 3],
        drag: -1.0,
        passes: 3,
    }
    .integrate(
        &mut c,
        &[(
            force(
                "Pull towards control point",
                vec![
                    ("falloff power", Value::Float(0.0)),
                    ("amount of force", Value::Float(0.0)),
                ],
            ),
            1.0,
        )],
    );
    assert!(
        close3(c.particles[0].xyz, [10.75, 0.0, -1.5]),
        "half the step ratio carries half the speed: {:?}",
        c.particles[0].xyz
    );
}

#[test]
fn traces_stop_at_the_floor_or_bounce_off_it() {
    let table = flat(0.5);
    let mut p = particle(0, 0.0, 1.0);
    (p.prev, p.xyz, p.radius) = ([0.0, 0.0, 10.0], [0.0, 0.0, -10.0], 0.0);
    let mut c = collection(&table, SimdRandom::new(), vec![p], 0.0);
    assert!(constraint("Collision via traces", vec![]).enforce(&mut c, &Floor));
    assert!(
        close3(c.particles[0].xyz, [0.0, 0.0, 0.0]),
        "{:?}",
        c.particles[0].xyz
    );
    assert_eq!(c.particles[0].prev, [0.0, 0.0, 10.0]);
    c.particles[0].xyz = [0.0, 0.0, -10.0];
    let bounce = constraint(
        "Collision via traces",
        vec![("amount of bounce", Value::Float(1.0))],
    );
    assert!(bounce.is_final());
    bounce.enforce(&mut c, &Floor);
    let q = c.particles[0];
    assert!(
        close3(q.xyz, [0.0, 0.0, 0.5]) && close3(q.prev, [0.0, 0.0, -0.5]),
        "{q:?}"
    );
    (c.particles[0].prev, c.particles[0].xyz) = ([0.0, 0.0, 10.0], [0.0, 0.0, -10.0]);
    constraint(
        "Collision via traces",
        vec![("kill particle on collision", Value::Bool(true))],
    )
    .enforce(&mut c, &Floor);
    assert_eq!(c.particles[0].life, 0.0);
    assert!(!constraint("Collision via traces", vec![]).enforce(&mut c, &Open));
}

#[test]
fn distance_and_plane_constraints_push_particles_back() {
    let table = flat(0.5);
    let mut a = particle(0, 0.0, 1.0);
    a.xyz = [0.0, 200.0, 0.0];
    let mut b = particle(1, 0.0, 1.0);
    b.xyz = [3.0, 0.0, 0.0];
    let mut c = collection(&table, SimdRandom::new(), vec![a, b], 0.0);
    let keep = constraint(
        "Constrain distance to control point",
        vec![("minimum distance", Value::Float(5.0))],
    );
    assert!(keep.enforce(&mut c, &Open));
    assert!(
        close3(c.particles[0].xyz, [0.0, 100.0, 0.0])
            && close3(c.particles[1].xyz, [5.0, 0.0, 0.0])
    );
    c.particles[0].xyz = [0.0, 0.0, -2.0];
    c.particles[0].radius = 1.0;
    let plane = constraint(
        "Prevent passing through a plane",
        vec![("plane point", Value::Vec3([0.0, 0.0, -7.0]))],
    );
    assert!(
        !plane.enforce(&mut c, &Open),
        "above the plane at -7 already"
    );
    c.particles[0].xyz = [0.0, 0.0, -9.0];
    assert!(plane.enforce(&mut c, &Open));
    assert!(
        close3(c.particles[0].xyz, [0.0, 0.0, -6.0]),
        "{:?}",
        c.particles[0].xyz
    );
}

#[test]
fn constraint_passes_rerun_others_and_finals_go_last() {
    let table = flat(0.5);
    let mut p = particle(0, 0.0, 1.0);
    (p.prev, p.xyz, p.radius) = ([0.0, 0.0, 10.0], [0.0, 0.0, -10.0], 0.0);
    let mut c = collection(&table, SimdRandom::new(), vec![p], 0.0);
    let cons = vec![
        (
            constraint(
                "Constrain distance to control point",
                vec![("maximum distance", Value::Float(4.0))],
            ),
            true,
        ),
        (
            constraint(
                "Collision via traces",
                vec![("amount of bounce", Value::Float(1.0))],
            ),
            true,
        ),
    ];
    Movement {
        gravity: [0.0; 3],
        drag: 0.0,
        passes: 3,
    }
    .constrain(&mut c, &cons, &Floor);
    let q = c.particles[0];
    // the cap pulls the move back to z = -4, then the final bounce reflects the clamped move
    assert!(
        close3(q.xyz, [0.0, 0.0, 2.0 / 7.0]) && close3(q.prev, [0.0, 0.0, -5.0 / 7.0]),
        "{q:?}"
    );
}

#[test]
fn traces_reach_past_the_move_by_the_particle_radius() {
    let table = flat(0.5);
    let mut p = particle(0, 0.0, 1.0);
    (p.prev, p.xyz, p.radius) = ([0.0, 0.0, 10.0], [0.0, 0.0, 1.0], 2.0);
    let mut c = collection(&table, SimdRandom::new(), vec![p], 0.0);
    assert!(constraint("Collision via traces", vec![]).enforce(&mut c, &Floor));
    // the fraction along the move extended by the radius, applied to the move itself
    assert!(
        close3(c.particles[0].xyz, [0.0, 0.0, 10.0 - 9.0 * 10.0 / 11.0]),
        "{:?}",
        c.particles[0].xyz
    );
}

#[test]
fn a_constraint_that_moves_particles_sends_the_others_round_again() {
    let table = flat(0.5);
    let mut p = particle(0, 0.0, 1.0);
    (p.xyz, p.radius) = ([10.0, 0.0, -5.0], 0.0);
    let mut c = collection(&table, SimdRandom::new(), vec![p], 0.0);
    let cons = vec![
        (
            constraint(
                "Prevent passing through a plane",
                vec![("plane point", Value::Vec3([0.0, 0.0, 3.0]))],
            ),
            true,
        ),
        (
            constraint(
                "Constrain distance to control point",
                vec![("maximum distance", Value::Float(4.0))],
            ),
            true,
        ),
    ];
    Movement {
        gravity: [0.0; 3],
        drag: 0.0,
        passes: 3,
    }
    .constrain(&mut c, &cons, &Open);
    assert!(
        close3(c.particles[0].xyz, [2.896_283_4, 0.0, 2.758_902_3]),
        "{:?}",
        c.particles[0].xyz
    );
}

#[test]
fn force_and_constraint_names_match_without_case() {
    let f = function("RANDOM FORCE", vec![]);
    assert!(Force::from_function(&f).is_some());
    let c = function("collision VIA traces", vec![]);
    assert!(Constraint::from_function(&c).is_some());
}
