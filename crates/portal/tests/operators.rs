// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

mod support;

fn op(name: &str, params: Vec<(&str, Value)>) -> Operator {
    Operator::from_function(&function(name, params)).unwrap()
}

use portal::dmx::Value;
use portal::particles::collection::{strength, Fade};
use portal::particles::operators::{parabolic_sine, Operator};
use portal::particles::random::SimdRandom;
use support::*;

#[test]
fn the_fade_window_scales_and_gates_an_operator() {
    let none = Fade::default();
    assert_eq!(strength(&none, 5.0), Some(1.0));
    let fade_in = Fade {
        start_in: 1.0,
        end_in: 3.0,
        ..none
    };
    assert_eq!(strength(&fade_in, 2.0), Some(0.5));
    assert_eq!(strength(&fade_in, 0.5), None, "before it starts");
    let fade_out = Fade {
        start_out: 2.0,
        end_out: 4.0,
        ..none
    };
    assert_eq!(strength(&fade_out, 3.0), Some(0.5));
    assert_eq!(strength(&fade_out, 4.5), None, "after it ends");
    let both = Fade {
        start_in: 0.0,
        end_in: 2.0,
        start_out: 1.0,
        end_out: 4.0,
        ..none
    };
    assert_eq!(
        strength(&both, 1.0),
        Some(0.5),
        "fade out starts at the end of fade in"
    );
    assert_eq!(strength(&both, 3.0), Some(0.5));
    let looping = Fade {
        start_in: 0.25,
        end_in: 0.75,
        oscillate: 2.0,
        ..none
    };
    assert_eq!(
        strength(&looping, 3.0),
        Some(0.5),
        "3 s into a 2 s period is half way"
    );
}

#[test]
fn kills_fill_holes_from_the_tail_like_source() {
    let table = flat(0.5);
    let ps = (0..6).map(|i| particle(i, 0.0, 1.0)).collect();
    let mut c = collection(&table, SimdRandom::new(), ps, 0.0);
    c.kills = vec![1, 4, 5];
    c.apply_kills();
    assert_eq!(
        c.particles.iter().map(|p| p.id).collect::<Vec<_>>(),
        [0, 3, 2]
    );
    c.kills = vec![0, 9];
    c.apply_kills();
    assert_eq!(
        c.particles.iter().map(|p| p.id).collect::<Vec<_>>(),
        [2, 3],
        "padding lanes are dropped"
    );
}

#[test]
fn lifespan_decay_kills_the_old_and_the_lifeless() {
    let table = flat(0.5);
    let ps = vec![
        particle(0, 0.0, 1.0),
        particle(1, 0.0, 2.0),
        particle(2, 1.5, 0.0),
    ];
    let mut c = collection(&table, SimdRandom::new(), ps, 1.0);
    op("Lifespan Decay", vec![]).operate(&mut c, 1.0);
    assert_eq!(c.kills, [0, 2]);
}

#[test]
fn alpha_fade_and_decay_eases_in_and_out() {
    let table = flat(0.5);
    let params = vec![
        ("start_alpha", Value::Float(0.5)),
        ("end_alpha", Value::Float(0.0)),
    ];
    let mut fade = op("Alpha Fade and Decay", params);
    let mut c = collection(
        &table,
        SimdRandom::new(),
        vec![particle(0, 0.0, 1.0), particle(1, -0.5, 1.0)],
        0.25,
    );
    fade.operate(&mut c, 1.0);
    // half way through the fade in from half alpha: the spline gives 0.5 + 0.5 * 0.5
    assert!(
        close(c.particles[0].alpha, 0.75),
        "{}",
        c.particles[0].alpha
    );
    assert!(close(c.particles[1].alpha, 0.5), "{}", c.particles[1].alpha);
    c.cur_time = 1.0;
    fade.operate(&mut c, 1.0);
    assert_eq!(c.kills, [0, 1]);
}

#[test]
fn alpha_fade_in_takes_a_random_time() {
    let table = flat(0.5);
    let params = vec![
        ("fade in time min", Value::Float(0.2)),
        ("fade in time max", Value::Float(0.4)),
    ];
    let mut c = collection(
        &table,
        SimdRandom::new(),
        vec![particle(0, 0.0, 1.0), particle(1, 0.0, 0.05)],
        0.075,
    );
    op("Alpha Fade In Random", params).operate(&mut c, 1.0);
    // a quarter into a 0.3 fade: (3 - 0.5) * 0.0625
    assert!(
        close(c.particles[0].alpha, 0.15625),
        "{}",
        c.particles[0].alpha
    );
    assert_eq!(c.particles[1].alpha, 1.0, "past its fade in");
}

