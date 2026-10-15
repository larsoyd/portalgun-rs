// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::entity::dissolve::colour;

#[test]
fn a_dissolve_tints_from_white_to_black_over_its_first_second() {
    assert_eq!(colour(-0.01), [255, 255, 255, 255]);
    assert_eq!(colour(0.25), [191, 191, 191, 255]);
    assert_eq!(colour(0.5), [127, 127, 127, 255]);
    assert_eq!(colour(0.99), [2, 2, 2, 255]);
    assert_eq!(colour(1.0), [0, 0, 0, 255]);
    assert_eq!(colour(1.5), [0, 0, 0, 255]);
}

#[test]
fn the_tint_is_black_at_the_exact_start_time() {
    assert_eq!(colour(0.0), [0, 0, 0, 255]);
}

#[test]
fn the_model_fades_out_in_its_last_tenth_of_a_second() {
    assert_eq!(colour(1.9)[3], 255);
    assert_eq!(colour(1.925)[3], 191);
    assert_eq!(colour(1.975)[3], 63);
    assert_eq!(colour(1.99)[3], 25);
    assert_eq!(colour(2.01), [0, 0, 0, 0]);
    assert_eq!(colour(2.5), [0, 0, 0, 0]);
}

use portal::entity::dissolve::{emit, near_fade, site, step, Draw, Spark, Sprite};

const IDENTITY: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn near(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-3)
}

#[test]
fn a_site_spans_the_two_box_axes_least_facing_the_eye() {
    let at = |eye| site([0.0; 3], IDENTITY, [40.0; 3], eye);
    let [a, b] = at([100.0, 0.0, 0.0]);
    assert!(
        near(a, [0.0, 40.0, 0.0]) && near(b, [0.0, 0.0, 40.0]),
        "{a:?} {b:?}"
    );
    let [a, b] = at([0.0, 0.0, 100.0]);
    assert!(
        near(a, [40.0, 0.0, 0.0]) && near(b, [0.0, 40.0, 0.0]),
        "{a:?} {b:?}"
    );
    let [a, b] = at([0.0, 100.0, 0.0]);
    assert!(
        near(a, [0.0, 0.0, 40.0]) && near(b, [40.0, 0.0, 0.0]),
        "{a:?} {b:?}"
    );
    let [a, b] = at([100.0, 100.0, 0.0]);
    assert!(
        near(a, [0.0, 0.0, 40.0]) && near(b, [40.0, -40.0, 0.0]),
        "{a:?} {b:?}"
    );
}

#[test]
fn a_site_is_at_least_five_units_across() {
    let [a, b] = site([0.0; 3], IDENTITY, [2.0; 3], [100.0, 0.0, 0.0]);
    assert!(
        near(a, [0.0, 5.0, 0.0]) && near(b, [0.0, 0.0, 5.0]),
        "{a:?} {b:?}"
    );
}

/// every float draw lands at a fixed point of its range, every int draw at its low end
struct Fixed(f32);

impl Draw for Fixed {
    fn float(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.0
    }
    fn int(&mut self, lo: i32, _hi: i32) -> i32 {
        lo
    }
}

fn sites() -> Vec<([f32; 3], [[f32; 3]; 2])> {
    vec![([0.0, 0.0, 100.0], [[0.0, 40.0, 0.0], [0.0, 0.0, 40.0]])]
}

fn sprites(s: &[Spark], kind: Sprite) -> Vec<Spark> {
    s.iter().filter(|p| p.sprite == kind).copied().collect()
}

#[test]
fn half_way_in_two_sparks_and_one_puff_rise_from_behind_the_box() {
    let made = emit(0.5, &sites(), [1.0, 0.0, 0.0], &mut Fixed(0.5));
    let sparks = sprites(&made, Sprite::Spark);
    assert_eq!(sparks.len(), 2);
    let s = sparks[0];
    assert!(near(s.pos, [-8.0, 0.0, 100.0]), "{s:?}");
    assert!(near(s.vel, [0.0, 0.0, 40.0]), "{s:?}");
    assert_eq!(
        (s.start_size, s.die, s.roll_rate, s.life),
        (5, 0.45, 0.0, 0.0)
    );
    let puffs = sprites(&made, Sprite::Muzzle);
    assert_eq!(puffs.len(), 1);
    let p = puffs[0];
    assert!(near(p.vel, [0.0, 0.0, 32.0]), "{p:?}");
    assert_eq!((p.start_size, p.die), (10, 0.1));
}

#[test]
fn the_spread_reaches_three_eighths_of_each_axis() {
    let made = emit(1.5, &sites(), [1.0, 0.0, 0.0], &mut Fixed(1.0));
    let s = sprites(&made, Sprite::Spark)[0];
    assert!(near(s.pos, [8.0, 15.0, 115.0]), "{s:?}");
    assert!(near(s.vel, [4.0, 4.0, 64.0]), "{s:?}");
    assert_eq!((s.start_size, s.die, s.roll_rate), (6, 0.5, 8.0));
    assert_eq!(sprites(&made, Sprite::Muzzle).len(), 3);
}

