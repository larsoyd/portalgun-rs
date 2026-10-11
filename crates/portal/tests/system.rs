// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

mod support;

use portal::dmx::Value::{self, Float, Int, Vec3 as V3};
use portal::particles::collection::Vec3;
use portal::particles::forces::{Trace, World};
use portal::particles::initializers::Query;
use portal::particles::random::SimdRandom;
use portal::particles::system::{Definition, System};
use portal::pcf::{Child, Function, SystemDef};
use support::*;

struct Empty;

impl World for Empty {
    fn trace(&self, _: Vec3, _: Vec3, _: bool) -> Trace {
        Trace {
            fraction: 1.0,
            normal: [0.0; 3],
            start_solid: false,
        }
    }
}

impl Query for Empty {
    fn trace(&self, _: Vec3, _: Vec3, _: i32) -> f32 {
        1.0
    }
}

fn f(name: &str, params: Vec<(&str, Value)>) -> Function {
    function(name, params)
}

fn def(name: &str) -> SystemDef {
    SystemDef {
        name: name.into(),
        max_particles: 1000,
        radius: 5.0,
        color: [255; 4],
        max_time_step: 0.1,
        ..SystemDef::default()
    }
}

fn life(v: f32) -> Function {
    f(
        "Lifetime Random",
        vec![("lifetime_min", Float(v)), ("lifetime_max", Float(v))],
    )
}

fn run(s: &mut System, simd: &mut SimdRandom, steps: &[f32]) {
    for dt in steps {
        s.simulate(*dt, simd, &Empty, &Empty);
    }
}

#[test]
fn a_system_with_an_unported_function_is_refused() {
    let mut d = def("noisy");
    d.operators.push(f("Noise Scalar", vec![]));
    let err = Definition::new(&d).unwrap_err();
    assert_eq!(
        err,
        "system=noisy kind=operator function=Noise Scalar not ported"
    );
}

#[test]
fn the_template_holds_the_definitions_constants() {
    let mut d = def("orange");
    d.color = [255, 198, 0, 51];
    d.radius = 7.0;
    d.max_particles = 9000;
    let t = Definition::new(&d).unwrap();
    assert_eq!(
        t.template.tint,
        [1.0, 198.0 * (1.0 / 255.0), 198.0 * (1.0 / 255.0)],
        "blue from green"
    );
    assert!(close(t.template.alpha, 0.2));
    assert_eq!(
        (t.template.radius, t.template.life, t.template.trail),
        (7.0, 1.0, 0.1)
    );
    assert_eq!(t.max_particles, 5000);
    d.max_particles = 0;
    assert_eq!(Definition::new(&d).unwrap().max_particles, 1);
}

#[test]
fn particles_are_born_named_and_age_out() {
    let mut d = def("glow");
    d.emitters.push(f("emit_continuously", vec![]));
    d.initializers.push(life(0.155));
    d.operators.push(f("Lifespan Decay", vec![]));
    let defs = [Definition::new(&d).unwrap()];
    let table = flat(0.5);
    let mut simd = SimdRandom::new();
    let mut s = System::new(&defs, 0, 0.0, 7, &table);
    run(&mut s, &mut simd, &[0.1]);
    assert_eq!(s.c.particles.len(), 10);
    let ids: Vec<u32> = s.c.particles.iter().map(|p| p.id).collect();
    assert_eq!(ids, (7..17).collect::<Vec<u32>>(), "unique id plus seed");
    assert!(s.c.particles.iter().all(|p| p.life == 0.155));
    run(&mut s, &mut simd, &[0.1, 0.1]);
    assert_eq!(
        s.c.particles.len(),
        16,
        "the first ten and four of the next gone"
    );
}

