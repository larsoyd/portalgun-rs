// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

mod support;

use portal::dmx::Value::{self, Float, Int};
use portal::particles::collection::Collection;
use portal::particles::emitters::Emitter;
use portal::particles::random::SimdRandom;
use support::*;

fn emitter(name: &str, params: Vec<(&str, Value)>, c: &mut Collection) -> Emitter {
    let mut e = Emitter::from_function(&function(name, params)).unwrap();
    e.init_context(c);
    e
}

fn step(c: &mut Collection, e: &mut Emitter, now: f32, strength: f32) -> Vec<f32> {
    let before = c.particles.len();
    c.cur_time = now;
    e.emit(c, strength, None);
    c.particles[before..].iter().map(|p| p.creation).collect()
}

#[test]
fn continuous_emission_spreads_the_due_count_over_the_step() {
    let table = flat(0.5);
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    c.template.radius = 3.0;
    let mut e = emitter("emit_continuously", vec![], &mut c);
    let made = step(&mut c, &mut e, 0.1, 1.0);
    assert_eq!(made.len(), 10, "100 a second for 0.1 s");
    assert!(close(made[0], 0.01), "{made:?}");
    assert!(close(made[9], 0.1), "ends with the step");
    assert!(
        c.particles.iter().all(|p| p.radius == 3.0),
        "from the template"
    );
    assert_eq!(step(&mut c, &mut e, 0.2, 0.5).len(), 5, "faded to half");
    e.stop();
    assert!(step(&mut c, &mut e, 0.3, 1.0).is_empty());
    assert!(!e.may_emit(&c));

    let mut e = emitter(
        "emit_continuously",
        vec![("emission_rate", Float(30.0))],
        &mut c,
    );
    c.particles.clear();
    assert_eq!(
        step(&mut c, &mut e, 0.1, 1.0)[2],
        0.1,
        "thirds would pass 0.1"
    );
}

#[test]
fn continuous_emission_counts_what_did_not_fit() {
    let table = flat(0.5);
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    c.max_allowed = 8;
    let mut e = emitter("emit_continuously", vec![], &mut c);
    assert_eq!(step(&mut c, &mut e, 0.1, 1.0).len(), 8);
    c.particles.clear();
    c.max_allowed = 100;
    assert_eq!(
        step(&mut c, &mut e, 0.2, 1.0).len(),
        10,
        "the two dropped are gone"
    );
}

#[test]
fn continuous_emission_keeps_to_its_window() {
    let table = flat(0.5);
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    let params = vec![("emission_duration", Float(0.05))];
    let mut e = emitter("emit_continuously", params, &mut c);
    c.cur_time = 0.1;
    assert!(e.may_emit(&c));
    let made = step(&mut c, &mut e, 0.1, 1.0);
    assert_eq!(made.len(), 5);
    assert!(close(made[4], 0.05), "ends with the window");
    assert!(step(&mut c, &mut e, 0.2, 1.0).is_empty());
    assert!(!e.may_emit(&c));

    let late = vec![("emission_start_time", Float(0.15))];
    let mut e = emitter("emit_continuously", late, &mut c);
    assert!(step(&mut c, &mut e, 0.1, 1.0).is_empty(), "not started");
}

#[test]
fn instant_emission_releases_its_count_a_frame_at_a_time() {
    let table = flat(0.5);
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    let params = vec![
        ("num_to_emit", Int(5)),
        ("maximum emission per frame", Int(2)),
    ];
    let mut e = emitter("emit_instantaneously", params, &mut c);
    let counts: Vec<usize> = [0.1, 0.2, 0.3, 0.4]
        .map(|t| step(&mut c, &mut e, t, 1.0).len())
        .to_vec();
    assert_eq!(counts, [2, 2, 1, 0]);
    assert!(
        c.particles.iter().all(|p| p.creation == 0.0),
        "all at the start time"
    );
    assert!(!e.may_emit(&c));
}

#[test]
fn instant_emission_draws_its_count_and_waits_for_its_start() {
    let table = ramp();
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    c.seed = 6;
    let params = vec![
        ("num_to_emit", Int(3)),
        ("num_to_emit_minimum", Int(6)),
        ("emission_start_time", Float(0.15)),
    ];
    let mut e = emitter("emit_instantaneously", params, &mut c);
    assert_eq!(c.query_count, 1);
    assert!(step(&mut c, &mut e, 0.1, 1.0).is_empty());
    assert_eq!(
        step(&mut c, &mut e, 0.2, 1.0),
        [0.15; 6],
        "3 + (int)(4 * 0.75)"
    );
}

#[test]
fn instant_emission_scales_by_a_control_point() {
    let table = flat(0.5);
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    c.control_points[1].prev = [0.5, 0.0, 0.0];
    c.control_points[1].pos = [2.0, 0.0, 0.0];
    let params = vec![
        ("num_to_emit", Int(10)),
        ("emission count scale control point", Int(1)),
        ("maximum emission per frame", Int(2)),
    ];
    let mut e = emitter("emit_instantaneously", params, &mut c);
    let counts = [0.2, 0.3, 0.4, 0.5].map(|t| step(&mut c, &mut e, t, 1.0).len());
    assert_eq!(counts, [2, 2, 1, 0], "the point's last x halves the count");
    e.stop();
    assert!(!e.may_emit(&c));
}
