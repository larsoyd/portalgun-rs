use portal::pair::Portal;
use portal::placement::{HALF_HEIGHT, HALF_WIDTH};
use portal::view::{
    clip_plane, clip_poly, doubler_usable, draws_doubler, max_depth, near_plane, remaining,
    shows_view, static_amount, Plane, STENCIL_DEPTH,
};

const X: [f32; 3] = [1.0, 0.0, 0.0];
const Z: [f32; 3] = [0.0, 0.0, 1.0];
const HALF: (f32, f32) = (HALF_WIDTH, HALF_HEIGHT);

fn wall_at(x: f32, forward: [f32; 3]) -> Portal {
    Portal::new([x, 0.0, 0.0], forward, Z, HALF)
}

fn plane(normal: [f32; 3], dist: f32) -> Plane {
    Plane { normal, dist }
}

/// A box of planes around the origin that keeps everything within reach on each axis.
fn boxed(reach: f32) -> Vec<Plane> {
    (0..3)
        .flat_map(|i| {
            let mut n = [0.0; 3];
            n[i] = 1.0;
            let m = n.map(|v| -v);
            [plane(n, -reach), plane(m, -reach)]
        })
        .collect()
}

#[test]
fn the_exit_clip_plane_sits_half_a_unit_behind_the_exit() {
    let exit = wall_at(100.0, X);
    let p = clip_plane(&exit);
    assert_eq!(p.normal, X);
    assert_eq!(p.dist, 99.5);
    let tilted = Portal::new([0.0, 0.0, 10.0], Z, X, HALF);
    assert_eq!(clip_plane(&tilted).dist, 9.5);
}

#[test]
fn the_portal_view_raises_the_near_plane_to_one_unit() {
    assert_eq!(near_plane(0.5), 1.0);
    assert_eq!(near_plane(0.999), 1.0);
    assert_eq!(near_plane(1.0), 1.0);
    assert_eq!(near_plane(1.001), 1.001);
    assert_eq!(near_plane(7.0), 7.0);
}

#[test]
fn views_nest_two_deep_by_default_capped_by_the_stencil_bits() {
    assert_eq!(STENCIL_DEPTH, 2);
    assert_eq!(max_depth(2, 8), 2);
    assert_eq!(max_depth(20, 8), 10);
    assert_eq!(max_depth(10, 8), 10);
    assert_eq!(max_depth(11, 8), 10);
    assert_eq!(max_depth(9, 3), 7);
    assert_eq!(max_depth(12, 4), 10, "sixteen values is past eleven");
    assert_eq!(max_depth(5, 2), 3);
    assert_eq!(max_depth(1, 8), 1);
}

#[test]
fn faces_drawn_at_a_level_have_the_depth_left_below_them() {
    assert_eq!(remaining(2, 0), 2);
    assert_eq!(remaining(2, 1), 1);
    assert_eq!(remaining(2, 2), 0);
    assert_eq!(remaining(2, 3), 0);
    assert_eq!(remaining(3, 1), 2);
}

#[test]
fn static_is_full_at_the_last_level_and_gone_with_the_doubler() {
    assert_eq!(static_amount(0.25, true, 2, false), 0.25);
    assert_eq!(static_amount(0.25, true, 1, false), 0.25);
    assert_eq!(static_amount(0.25, true, 0, false), 1.0);
    assert_eq!(static_amount(0.25, false, 2, false), 1.0);
    assert_eq!(static_amount(0.25, false, 1, false), 1.0);
    assert_eq!(static_amount(0.25, true, 0, true), 0.0);
    assert_eq!(static_amount(0.25, false, 0, true), 0.0);
}

#[test]
fn the_doubler_needs_facing_portals_with_the_partner_in_front() {
    let a = wall_at(0.0, X);
    let facing = wall_at(200.0, [-1.0, 0.0, 0.0]);
    assert!(doubler_usable(&a, &facing));
    let behind = wall_at(-200.0, [-1.0, 0.0, 0.0]);
    assert!(!doubler_usable(&a, &behind));
    let level = wall_at(0.0, [-1.0, 0.0, 0.0]);
    assert!(!doubler_usable(&a, &level), "on the plane is not in front");
    let facing_by =
        |d: f32| Portal::new([200.0, 0.0, 0.0], [d, (1.0 - d * d).sqrt(), 0.0], Z, HALF);
    assert!(doubler_usable(&a, &facing_by(-0.70715)));
    assert!(!doubler_usable(&a, &facing_by(-0.70705)));
    let same_way = wall_at(200.0, X);
    assert!(!doubler_usable(&a, &same_way));
}

#[test]
fn the_doubler_draws_only_at_the_bottom_from_level_two() {
    assert!(draws_doubler(true, 0, 2));
    assert!(draws_doubler(true, 0, 3));
    assert!(!draws_doubler(true, 0, 1));
    assert!(!draws_doubler(true, 1, 2));
    assert!(!draws_doubler(false, 0, 2));
}

#[test]
fn clipping_keeps_the_front_and_cuts_crossing_edges() {
    let square = [
        [0.0, 0.0, 0.0],
        [10.0, 0.0, 0.0],
        [10.0, 10.0, 0.0],
        [0.0, 10.0, 0.0],
    ];
    let all = clip_poly(&square, &plane(X, -1.0), 0.01);
    assert_eq!(all.len(), 4);
    let none = clip_poly(&square, &plane(X, 11.0), 0.01);
    assert!(none.is_empty());
    let half = clip_poly(&square, &plane(X, 5.0), 0.01);
    assert_eq!(half.len(), 4, "{half:?}");
    assert!(half.iter().all(|p| p[0] >= 5.0 - 1e-4), "{half:?}");
    assert!(half.contains(&[5.0, 0.0, 0.0]), "{half:?}");
    assert!(half.contains(&[5.0, 10.0, 0.0]), "{half:?}");
}

