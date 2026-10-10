mod support;

use portal::dmx::Value::{Float, Int};
use portal::particles::collection::{Particle, Vec3};
use portal::particles::random::SimdRandom;
use portal::particles::renderers::{byte, Camera, Card, Look, Renderer};
use portal::particles::sheet::Sheet;
use support::*;

fn renderer(name: &str, params: Vec<(&str, portal::dmx::Value)>) -> Renderer {
    Renderer::from_function(&function(name, params)).unwrap()
}

fn looking_north() -> Camera {
    Camera {
        pos: [0.0, 0.0, 0.0],
        right: [1.0, 0.0, 0.0],
        up: [0.0, 0.0, 1.0],
    }
}

fn at(pos: Vec3, radius: f32) -> Particle {
    Particle {
        xyz: pos,
        prev: pos,
        radius,
        alpha: 1.0,
        alpha2: 1.0,
        tint: [1.0, 0.5, 0.0],
        ..Particle::default()
    }
}

fn plain() -> Look<'static> {
    Look {
        height: 64.0,
        ..Look::default()
    }
}

fn rgba(b: [u8; 4]) -> [f32; 4] {
    b.map(|v| v as f32 / 255.0)
}

fn near(a: Vec3, b: Vec3) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < 1e-5)
}

#[test]
fn bytes_round_to_nearest_and_wrap() {
    assert_eq!(byte(127.5), 128, "ties to even");
    assert_eq!(byte(126.5), 126);
    assert_eq!(byte(306.0), 50, "past 255 wraps");
    assert_eq!(byte(0.2), 0);
}

#[test]
fn sprites_face_the_view_and_turn_by_their_roll() {
    let table = flat(0.5);
    let mut p = at([0.0, 10.0, 0.0], 2.0);
    let c = collection(&table, SimdRandom::new(), vec![p], 0.0);
    let sprites = renderer("render_animated_sprites", vec![]);
    let m = sprites.build(&c, &looking_north(), &plain());
    let corners: Vec<(Vec3, [f32; 2])> = m.vertices.iter().map(|v| (v.pos, v.uv)).collect();
    assert_eq!(
        corners,
        [
            ([-2.0, 10.0, -2.0], [0.0, 1.0]),
            ([-2.0, 10.0, 2.0], [0.0, 0.0]),
            ([2.0, 10.0, 2.0], [1.0, 0.0]),
            ([2.0, 10.0, -2.0], [1.0, 1.0]),
        ]
    );
    assert_eq!(m.indices, [0, 1, 2, 0, 2, 3]);
    assert_eq!(m.vertices[0].color, rgba([255, 128, 0, 255]));

    p.rotation = std::f32::consts::FRAC_PI_2;
    let c = collection(&table, SimdRandom::new(), vec![p], 0.0);
    let m = sprites.build(&c, &looking_north(), &plain());
    assert!(
        near(m.vertices[0].pos, [2.0, 10.0, -2.0]),
        "{:?}",
        m.vertices[0].pos
    );
}

#[test]
fn sprites_draw_far_to_near_and_skip_the_invisible() {
    let table = flat(0.5);
    let mut faint = at([0.0, 30.0, 0.0], 1.0);
    faint.alpha = 0.001;
    let mut over = at([0.0, 5.0, 0.0], 1.0);
    over.alpha = 1.2;
    let mut half = at([0.0, 20.0, 0.0], 1.0);
    half.alpha2 = 0.5;
    let particles = vec![at([0.0, 10.0, 0.0], 1.0), faint, half, over];
    let c = collection(&table, SimdRandom::new(), particles, 0.0);
    let m = renderer("render_animated_sprites", vec![]).build(&c, &looking_north(), &plain());
    let order: Vec<(f32, u8)> = m
        .vertices
        .iter()
        .step_by(4)
        .map(|v| (v.pos[1], byte(v.color[3] * 255.0)))
        .collect();
    assert_eq!(
        order,
        [(20.0, 128), (10.0, 255), (5.0, 50)],
        "1.2 wraps to 50"
    );
}

#[test]
fn other_orientations_are_refused() {
    let side = function(
        "render_animated_sprites",
        vec![("orientation_type", Int(1))],
    );
    assert_eq!(Renderer::from_function(&side), None);
    let proxy = function(
        "render_sprite_trail",
        vec![("Visibility Proxy Input Control Point Number", Int(2))],
    );
    assert_eq!(Renderer::from_function(&proxy), None);
}

