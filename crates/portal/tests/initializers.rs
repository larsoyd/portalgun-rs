mod support;

use std::cell::RefCell;
use std::f32::consts::PI;

use portal::dmx::Value::{self, Bool, Color, Float, Int, Vec3 as V3};
use portal::particles::collection::{Collection, Particle, Vec3};
use portal::particles::initializers::{initialize, Initializer, Query, LIFE, PREV};
use portal::particles::random::{pow_fixed, SimdRandom};
use support::*;

fn init(name: &str, params: Vec<(&str, Value)>) -> Initializer {
    Initializer::from_function(&function(name, params)).unwrap()
}

fn run(c: &mut Collection, inits: &mut [Initializer], n: usize) {
    c.particles = vec![c.template; n];
    initialize(c, inits, 0, n, 0, &Open);
}

/// Every trace runs its full length.
struct Open;

impl Query for Open {
    fn trace(&self, _: Vec3, _: Vec3, _: i32) -> f32 {
        1.0
    }
}

/// A wall a quarter of the trace away on +x, with every trace end kept.
struct WallEast(RefCell<Vec<Vec3>>);

impl Query for WallEast {
    fn trace(&self, start: Vec3, end: Vec3, _: i32) -> f32 {
        self.0.borrow_mut().push(end);
        if end[0] > start[0] {
            0.25
        } else {
            1.0
        }
    }
}

fn near(a: Vec3, b: Vec3, tol: f32) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < tol)
}

#[test]
fn random_attributes_read_the_table_in_order_after_the_seed() {
    let table = ramp();
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    c.seed = 1;
    let mut inits = [
        init(
            "Alpha Random",
            vec![
                ("alpha_min", Int(51)),
                ("alpha_max", Int(255)),
                ("alpha_random_exponent", Float(2.0)),
            ],
        ),
        init(
            "lifetime random",
            vec![("lifetime_min", Float(0.5)), ("lifetime_max", Float(1.5))],
        ),
        init(
            "Trail Length Random",
            vec![("length_min", Float(0.1)), ("length_max", Float(0.5))],
        ),
    ];
    run(&mut c, &mut inits, 2);
    let alpha: Vec<f32> = c.particles.iter().map(|p| p.alpha).collect();
    assert!(
        close(alpha[0], 0.2125) && close(alpha[1], 0.25),
        "{alpha:?}"
    );
    let life: Vec<f32> = c.particles.iter().map(|p| p.life).collect();
    assert_eq!(
        life,
        [0.875, 1.0],
        "the next draws, names matched without case"
    );
    let trail: Vec<f32> = c.particles.iter().map(|p| p.trail).collect();
    assert!(close(trail[0], 0.35) && close(trail[1], 0.4), "{trail:?}");
    assert_eq!(c.query_count, 6);
}

#[test]
fn bursts_of_sixteen_take_the_simd_path_for_their_aligned_middle() {
    let table = ramp();
    let mut c = collection(
        &table,
        SimdRandom::new(),
        vec![Particle::default(); 18],
        0.0,
    );
    let mut inits = [init(
        "Lifetime Random",
        vec![("lifetime_min", Float(1.0)), ("lifetime_max", Float(3.0))],
    )];
    initialize(&mut c, &mut inits, 2, 16, 0, &Open);
    let mut replay = SimdRandom::new();
    let mut want = vec![0.0, 0.0, 1.0, 1.25];
    for _ in 0..3 {
        want.extend(replay.draw().map(|r| 1.0 + 2.0 * r));
    }
    want.extend([1.5, 1.75]);
    let life: Vec<f32> = c.particles.iter().map(|p| p.life).collect();
    assert_eq!(
        life, want,
        "head and tail from the table, blocks from the stream"
    );
    assert_eq!(c.query_count, 4);
    c.query_count = 0;
    initialize(&mut c, &mut inits, 0, 15, 0, &Open);
    assert_eq!(c.query_count, 15, "fifteen stay on the table");
}