#[test]
fn alpha_fade_out_eases_or_follows_its_bias() {
    let table = flat(0.5);
    // a quarter into the last 0.25 of life
    for (ease, bias, want) in [
        (true, 0.5, 0.84375),
        (false, 0.5, 0.75),
        (false, 0.25, 1.0 - 0.25 / (0.75 * 2.0 + 1.0)),
    ] {
        let params = vec![
            ("ease in and out", Value::Bool(ease)),
            ("fade bias", Value::Float(bias)),
        ];
        let mut c = collection(
            &table,
            SimdRandom::new(),
            vec![particle(0, 0.0, 1.0)],
            0.8125,
        );
        op("Alpha Fade Out Random", params).operate(&mut c, 1.0);
        assert!(
            close(c.particles[0].alpha, want),
            "ease {ease} bias {bias}: {}",
            c.particles[0].alpha
        );
    }
}

#[test]
fn color_fade_blends_from_the_initial_tint() {
    let table = flat(0.5);
    let mut p = particle(0, 0.0, 1.0);
    p.initial.tint = [1.0, 0.0, 0.0];
    let params = vec![
        ("color_fade", Value::Color([0, 0, 255, 255])),
        ("ease_in_and_out", Value::Bool(false)),
    ];
    let mut c = collection(&table, SimdRandom::new(), vec![p], 0.25);
    op("Color Fade", params).operate(&mut c, 1.0);
    let t = c.particles[0].tint;
    assert!(
        close(t[0], 0.75) && close(t[1], 0.0) && close(t[2], 0.25),
        "{t:?}"
    );
}

#[test]
fn cull_random_picks_by_the_particles_randoms() {
    let low = flat(0.0);
    let mut c = collection(&low, SimdRandom::new(), vec![particle(0, 0.0, 1.0)], 0.5);
    op("Cull Random", vec![]).operate(&mut c, 1.0);
    assert_eq!(c.kills, [0]);
    let high = flat(0.9);
    let mut c = collection(&high, SimdRandom::new(), vec![particle(0, 0.0, 1.0)], 0.5);
    op("Cull Random", vec![]).operate(&mut c, 1.0);
    assert!(c.kills.is_empty(), "0.9 is above half the cull percentage");
    let mid = flat(0.3);
    let mut c = collection(&mid, SimdRandom::new(), vec![particle(0, 0.0, 1.0)], 0.5);
    op("Cull Random", vec![]).operate(&mut c, 0.5);
    assert!(
        c.kills.is_empty(),
        "the operator's fade lowers the share culled"
    );
    let mut c = collection(&low, SimdRandom::new(), vec![particle(0, 0.0, 1.0)], 0.5);
    op("Cull Random", vec![("Cull Start Time", Value::Float(0.6))]).operate(&mut c, 1.0);
    assert!(c.kills.is_empty(), "before the cull window");
    let mut c = collection(&low, SimdRandom::new(), vec![particle(0, 0.0, 1.0)], 0.5);
    op("Cull Random", vec![("Cull End Time", Value::Float(0.4))]).operate(&mut c, 1.0);
    assert!(c.kills.is_empty(), "after the cull window");
}

#[test]
fn dampen_pulls_velocity_down_near_its_point() {
    let table = flat(0.5);
    let mut p = particle(0, 0.0, 1.0);
    p.xyz = [50.0, 0.0, 0.0];
    p.prev = [40.0, 0.0, 0.0];
    let mut far = particle(1, 0.0, 1.0);
    far.xyz = [150.0, 0.0, 0.0];
    let mut c = collection(&table, SimdRandom::new(), vec![p, far], 0.5);
    op("Movement Dampen Relative to Control Point", vec![]).operate(&mut c, 1.0);
    assert!(
        close(c.particles[0].xyz[0], 45.0),
        "{:?}",
        c.particles[0].xyz
    );
    assert_eq!(c.particles[0].prev, [40.0, 0.0, 0.0]);
    assert_eq!(c.particles[1].xyz[0], 150.0, "outside the falloff range");
}