#[test]
fn the_fade_pulls_the_sparks_forward_over_the_first_second() {
    let x = |dt: f32| emit(dt, &sites(), [1.0, 0.0, 0.0], &mut Fixed(0.5))[0].pos[0];
    assert!((x(0.25) + 16.0).abs() < 1e-4, "{}", x(0.25));
    assert!((x(0.995) - 7.84).abs() < 1e-3, "{}", x(0.995));
    assert_eq!(x(1.0), 8.0);
    let puffs = |dt: f32| {
        sprites(
            &emit(dt, &sites(), [1.0, 0.0, 0.0], &mut Fixed(0.5)),
            Sprite::Muzzle,
        )
        .len()
    };
    assert_eq!(
        [
            puffs(0.25),
            puffs(0.5),
            puffs(0.995),
            puffs(1.0),
            puffs(1.9)
        ],
        [0, 1, 2, 3, 3]
    );
}

#[test]
fn after_two_seconds_only_long_lived_small_sparks_come() {
    let made = emit(2.1, &sites(), [1.0, 0.0, 0.0], &mut Fixed(0.5));
    assert!(sprites(&made, Sprite::Muzzle).is_empty());
    let s = sprites(&made, Sprite::Spark)[0];
    assert!(near(s.pos, [-24.0, 0.0, 100.0]), "{s:?}");
    assert_eq!((s.start_size, s.die, s.roll_rate), (2, 0.9, 0.0));
}

#[test]
fn a_spark_is_only_made_on_a_one_in_three_draw() {
    struct Never;
    impl Draw for Never {
        fn float(&mut self, lo: f32, _hi: f32) -> f32 {
            lo
        }
        fn int(&mut self, _lo: i32, hi: i32) -> i32 {
            hi
        }
    }
    let made = emit(1.5, &sites(), [1.0, 0.0, 0.0], &mut Never);
    assert!(sprites(&made, Sprite::Spark).is_empty());
    assert_eq!(sprites(&made, Sprite::Muzzle).len(), 3);
}

#[test]
fn sparks_coast_shrink_and_fade_then_go_at_their_die_time() {
    let mut s = vec![emit(0.5, &sites(), [1.0, 0.0, 0.0], &mut Fixed(0.5))[0]];
    s[0].roll_rate = 2.0;
    step(&mut s, 0.225);
    assert!(near(s[0].pos, [-8.0, 0.0, 109.0]), "{:?}", s[0]);
    assert!((s[0].roll - 0.45).abs() < 1e-5);
    assert!(
        (s[0].size() - 2.5).abs() < 1e-5 && (s[0].alpha() - 0.5).abs() < 1e-5,
        "{:?}",
        s[0]
    );
    step(&mut s, 0.224);
    assert_eq!(s.len(), 1);
    step(&mut s, 0.001);
    assert!(s.is_empty());
}

#[test]
fn sparks_fade_out_within_sixty_four_units_of_the_eye() {
    assert_eq!(near_fade(1.0, 100.0), 1.0);
    assert_eq!(near_fade(1.0, 64.0), 1.0);
    assert_eq!(near_fade(1.0, 40.0), 0.5);
    assert_eq!(near_fade(1.0, 16.0), 0.0);
    assert_eq!(near_fade(0.0009, 100.0), 0.0);
}

#[test]
fn the_box_has_one_hitbox_on_its_root_bone() {
    let dir = portal::install::root(
        None,
        None,
        std::path::Path::new(&std::env::var("HOME").unwrap()),
    );
    let files = portal::install::Install::open(&dir).unwrap().search;
    let mdl = portal::mdl::Mdl::parse(files.read("models/props/metal_box.mdl").unwrap()).unwrap();
    assert_eq!(mdl.hitboxes.len(), 1);
    let h = &mdl.hitboxes[0];
    assert_eq!(h.bone, 0);
    assert_eq!(h.min, [-19.795658, -19.796953, -19.795374]);
    assert_eq!(h.max, [19.795681, 19.79499, 19.795372]);
}

#[test]
fn when_two_axes_face_the_eye_equally_the_middle_one_is_dropped() {
    let [a, b] = site([0.0; 3], IDENTITY, [40.0; 3], [0.0, 100.0, 100.0]);
    assert!(
        near(a, [0.0, -40.0, 40.0]) && near(b, [40.0, 0.0, 0.0]),
        "{a:?} {b:?}"
    );
}

#[test]
fn a_faint_spark_far_away_is_still_drawn() {
    assert_eq!(near_fade(0.0015, 100.0), 0.0015);
}

#[test]
fn a_middle_draw_makes_no_spark_either() {
    struct One;
    impl Draw for One {
        fn float(&mut self, lo: f32, _hi: f32) -> f32 {
            lo
        }
        fn int(&mut self, lo: i32, _hi: i32) -> i32 {
            lo + 1
        }
    }
    let made = emit(1.5, &sites(), [1.0, 0.0, 0.0], &mut One);
    assert!(sprites(&made, Sprite::Spark).is_empty());
}