#[test]
fn long_frames_are_cut_into_steps_and_capped() {
    let mut d = def("steps");
    d.max_time_step = 0.05;
    d.emitters.push(f("emit_continuously", vec![]));
    let defs = [Definition::new(&d).unwrap()];
    let table = flat(0.5);
    let mut simd = SimdRandom::new();
    let mut s = System::new(&defs, 0, 0.0, 1, &table);
    run(&mut s, &mut simd, &[0.1]);
    assert_eq!(s.c.particles.len(), 10);
    assert!(close(s.c.cur_time, 0.1));
    assert!(close(s.c.dt, 0.05), "the last sub-step");
    assert!(close(s.c.prev_dt, 0.1), "the whole frame");
    run(&mut s, &mut simd, &[5.0]);
    assert!(close(s.c.cur_time, 0.6), "at most ten steps");
}

#[test]
fn undrawn_systems_stop_at_their_sim_limit() {
    let mut d = def("limited");
    d.max_sim = 0.025;
    let defs = [Definition::new(&d).unwrap()];
    let table = flat(0.5);
    let mut simd = SimdRandom::new();
    let mut s = System::new(&defs, 0, 0.0, 1, &table);
    run(&mut s, &mut simd, &[0.1]);
    assert!(close(s.c.cur_time, 0.025));
    run(&mut s, &mut simd, &[0.1]);
    assert!(close(s.c.cur_time, 0.125), "only for the first frames");
}

#[test]
fn children_wait_out_their_delay() {
    let mut parent = def("parent");
    parent.children.push(Child {
        system: 1,
        delay: 0.25,
    });
    let mut child = def("child");
    child.initial_particles = 3;
    let defs = [
        Definition::new(&parent).unwrap(),
        Definition::new(&child).unwrap(),
    ];
    let table = flat(0.5);
    let mut simd = SimdRandom::new();
    let mut s = System::new(&defs, 0, 0.0, 1, &table);
    s.set_control_point(2, [1.0, 2.0, 3.0]);
    assert_eq!(s.children[0].c.control_points[2].pos, [1.0, 2.0, 3.0]);
    assert_eq!(s.children[0].c.highest_cp, 2);
    run(&mut s, &mut simd, &[0.1, 0.1, 0.1]);
    let kid = &s.children[0];
    assert!(close(kid.c.cur_time, 0.05), "past zero and not clamped");
    assert!(close(kid.c.prev_dt, 0.1), "kept up while waiting");
    assert!(kid.c.particles.is_empty(), "started on the next frame");
    run(&mut s, &mut simd, &[0.1]);
    let kid = &s.children[0];
    assert!(close(kid.c.cur_time, 0.15));
    assert_eq!(kid.c.particles.len(), 3);
    assert!(kid.c.particles.iter().all(|p| close(p.creation, 0.05)));

    let late = System::new(&defs, 0, 0.1, 1, &table);
    assert!(close(late.children[0].c.cur_time, -0.35), "delays add up");
}

#[test]
fn movement_applies_gravity_and_only_running_forces() {
    let mut d = def("falling");
    d.emitters
        .push(f("emit_instantaneously", vec![("num_to_emit", Int(1))]));
    d.initializers
        .push(f("Position Within Sphere Random", vec![]));
    d.operators.push(f(
        "Movement Basic",
        vec![("gravity", V3([0.0, 0.0, -100.0]))],
    ));
    d.forces.push(f(
        "random force",
        vec![
            ("min force", V3([0.0, 500.0, 0.0])),
            ("max force", V3([0.0, 500.0, 0.0])),
            ("operator start fadein", Float(5.0)),
        ],
    ));
    d.forces.push(f(
        "random force",
        vec![
            ("min force", V3([500.0, 0.0, 0.0])),
            ("max force", V3([500.0, 0.0, 0.0])),
        ],
    ));
    let defs = [Definition::new(&d).unwrap()];
    let table = flat(0.5);
    let mut simd = SimdRandom::new();
    let mut s = System::new(&defs, 0, 0.0, 1, &table);
    s.set_control_point(0, [4.0, 0.0, 10.0]);
    run(&mut s, &mut simd, &[0.1]);
    let p = s.c.particles[0];
    assert!(close(p.xyz[0], 9.0), "the running force {:?}", p.xyz);
    assert!(close(p.xyz[1], 0.0), "the other has not faded in");
    assert!(
        close(p.xyz[2], 9.0),
        "a hundred down for a tenth squared {:?}",
        p.xyz
    );
}