#[test]
fn trails_stretch_back_along_the_motion() {
    let table = flat(0.5);
    let mut p = at([0.0, 0.0, 0.0], 2.0);
    p.prev = [0.0, -1.0, 0.0];
    p.trail = 0.5;
    let cam = Camera {
        pos: [10.0, 0.0, 0.0],
        ..looking_north()
    };
    let c = collection(&table, SimdRandom::new(), vec![p], 1.0);
    let trail = |extra| renderer("render_sprite_trail", extra);
    let m = trail(vec![]).build(&c, &cam, &plain());
    let corners: Vec<(Vec3, [f32; 2])> = m.vertices.iter().map(|v| (v.pos, v.uv)).collect();
    let want = [
        ([0.0, 0.0, 1.0], [0.0, 1.0]),
        ([0.0, 0.0, -1.0], [1.0, 1.0]),
        ([0.0, -5.0, -1.0], [1.0, 0.0]),
        ([0.0, -5.0, 1.0], [0.0, 0.0]),
    ];
    for (got, want) in corners.iter().zip(want) {
        assert!(near(got.0, want.0) && got.1 == want.1, "{got:?} {want:?}");
    }
    let m = trail(vec![("max length", Float(1.5))]).build(&c, &cam, &plain());
    assert!(
        near(m.vertices[2].pos, [0.0, -1.5, -0.75]),
        "width capped by length {:?}",
        m.vertices[2].pos
    );
    let m = trail(vec![("length fade in time", Float(2.0))]).build(&c, &cam, &plain());
    assert!(
        near(m.vertices[3].pos, [0.0, -2.5, 1.0]),
        "half faded in {:?}",
        m.vertices[3].pos
    );
    let m = trail(vec![("min length", Float(8.0))]).build(&c, &cam, &plain());
    assert!(
        near(m.vertices[3].pos, [0.0, -8.0, 1.0]),
        "at least the minimum {:?}",
        m.vertices[3].pos
    );
    let mut c = collection(&table, SimdRandom::new(), vec![p], 1.0);
    c.dt = 0.0;
    let m = trail(vec![]).build(&c, &cam, &plain());
    assert!(
        near(m.vertices[3].pos, [0.0, -0.5, 0.25]),
        "a stopped clock counts one {:?}",
        m.vertices[3].pos
    );
}

#[test]
fn ropes_run_through_the_particles_facing_the_camera() {
    let table = flat(0.5);
    let mut a = at([0.0, 0.0, 0.0], 2.0);
    let mut b = at([0.0, 10.0, 0.0], 2.0);
    a.alpha = 0.5;
    b.alpha = 0.5;
    b.tint = [0.0, 0.5, 1.0];
    let cam = Camera {
        pos: [10.0, 5.0, 0.0],
        ..looking_north()
    };
    let c = collection(&table, SimdRandom::new(), vec![a, b], 0.0);
    let rope = |subdivisions, texel| {
        renderer(
            "render_rope",
            vec![
                ("subdivision_count", Int(subdivisions)),
                ("texel_size", Float(texel)),
            ],
        )
    };
    let m = rope(1, 4.0).build(&c, &cam, &plain());
    let verts: Vec<(Vec3, [f32; 2])> = m.vertices.iter().map(|v| (v.pos, v.uv)).collect();
    let want = [
        ([0.0, 0.0, -1.0], [0.0, 0.0]),
        ([0.0, 0.0, 1.0], [1.0, 0.0]),
        ([0.0, 10.0, -1.0], [0.0, 10.0 / 256.0]),
        ([0.0, 10.0, 1.0], [1.0, 10.0 / 256.0]),
    ];
    for (got, want) in verts.iter().zip(want) {
        assert!(near(got.0, want.0) && got.1 == want.1, "{got:?} {want:?}");
    }
    assert_eq!(m.indices, [0, 1, 2, 1, 3, 2]);
    assert_eq!(
        m.vertices[0].color,
        rgba([255, 128, 0, 128]),
        "alpha from the particle alone"
    );

    let m = rope(2, 4.0).build(&c, &cam, &plain());
    assert_eq!(m.vertices.len(), 6, "one segment between");
    assert!(
        near(m.vertices[2].pos, [0.0, 5.0, -1.0]),
        "{:?}",
        m.vertices[2].pos
    );
    assert_eq!(m.vertices[2].uv, [0.0, 5.0 / 256.0]);
    assert_eq!(
        m.vertices[2].color,
        rgba([128, 128, 128, 128]),
        "colour blended between"
    );
    assert_eq!(
        rope(0, 0.0).build(&c, &cam, &plain()).vertices[2].uv,
        [0.0, 10.0 / 64.0],
        "texel and count at least one"
    );
}