#[test]
fn the_block_path_keeps_its_own_quirks() {
    let table = ramp();
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    c.seed = 3;
    let rotation = || {
        init(
            "Rotation Random",
            vec![
                ("rotation_offset_max", Float(90.0)),
                ("rotation_initial", Float(180.0)),
            ],
        )
    };
    let radius = || {
        init(
            "Radius Random",
            vec![
                ("radius_min", Float(0.0)),
                ("radius_max", Float(1.0)),
                ("radius_random_exponent", Float(1.6)),
            ],
        )
    };
    let sequence = || {
        init(
            "Sequence Random",
            vec![("sequence_min", Int(0)), ("sequence_max", Int(15))],
        )
    };
    run(&mut c, &mut [rotation(), radius(), sequence()], 1);
    let p = c.particles[0];
    assert!(close(p.rotation, PI / 2.0 * 0.375 + PI), "{}", p.rotation);
    assert!(close(p.radius, 0.5f32.powf(1.6)), "{}", p.radius);
    assert_eq!(p.sequence, 10.0, "a whole frame from 16 * 0.625");

    run(&mut c, &mut [rotation(), radius(), sequence()], 16);
    let mut replay = SimdRandom::new();
    let draws: Vec<[f32; 4]> = (0..12).map(|_| replay.draw()).collect();
    let (rot, rad, seq) = (draws[0], draws[4], draws[8]);
    let p = c.particles[1];
    assert!(
        close(p.rotation, PI / 2.0 * rot[1]),
        "rotation_initial dropped"
    );
    assert!(
        close(p.radius, pow_fixed(rad[1], 1.5)),
        "exponent cut to 1.5"
    );
    assert_eq!(p.sequence, 15.0 * seq[1], "not a whole frame");
}

#[test]
fn colour_random_mixes_both_colours_with_one_draw() {
    let table = ramp();
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    c.seed = 2;
    let colours = || {
        init(
            "Color Random",
            vec![
                ("color1", Color([0, 0, 255, 255])),
                ("color2", Color([255, 255, 0, 255])),
            ],
        )
    };
    run(&mut c, &mut [colours()], 1);
    assert!(near(c.particles[0].tint, [0.25, 0.25, 0.75], 1e-6));
    run(&mut c, &mut [colours()], 16);
    let r = SimdRandom::new().draw()[2];
    assert!(near(c.particles[2].tint, [r, r, 1.0 - r], 1e-6), "a burst");
    let lit = function("Color Random", vec![("tint_perc", Float(0.5))]);
    assert_eq!(
        Initializer::from_function(&lit),
        None,
        "lighting tint refused"
    );
}