#[test]
fn points_within_the_epsilon_count_as_on_the_plane() {
    let tri = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]];
    let touching = clip_poly(&tri, &plane(X, 10.005), 0.01);
    assert!(
        touching.is_empty(),
        "points only on the plane leave nothing"
    );
    let just_in = clip_poly(&tri, &plane(X, 9.985), 0.01);
    assert_eq!(just_in.len(), 4, "{just_in:?}");
    let on_kept = clip_poly(&tri, &plane(X, -0.005), 0.01);
    assert_eq!(on_kept.len(), 3, "{on_kept:?}");
}

#[test]
fn an_axis_plane_puts_cut_points_exactly_on_it() {
    let edge = [[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [3.0, 1.0, 0.0]];
    let cut = clip_poly(&edge, &plane(X, 0.1), 0.01);
    assert!(cut.iter().any(|p| p[0] == 0.1), "{cut:?}");
    let back = clip_poly(&edge, &plane([-1.0, 0.0, 0.0], -0.1), 0.01);
    assert!(back.iter().any(|p| p[0] == 0.1), "{back:?}");
}

#[test]
fn a_portal_view_needs_a_link_and_less_than_full_static() {
    let p = wall_at(0.0, X);
    let eye = [200.0, 0.0, 0.0];
    let wide = boxed(10000.0);
    assert!(shows_view(&p, 0.0, true, eye, 0, &wide));
    assert!(shows_view(&p, 0.999, true, eye, 0, &wide));
    assert!(!shows_view(&p, 1.0, true, eye, 0, &wide));
    assert!(!shows_view(&p, 0.0, false, eye, 0, &wide));
}

#[test]
fn a_portal_view_needs_the_eye_strictly_in_front() {
    let p = wall_at(0.0, X);
    let wide = boxed(10000.0);
    assert!(shows_view(&p, 0.0, true, [100.0, 0.0, 0.0], 1, &wide));
    assert!(!shows_view(&p, 0.0, true, [0.0, 0.0, 0.0], 1, &wide));
    assert!(!shows_view(&p, 0.0, true, [-100.0, 0.0, 0.0], 1, &wide));
}

#[test]
fn close_up_at_the_top_level_the_view_shows_from_any_side() {
    let p = wall_at(0.0, X);
    let none: Vec<Plane> = boxed(1.0)
        .into_iter()
        .map(|q| plane(q.normal, 5000.0))
        .collect();
    assert!(shows_view(&p, 0.0, true, [-53.9, 0.0, 0.0], 0, &none));
    assert!(!shows_view(&p, 0.0, true, [-54.0, 0.0, 0.0], 0, &none));
    assert!(!shows_view(&p, 0.0, true, [-53.9, 0.0, 0.0], 1, &none));
}

#[test]
fn a_portal_off_screen_shows_no_view() {
    let p = wall_at(0.0, X);
    let eye = [300.0, 0.0, 0.0];
    let off = [plane([0.0, 1.0, 0.0], 100.0)];
    assert!(!shows_view(&p, 0.0, true, eye, 0, &off));
    let edge = [plane([0.0, 1.0, 0.0], 31.0)];
    assert!(shows_view(&p, 0.0, true, eye, 0, &edge));
    let sliver = [plane([0.0, 1.0, 0.0], 31.9)];
    assert!(shows_view(&p, 0.0, true, eye, 0, &sliver));
    let behind = [plane(X, 2.0)];
    assert!(
        !shows_view(&p, 0.0, true, eye, 0, &behind),
        "the quad sits one unit out"
    );
    let front = [plane(X, 0.5)];
    assert!(shows_view(&p, 0.0, true, eye, 0, &front));
}

#[test]
fn the_epsilon_bounds_belong_to_on() {
    let on_or_back = [[10.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 5.0, 0.0]];
    assert!(clip_poly(&on_or_back, &plane(X, 9.5), 0.5).is_empty());
    let on_and_front = [[9.0, 0.0, 0.0], [20.0, 0.0, 0.0], [20.0, 5.0, 0.0]];
    assert_eq!(clip_poly(&on_and_front, &plane(X, 9.5), 0.5).len(), 3);
    let back_and_front = [[8.9, 0.0, 0.0], [20.0, 0.0, 0.0], [20.0, 5.0, 0.0]];
    assert_eq!(clip_poly(&back_and_front, &plane(X, 9.5), 0.5).len(), 4);
    let all_three = [[9.0, 0.0, 0.0], [20.0, 0.0, 0.0], [0.0, 5.0, 0.0]];
    let kept = clip_poly(&all_three, &plane(X, 9.5), 0.5);
    assert_eq!(kept.len(), 3, "the on vertex stays {kept:?}");
    assert!(kept.contains(&[9.0, 0.0, 0.0]), "{kept:?}");
}

#[test]
fn a_corner_left_on_screen_still_shows_the_view() {
    let p = wall_at(0.0, X);
    let s = std::f32::consts::FRAC_1_SQRT_2;
    let corner = [plane([0.0, s, s], (31.0 + 53.0) * s)];
    assert!(shows_view(&p, 0.0, true, [300.0, 0.0, 0.0], 1, &corner));
    let past = [plane([0.0, s, s], (32.0 + 54.0) * s + 0.1)];
    assert!(!shows_view(&p, 0.0, true, [300.0, 0.0, 0.0], 1, &past));
}