#[test]
fn ropes_bend_by_catmull_rom_between_points() {
    let table = flat(0.5);
    let ps = vec![
        at([0.0, 0.0, 0.0], 1.0),
        at([0.0, 10.0, 0.0], 1.0),
        at([0.0, 10.0, 10.0], 1.0),
    ];
    let cam = Camera {
        pos: [100.0, 0.0, 0.0],
        ..looking_north()
    };
    let c = collection(&table, SimdRandom::new(), ps, 0.0);
    let m = renderer("render_rope", vec![("subdivision_count", Int(2))]).build(&c, &cam, &plain());
    let mid = |k: usize| {
        let (a, b) = (m.vertices[2 * k].pos, m.vertices[2 * k + 1].pos);
        [0, 1, 2].map(|i| (a[i] + b[i]) / 2.0)
    };
    assert!(near(mid(1), [0.0, 5.0, -0.625]), "{:?}", mid(1));
    assert!(near(mid(3), [0.0, 10.625, 5.0]), "{:?}", mid(3));

    let m = renderer("render_rope", vec![("subdivision_count", Int(1))]).build(&c, &cam, &plain());
    let close3 = |a: Vec3, b: Vec3| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-3);
    assert!(
        close3(m.vertices[2].pos, [0.0352, 10.3518, -0.3536]),
        "averaged edge {:?}",
        m.vertices[2].pos
    );
    assert!(
        close3(m.vertices[4].pos, [0.0498, 10.4975, 10.0]),
        "the last keeps its own {:?}",
        m.vertices[4].pos
    );
}

/// A version 0 sheet: one rect a frame, each frame lasting one unit. Sequence 5 wraps.
fn sheet(sequences: &[(i32, Vec<[f32; 4]>)]) -> Sheet {
    let mut b = Vec::new();
    b.extend(0i32.to_le_bytes());
    b.extend((sequences.len() as i32).to_le_bytes());
    for (seq, frames) in sequences {
        b.extend(seq.to_le_bytes());
        b.extend(((*seq != 5) as i32).to_le_bytes());
        b.extend((frames.len() as i32).to_le_bytes());
        b.extend((frames.len() as f32).to_le_bytes());
        for rect in frames {
            b.extend(1.0f32.to_le_bytes());
            rect.iter().for_each(|v| b.extend(v.to_le_bytes()));
        }
    }
    Sheet::parse(&b).unwrap()
}

#[test]
fn sprites_pick_their_sheet_frame_by_age_and_sequence() {
    let table = flat(0.5);
    let (f0, f1) = ([0.0, 0.0, 0.5, 0.5], [0.5, 0.0, 1.0, 0.5]);
    let one = [0.0, 0.5, 0.25, 1.0];
    let sheet = sheet(&[(0, vec![f0, f1]), (3, vec![one]), (5, vec![f0, f1])]);
    let look = Look {
        sheet: Some(&sheet),
        ..plain()
    };
    let sprites = renderer(
        "render_animated_sprites",
        vec![("animation rate", Float(0.5))],
    );
    let uv_at = |age: f32, seq: f32| {
        let mut p = at([0.0, 10.0, 0.0], 1.0);
        p.sequence = seq;
        p.creation = 2.0;
        let c = collection(&table, SimdRandom::new(), vec![p], 2.0 + age);
        sprites.build(&c, &looking_north(), &look).vertices[0].uv
    };
    assert_eq!(
        uv_at(0.25, 0.0),
        [0.0, 0.5],
        "512 samples a second, sample 128 still frame 0"
    );
    assert_eq!(uv_at(1.5, 0.0), [0.5, 0.5], "past the last knot, frame 1");
    assert_eq!(
        uv_at(1.01, 0.0),
        [0.5, 0.5],
        "sample 517 is past the knot at 512"
    );
    assert_eq!(uv_at(1.5, 3.7), [0.0, 1.0], "sequence 3, a single frame");
    assert_eq!(
        uv_at(2.5, 5.0),
        [0.0, 0.5],
        "sample 1280 wraps round to 256, frame 0"
    );
    assert_eq!(
        uv_at(2.5, 0.0),
        [0.5, 0.5],
        "a clamped sequence holds its last sample"
    );
    let fit = renderer(
        "render_animated_sprites",
        vec![("animation_fit_lifetime", portal::dmx::Value::Bool(true))],
    );
    let mut p = at([0.0, 10.0, 0.0], 1.0);
    p.life = 2.0;
    let c = collection(&table, SimdRandom::new(), vec![p], 1.5);
    assert_eq!(
        fit.build(&c, &looking_north(), &look).vertices[0].uv,
        [0.5, 0.5],
        "1024 over a 2 s life"
    );
    let c = collection(&table, SimdRandom::new(), vec![p], 0.4);
    assert_eq!(
        fit.build(&c, &looking_north(), &look).vertices[0].uv,
        [0.0, 0.5]
    );
}