fn sphere(extra: Vec<(&'static str, Value)>) -> Initializer {
    let mut params = vec![
        ("distance_min", Float(10.0)),
        ("distance_max", Float(10.0)),
        ("speed_min", Float(100.0)),
        ("speed_max", Float(100.0)),
        ("speed_in_local_coordinate_system_min", V3([20.0, 0.0, 0.0])),
        ("speed_in_local_coordinate_system_max", V3([20.0, 0.0, 0.0])),
    ];
    params.extend(extra);
    init("Position Within Sphere Random", params)
}

#[test]
fn sphere_places_on_the_shell_and_sets_the_launch_speed() {
    let table = flat(0.5);
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    c.control_points[0].pos = [5.0, 0.0, 0.0];
    c.control_points[0].prev = [5.0, 0.0, 0.0];
    run(&mut c, &mut [sphere(vec![])], 1);
    let p = c.particles[0];
    assert!(near(p.xyz, [-5.0, 0.0, 0.0], 1e-2), "{:?}", p.xyz);
    assert!(near(p.prev, [5.0, -2.0, 0.0], 1e-2), "{:?}", p.prev);
    assert_eq!(c.query_count, 5);

    c.query_count = 0;
    c.particles = vec![c.template];
    initialize(&mut c, &mut [sphere(vec![])], 0, 1, PREV, &Open);
    assert_eq!(
        c.particles[0].prev, [0.0; 3],
        "previous position already set"
    );
    assert_eq!(c.query_count, 1);

    let local = vec![
        ("distance_bias", V3([1.0, 1.0, 0.5])),
        ("bias in local system", Bool(true)),
    ];
    run(&mut c, &mut [sphere(local)], 1);
    assert!(
        near(c.particles[0].xyz, [5.0, -10.0, 0.0], 1e-2),
        "{:?}",
        c.particles[0].xyz
    );

    let abs = vec![("distance_bias_absolute_value", V3([1.0, 0.0, 0.0]))];
    run(&mut c, &mut [sphere(abs)], 1);
    assert!(
        near(c.particles[0].xyz, [15.0, 0.0, 0.0], 1e-2),
        "{:?}",
        c.particles[0].xyz
    );
}

#[test]
fn sphere_draws_keep_clear_of_the_pole() {
    let table = flat(0.0);
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    run(&mut c, &mut [sphere(vec![])], 1);
    let p = c.particles[0].xyz;
    assert!(near(p, [0.2, 0.0, 9.998], 1e-3), "theta acos(0.9998) {p:?}");
}

#[test]
fn sphere_fills_the_ball_by_the_cube_root_and_skips_a_zero_speed() {
    let table = flat(0.5);
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    let filled = init(
        "Position Within Sphere Random",
        vec![("distance_max", Float(10.0))],
    );
    run(&mut c, &mut [filled], 1);
    let p = c.particles[0].xyz;
    let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    assert!(
        (len - 7.937_5).abs() < 1e-3,
        "ten times the cube root of 0.50005 {len}"
    );
    assert_eq!(c.query_count, 4, "no speed draw when the maximum is zero");
}

#[test]
fn sphere_bursts_use_the_simd_stream_unless_points_are_spread() {
    let table = flat(0.5);
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    c.control_points[0].pos = [5.0, 0.0, 0.0];
    c.control_points[0].prev = [5.0, 0.0, 0.0];
    let still = || {
        init(
            "Position Within Sphere Random",
            vec![("distance_min", Float(10.0)), ("distance_max", Float(10.0))],
        )
    };
    run(&mut c, &mut [still()], 16);
    for p in &c.particles {
        let d = [p.xyz[0] - 5.0, p.xyz[1], p.xyz[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        assert!((len - 10.0).abs() < 1e-4, "on the shell {len}");
        assert_eq!(p.prev, p.xyz, "no speed");
    }
    assert_eq!(c.query_count, 0);
    let mut replay = SimdRandom::new();
    let r = [replay.draw(), replay.draw(), replay.draw()];
    let lane = (0..4)
        .find(|&l| (0..3).map(|k| (2.0 * r[k][l] - 1.0).powi(2)).sum::<f32>() <= 1.0)
        .unwrap();
    let v = [0, 1, 2].map(|k| 2.0 * r[k][lane] - 1.0);
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let want = [
        5.0 + 10.0 * v[0] / len,
        10.0 * v[1] / len,
        10.0 * v[2] / len,
    ];
    assert!(near(c.particles[lane].xyz, want, 1e-4), "lane {lane}");

    let mut c = collection(&table, SimdRandom::new(), vec![], 0.1);
    c.control_points[0].prev = [5.0, 0.0, 0.0];
    c.control_points[0].pos = [7.0, 0.0, 0.0];
    c.template.creation = 0.05;
    let filled = init(
        "Position Within Sphere Random",
        vec![("distance_max", Float(10.0))],
    );
    run(&mut c, &mut [filled], 16);
    let want = [6.0 + 10.0 * v[0], 10.0 * v[1], 10.0 * v[2]];
    assert!(
        near(c.particles[lane].xyz, want, 1e-4),
        "half way along the point's move"
    );

    let filled = || {
        init(
            "Position Within Sphere Random",
            vec![("distance_max", Float(10.0))],
        )
    };
    run(&mut c, &mut [filled()], 400);
    for p in &c.particles {
        let d = [p.xyz[0] - 6.0, p.xyz[1], p.xyz[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        assert!(len <= 10.0, "inside the ball {len}");
    }

    let spread = vec![(
        "randomly distribute to highest supplied Control Point",
        Bool(true),
    )];
    c.query_count = 0;
    run(&mut c, &mut [sphere(spread)], 16);
    assert_eq!(c.query_count, 16 * 6, "the scalar path for every particle");
}

#[test]
fn offset_draws_one_vector_with_the_seed_added_twice() {
    let table = ramp();
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    c.seed = 1;
    c.template.xyz = [1.0, 1.0, 1.0];
    let offset = init(
        "Position Modify Offset Random",
        vec![
            ("offset min", V3([1.0, 2.0, 3.0])),
            ("offset max", V3([3.0, 4.0, 5.0])),
            ("offset in local space 0/1", Bool(true)),
            ("offset proportional to radius 0/1", Bool(true)),
        ],
    );
    c.template.radius = 2.0;
    run(&mut c, &mut [offset], 1);
    assert_eq!(c.particles[0].xyz, [-4.5, 4.0, 9.0]);
    assert_eq!(c.particles[0].prev, [-5.5, 3.0, 8.0]);
    assert_eq!(c.query_count, 1);
}

#[test]
fn warp_scales_in_the_control_points_frame() {
    let table = flat(0.5);
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    c.template.xyz = [2.0, 4.0, 6.0];
    c.template.prev = [2.0, 4.0, 6.0];
    let warp = |extra: Vec<(&'static str, Value)>| {
        let mut params = vec![
            ("warp min", V3([1.0, 1.0, 1.0])),
            ("warp max", V3([0.0, 1.0, 1.0])),
        ];
        params.extend(extra);
        init("Position Modify Warp Random", params)
    };
    run(&mut c, &mut [warp(vec![])], 1);
    assert_eq!(
        c.particles[0].xyz,
        [2.0, 2.0, 6.0],
        "half way along forward"
    );
    assert_eq!(c.particles[0].prev, [2.0, 2.0, 6.0]);
    assert_eq!(c.query_count, 1);

    c.template.creation = 0.006_25;
    let timed = vec![
        (
            "warp transition time (treats min/max as start/end sizes)",
            Float(0.025),
        ),
        ("reverse warp (0/1)", Bool(true)),
    ];
    run(&mut c, &mut [warp(timed)], 1);
    assert_eq!(
        c.particles[0].xyz,
        [2.0, 1.0, 6.0],
        "a quarter in, reversed"
    );
    assert_eq!(c.query_count, 1, "no draw when timed");
}

#[test]
fn remap_scalar_maps_creation_time_onto_an_attribute() {
    let table = flat(0.5);
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    let remap = |extra: Vec<(&'static str, Value)>| {
        let mut params = vec![
            ("input maximum", Float(0.25)),
            ("output minimum", Float(15.0)),
            ("output maximum", Float(5.0)),
        ];
        params.extend(extra);
        init("remap initial scalar", params)
    };
    c.template.radius = 2.0;
    c.template.creation = 0.125;
    run(&mut c, &mut [remap(vec![])], 1);
    assert_eq!(c.particles[0].radius, 10.0);
    run(
        &mut c,
        &mut [remap(vec![(
            "output is scalar of initial random range",
            Bool(true),
        )])],
        1,
    );
    assert_eq!(c.particles[0].radius, 20.0);
    let window = vec![
        ("emitter lifetime start time (seconds)", Float(0.2)),
        ("emitter lifetime end time (seconds)", Float(0.3)),
    ];
    run(&mut c, &mut [remap(window)], 1);
    assert_eq!(c.particles[0].radius, 2.0, "outside the emitter window");
    c.template.creation = 0.5;
    run(
        &mut c,
        &mut [remap(vec![(
            "only active within specified input range",
            Bool(true),
        )])],
        1,
    );
    assert_eq!(c.particles[0].radius, 2.0, "input out of range");
    c.template.creation = 0.25;
    let alpha = init(
        "Remap Initial Scalar",
        vec![
            ("input maximum", Float(0.25)),
            ("output field", Int(7)),
            ("output maximum", Float(2.0)),
        ],
    );
    run(&mut c, &mut [alpha], 1);
    assert_eq!(c.particles[0].alpha, 1.0, "alpha outputs clamp to one");
}

#[test]
fn remap_to_vector_places_along_the_control_point() {
    let table = flat(0.5);
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    c.control_points[0].pos = [1.0, 2.0, 3.0];
    c.dt = 0.0;
    c.template.creation = 0.0125;
    let remap = |local| {
        init(
            "remap scalar to vector",
            vec![
                ("input maximum", Float(0.025)),
                ("output maximum", V3([32.0, 0.0, 0.0])),
                ("use local system", Bool(local)),
            ],
        )
    };
    run(&mut c, &mut [remap(true)], 1);
    assert_eq!(c.particles[0].xyz, [1.0, 18.0, 3.0]);
    assert_eq!(c.particles[0].prev, [1.0, 18.0, 3.0]);
    run(&mut c, &mut [remap(false)], 1);
    assert_eq!(c.particles[0].xyz, [17.0, 2.0, 3.0]);
}

fn repulse(extra: Vec<(&'static str, Value)>) -> Initializer {
    let mut params = vec![
        ("maximum velocity", V3([2.0, 2.0, 2.0])),
        ("Trace Length", Float(16.0)),
        ("collision group", Value::String("DEBRIS".into())),
    ];
    params.extend(extra);
    init("Velocity Repulse from World", params)
}

#[test]
fn repulse_pushes_away_from_close_walls() {
    let table = flat(0.5);
    let mut c = collection(&table, SimdRandom::new(), vec![Particle::default(); 2], 0.0);
    c.template.radius = 2.0;
    c.particles = vec![c.template; 2];
    let wall = WallEast(RefCell::new(vec![]));
    initialize(&mut c, &mut [repulse(vec![])], 0, 2, 0, &wall);
    for p in &c.particles {
        assert_eq!(p.xyz, [-1.125, 0.0, 0.0]);
        assert_eq!(p.prev, [0.0; 3], "an implied speed, not an offset");
    }
    assert_eq!(wall.0.borrow().len(), 6, "one set of traces for the call");

    c.particles = vec![c.template; 2];
    c.particles[1].radius = 3.0;
    let scaled = repulse(vec![("Offset proportional to radius 0/1", Bool(true))]);
    initialize(&mut c, &mut [scaled], 0, 2, 0, &wall);
    let x: Vec<f32> = c.particles.iter().map(|p| p.xyz[0]).collect();
    assert_eq!(x, [-2.25, -6.75], "the radius scale builds up");
}

#[test]
fn repulse_per_particle_reuses_the_shrunk_directions() {
    let table = flat(0.5);
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    c.particles = vec![c.template; 2];
    let wall = WallEast(RefCell::new(vec![]));
    let each = repulse(vec![
        ("Per Particle World Collision Tests", Bool(true)),
        ("Offset instead of accelerate", Bool(true)),
    ]);
    initialize(&mut c, &mut [each], 0, 2, 0, &wall);
    assert_eq!(c.particles[0].xyz, [-1.125, 0.0, 0.0]);
    assert_eq!(
        c.particles[0].prev,
        [-1.125, 0.0, 0.0],
        "moved as an offset"
    );
    let ends = wall.0.borrow();
    assert_eq!(ends[6], [-12.0, 0.0, 0.0], "east trace now points west");
    assert_eq!(ends[7], [0.0; 3], "west trace collapsed to nothing");
    assert_eq!(c.particles[1].xyz, [0.0; 3], "no hits left to push it");
}

#[test]
fn modifiers_run_last_and_a_second_writer_is_skipped() {
    let table = flat(0.5);
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    c.control_points[0].pos = [5.0, 0.0, 0.0];
    c.control_points[0].prev = [5.0, 0.0, 0.0];
    let life = |v| {
        init(
            "Lifetime Random",
            vec![("lifetime_min", Float(v)), ("lifetime_max", Float(v))],
        )
    };
    let mut inits = [
        init(
            "Position Modify Offset Random",
            vec![
                ("offset min", V3([1.0, 0.0, 0.0])),
                ("offset max", V3([1.0, 0.0, 0.0])),
            ],
        ),
        init("Position Within Sphere Random", vec![]),
        life(1.0),
        life(7.0),
    ];
    run(&mut c, &mut inits, 1);
    assert_eq!(
        c.particles[0].xyz,
        [6.0, 0.0, 0.0],
        "offset after the sphere"
    );
    assert_eq!(c.particles[0].life, 1.0, "the second lifetime skipped");

    c.particles = vec![c.template];
    initialize(&mut c, &mut inits, 0, 1, LIFE, &Open);
    assert_eq!(
        c.particles[0].life, 0.0,
        "already initialised by the caller"
    );
}

#[test]
fn sphere_bursts_launch_along_the_direction_even_below_zero_speed() {
    let table = flat(0.5);
    let mut c = collection(&table, SimdRandom::new(), vec![], 0.0);
    let launch = init(
        "Position Within Sphere Random",
        vec![
            ("distance_min", Float(10.0)),
            ("distance_max", Float(10.0)),
            ("speed_min", Float(-30.0)),
            ("speed_max", Float(-30.0)),
            ("speed_in_local_coordinate_system_min", V3([20.0, 0.0, 0.0])),
            ("speed_in_local_coordinate_system_max", V3([20.0, 0.0, 0.0])),
        ],
    );
    run(&mut c, &mut [launch], 16);
    for p in &c.particles {
        let dir = p.xyz.map(|v| v / 10.0);
        let vel = [-30.0 * dir[0], -30.0 * dir[1] + 20.0, -30.0 * dir[2]];
        let want = [0, 1, 2].map(|k| p.xyz[k] - vel[k] * 0.1);
        assert!(near(p.prev, want, 1e-4), "{:?} {want:?}", p.prev);
    }
}