#[test]
fn radius_scale_runs_linear_eased_or_biased() {
    let table = flat(0.5);
    for (ease, bias, want) in [(false, 0.5, 3.0), (true, 0.5, 2.625), (false, 0.25, 2.4)] {
        let params = vec![
            ("radius_end_scale", Value::Float(3.0)),
            ("ease_in_and_out", Value::Bool(ease)),
            ("scale_bias", Value::Float(bias)),
        ];
        let mut c = collection(&table, SimdRandom::new(), vec![particle(0, 0.0, 1.0)], 0.25);
        op("Radius Scale", params).operate(&mut c, 1.0);
        assert!(
            close(c.particles[0].radius, want),
            "ease {ease} bias {bias}: {}",
            c.particles[0].radius
        );
    }
}

#[test]
fn the_parabolic_sine_follows_sources_shape() {
    for (x, want) in [
        (0.0, 0.0),
        (0.25, 0.75),
        (0.5, 1.0),
        (1.0, 0.0),
        (1.5, -1.0),
        (-0.5, -1.0),
        (2.25, 0.75),
    ] {
        assert!(
            close(parabolic_sine(x), want),
            "S({x}) = {}",
            parabolic_sine(x)
        );
    }
}

#[test]
fn oscillate_vector_pushes_along_a_sine() {
    let table = flat(0.5);
    let params = vec![
        ("oscillation rate min", Value::Vec3([1.0, 0.0, 0.0])),
        ("oscillation rate max", Value::Vec3([1.0, 0.0, 0.0])),
        ("proportional 0/1", Value::Bool(false)),
    ];
    let mut c = collection(
        &table,
        SimdRandom::new(),
        vec![particle(0, -0.1, 1.0)],
        0.125,
    );
    op("Oscillate Vector", params).operate(&mut c, 1.0);
    // the sine's argument is 0.125 times two plus half, where it reads 0.75
    assert!(
        close(c.particles[0].xyz[0], 0.075),
        "{:?}",
        c.particles[0].xyz
    );
}

#[test]
fn position_lock_carries_particles_with_their_point() {
    let table = flat(0.5);
    let mut lock = op("Movement Lock to Control Point", vec![]);
    let mut c = collection(
        &table,
        SimdRandom::new(),
        vec![particle(0, 0.0, 1.0), particle(1, 0.45, 1.0)],
        0.5,
    );
    lock.operate(&mut c, 1.0);
    assert_eq!(
        c.particles[0].xyz, [0.0; 3],
        "the first frame only remembers the point"
    );
    c.control_points[0].pos = [10.0, 0.0, 0.0];
    lock.operate(&mut c, 1.0);
    assert!(close(c.particles[0].xyz[0], 10.0) && close(c.particles[0].prev[0], 10.0));
    assert!(
        close(c.particles[1].xyz[0], 5.0),
        "a particle born mid step moves part way: {:?}",
        c.particles[1].xyz
    );
}

#[test]
fn position_lock_can_turn_with_its_point() {
    let table = flat(0.5);
    let mut lock = op(
        "Movement Lock to Control Point",
        vec![("lock rotation", Value::Bool(true))],
    );
    let mut p = particle(0, 0.0, 1.0);
    p.xyz = [1.0, 0.0, 0.0];
    let mut c = collection(&table, SimdRandom::new(), vec![p], 0.5);
    lock.operate(&mut c, 1.0);
    let cp = &mut c.control_points[0];
    (cp.forward, cp.right) = ([-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    lock.operate(&mut c, 1.0);
    let v = c.particles[0].xyz;
    assert!(
        close(v[0], 0.0) && close(v[1], 1.0) && close(v[2], 0.0),
        "{v:?}"
    );
}

#[test]
fn position_lock_fades_over_a_drawn_part_of_life() {
    let table = flat(0.5);
    let params = vec![
        ("start_fadeout_min", Value::Float(0.0)),
        ("start_fadeout_max", Value::Float(0.0)),
        ("end_fadeout_min", Value::Float(1.0)),
        ("end_fadeout_max", Value::Float(1.0)),
    ];
    let mut lock = op("Movement Lock to Control Point", params);
    let mut c = collection(&table, SimdRandom::new(), vec![particle(0, 0.0, 1.0)], 0.5);
    lock.operate(&mut c, 1.0);
    c.control_points[0].pos = [10.0, 0.0, 0.0];
    lock.operate(&mut c, 1.0);
    assert!(
        close(c.particles[0].xyz[0], 5.0),
        "half way through life it follows half way: {:?}",
        c.particles[0].xyz
    );
}

#[test]
fn names_match_without_case_as_client_so_compares_them() {
    assert_eq!(op("LIFESPAN decay", vec![]), Operator::Decay);
    assert!(Operator::from_function(&function("Lifespan Decays", vec![])).is_none());
}