#[test]
fn cards_size_and_fade_with_distance() {
    let table = flat(0.5);
    let sprites = renderer("render_animated_sprites", vec![]);
    let card = |c: Card, radius: f32| {
        let look = Look {
            card: Some(c),
            ..plain()
        };
        let col = collection(
            &table,
            SimdRandom::new(),
            vec![at([0.0, 100.0, 0.0], radius)],
            0.0,
        );
        sprites.build(&col, &looking_north(), &look)
    };
    let m = card(Card::default(), 2.0);
    let corners: Vec<(Vec3, [f32; 2])> = m.vertices.iter().map(|v| (v.pos, v.uv)).collect();
    assert_eq!(
        corners,
        [
            ([2.0, 100.0, -2.0], [1.0, 1.0]),
            ([-2.0, 100.0, -2.0], [0.0, 1.0]),
            ([-2.0, 100.0, 2.0], [0.0, 0.0]),
            ([2.0, 100.0, 2.0], [1.0, 0.0]),
        ],
        "corner (0, 0) takes the right bottom"
    );
    let motes = Card {
        min_size: 0.00075,
        max_size: 0.002,
        ..Card::default()
    };
    assert!(
        near(card(motes, 2.0).vertices[0].pos, [0.2, 100.0, -0.2]),
        "capped at 0.002 of the distance"
    );
    assert!(
        near(card(motes, 0.01).vertices[0].pos, [0.075, 100.0, -0.075]),
        "at least 0.00075 of it"
    );
    let fading = Card {
        start_fade: 0.01,
        end_fade: 0.03,
        ..Card::default()
    };
    let c = card(fading, 2.0).vertices[0].color;
    assert!((c[3] - 0.5).abs() < 1e-6, "half way through the fade {c:?}");
    assert!(
        (c[0] - 0.5f32.powf(1.0 / 2.2)).abs() < 1e-6,
        "colour fades in linear terms {c:?}"
    );
    let gone = card(fading, 4.0);
    assert!(
        gone.vertices
            .iter()
            .all(|v| v.pos == [0.0, 100.0, 0.0] && v.color[3] == 0.0),
        "past the end"
    );
    let far = Card {
        max_distance: 150.0,
        far_interval: 100.0,
        ..Card::default()
    };
    assert!(
        (card(far, 2.0).vertices[0].color[3] - 0.5).abs() < 1e-6,
        "half way through the far fade"
    );
}

#[test]
fn trails_and_ropes_skip_spritecard_materials() {
    let table = flat(0.5);
    let mut p = at([0.0, 0.0, 0.0], 2.0);
    p.prev = [0.0, -1.0, 0.0];
    p.trail = 0.5;
    let c = collection(
        &table,
        SimdRandom::new(),
        vec![p, at([0.0, 10.0, 0.0], 2.0)],
        1.0,
    );
    let look = Look {
        card: Some(Card::default()),
        ..plain()
    };
    assert!(renderer("render_sprite_trail", vec![])
        .build(&c, &looking_north(), &look)
        .vertices
        .is_empty());
    assert!(renderer("render_rope", vec![])
        .build(&c, &looking_north(), &look)
        .vertices
        .is_empty());
}