#[test]
fn a_burst_is_finished_once_its_particles_die() {
    let mut d = def("burst");
    d.emitters
        .push(f("emit_instantaneously", vec![("num_to_emit", Int(2))]));
    d.initializers.push(life(0.15));
    d.operators.push(f("Lifespan Decay", vec![]));
    let defs = [Definition::new(&d).unwrap()];
    let table = flat(0.5);
    let mut simd = SimdRandom::new();
    let mut s = System::new(&defs, 0, 0.0, 1, &table);
    assert!(!s.is_finished(), "not yet started");
    run(&mut s, &mut simd, &[0.1]);
    assert_eq!(s.c.particles.len(), 2);
    assert!(!s.is_finished());
    run(&mut s, &mut simd, &[0.1]);
    assert!(s.is_finished());

    let mut s = System::new(&defs, 0, 0.0, 1, &table);
    run(&mut s, &mut simd, &[0.1]);
    s.stop_emission(true);
    assert!(s.is_finished(), "stopped and cleared");

    let mut d = def("late");
    d.emitters.push(f(
        "emit_continuously",
        vec![("emission_start_time", Float(1.0))],
    ));
    let defs = [Definition::new(&d).unwrap()];
    let mut s = System::new(&defs, 0, 0.0, 1, &table);
    run(&mut s, &mut simd, &[0.1]);
    assert!(!s.is_finished(), "its emitter has not started");

    let defs = [Definition::new(&def("empty")).unwrap()];
    let mut s = System::new(&defs, 0, 0.0, 1, &table);
    assert!(!s.is_finished(), "the first frame is still to come");
    run(&mut s, &mut simd, &[0.1]);
    assert!(s.is_finished());
}

#[test]
fn operators_after_a_kill_still_run_on_the_survivors() {
    let mut d = def("thinned");
    d.emitters
        .push(f("emit_instantaneously", vec![("num_to_emit", Int(2))]));
    d.initializers.push(f(
        "Lifetime Random",
        vec![("lifetime_min", Float(0.0)), ("lifetime_max", Float(0.8))],
    ));
    d.operators.push(f("Lifespan Decay", vec![]));
    d.operators.push(f(
        "Radius Scale",
        vec![
            ("radius_start_scale", Float(2.0)),
            ("radius_end_scale", Float(2.0)),
        ],
    ));
    let defs = [Definition::new(&d).unwrap()];
    let table = ramp();
    let mut simd = SimdRandom::new();
    let mut s = System::new(&defs, 0, 0.0, 1, &table);
    run(&mut s, &mut simd, &[0.1]);
    assert_eq!(s.c.particles.len(), 1, "lives of 0.1 and 0.2");
    assert_eq!(s.c.particles[0].radius, 10.0);
}

#[test]
fn each_operator_that_runs_moves_the_random_offset_on_by_17() {
    let mut d = def("offset");
    d.emitters
        .push(f("emit_instantaneously", vec![("num_to_emit", Int(1))]));
    d.operators.push(f("Radius Scale", vec![]));
    d.operators.push(f(
        "Alpha Fade In Random",
        vec![
            ("fade in time min", Float(0.0)),
            ("fade in time max", Float(8.0)),
            ("proportional 0/1", Value::Bool(false)),
        ],
    ));
    let defs = [Definition::new(&d).unwrap()];
    let table = ramp();
    let mut simd = SimdRandom::new();
    let mut s = System::new(&defs, 0, 0.0, 0, &table);
    run(&mut s, &mut simd, &[0.1]);
    let alpha = s.c.particles[0].alpha;
    assert!(
        close(alpha, 0.028),
        "a one second fade from entry 17 {alpha}"
    );
}
