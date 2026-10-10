use portal::pair::Portal;
use portal::placement::{HALF_HEIGHT, HALF_WIDTH};
use portal::portal_face::{
    corner, coverage, ellipse, in_hole, near, noise_coords, opening, overlay, overlay_rgb, radius,
    refraction, screen_uv, shader_time, srgb_decode, srgb_encode, Fade,
};

const X: [f32; 3] = [1.0, 0.0, 0.0];
const Z: [f32; 3] = [0.0, 0.0, 1.0];

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

fn close2(a: [f32; 2], b: [f32; 2]) -> bool {
    close(a[0], b[0]) && close(a[1], b[1])
}

fn flat(n: [f32; 2]) -> impl FnMut([f32; 2]) -> [f32; 2] {
    move |_| n
}

#[test]
fn the_quad_is_64_by_108_lifted_a_quarter_unit_with_u_across_and_v_down() {
    let lift = 0.25;
    // u runs from the portal's right (the viewer's left) and v from the top
    for (uv, want) in [
        ([0.0, 1.0], [lift, HALF_WIDTH, -HALF_HEIGHT]),
        ([0.0, 0.0], [lift, HALF_WIDTH, HALF_HEIGHT]),
        ([1.0, 1.0], [lift, -HALF_WIDTH, -HALF_HEIGHT]),
        ([1.0, 0.0], [lift, -HALF_WIDTH, HALF_HEIGHT]),
    ] {
        let got = corner(uv);
        assert!(
            got.iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-4),
            "uv={uv:?} got={got:?} want={want:?}"
        );
    }
    assert!(close(HALF_WIDTH, 32.0) && close(HALF_HEIGHT, 54.0));
}

#[test]
fn the_ellipse_overhangs_the_quad_so_its_edge_falls_inside() {
    assert!(close2(ellipse([0.0, 0.0]), [-0.0375, -0.0375]));
    assert!(close2(ellipse([1.0, 1.0]), [1.0375, 1.0375]));
    assert!(close2(ellipse([0.5, 0.5]), [0.5, 0.5]));
    assert!(close(radius(ellipse([0.5, 0.0])), 1.075));
    // the edge sits at 0.5 / 1.075 of the quad from its middle
    assert!(close(radius(ellipse([0.5 + 0.5 / 1.075, 0.5])), 1.0));
    assert!(close(radius([0.8, 0.9]), 1.0));
}

#[test]
fn opening_is_a_smoothstep_of_the_clamped_amount() {
    assert_eq!(opening(0.5), 0.5);
    assert!(close(opening(0.25), 0.15625));
    assert!(close(opening(0.2), 0.104));
    assert_eq!(opening(-1.0), 0.0);
    assert_eq!(opening(2.0), 1.0);
}

#[test]
fn the_hole_is_the_disc_inside_the_squared_opening() {
    // half open: smoothstep 0.5, so the hole reaches 0.25
    assert!(in_hole(0.5, [0.625, 0.5]));
    assert!(!in_hole(0.5, [0.626, 0.5]));
    assert!(in_hole(0.5, [0.5, 0.375]));
    assert!(in_hole(1.0, [1.0, 0.5]));
    assert!(!in_hole(1.0, [0.5, 1.01]));
    assert!(in_hole(0.0, [0.5, 0.5]));
    assert!(!in_hole(0.0, [0.5, 0.5001]));
}

#[test]
fn shader_time_wraps_every_thousand_seconds() {
    assert_eq!(shader_time(1234.5), 234.5);
    assert_eq!(shader_time(999.0), 999.0);
    assert_eq!(shader_time(1000.0), 0.0);
    assert_eq!(shader_time(2003.0), 3.0);
}

#[test]
fn the_noise_layers_scroll_apart_and_shrink_with_the_opening() {
    let t = noise_coords([0.5, 0.5], 1.0, 10.0);
    let want = [0.425, 0.15, -0.125, 0.15];
    assert!(t.iter().zip(want).all(|(a, b)| close(*a, b)), "{t:?}");
    // 0.499 open is 0.5 after the 0.001 nudge, so the pattern spreads twice as far
    let t = noise_coords([1.0, 0.0], 0.499, 0.0);
    let want = [0.45, -0.15, 0.45, -0.15];
    assert!(t.iter().zip(want).all(|(a, b)| close(*a, b)), "{t:?}");
    // past fully open the spread stays at one
    let t = noise_coords([1.0, 0.0], 3.0, 0.0);
    let want = [0.3, 0.0, 0.3, 0.0];
    assert!(t.iter().zip(want).all(|(a, b)| close(*a, b)), "{t:?}");
}

#[test]
fn the_rim_fades_in_over_point_three_inside_the_opening() {
    let o = overlay(1.0, 1.0, [0.925, 0.5], [0.0; 4], flat([0.4, 0.6]));
    assert!(close(o.alpha, 0.310_994_17), "{o:?}");
    assert!(close(o.u, 0.096_694_87), "{o:?}");
}

#[test]
fn full_static_covers_the_middle() {
    let o = overlay(1.0, 0.0, [0.5, 0.5], [0.0; 4], flat([0.4, 0.6]));
    assert!(close(o.alpha, 1.5), "{o:?}");
    assert!(close(o.u, 0.186_518_32), "{o:?}");
}

#[test]
fn outside_the_opening_the_rim_dies_within_0_075() {
    let o = overlay(1.0, 1.0, [1.025, 0.5], [0.0; 4], flat([0.4, 0.6]));
    assert!(close(o.alpha, 0.139_252_58), "{o:?}");
    assert!(close(o.u, 0.035_483_43), "{o:?}");
    let o = overlay(1.0, 1.0, [1.04, 0.5], [0.0; 4], flat([0.4, 0.6]));
    assert_eq!(o.alpha, 0.0, "{o:?}");
}

#[test]
fn a_barely_open_portal_rims_at_two_and_a_half_times_its_opening() {
    let o = overlay(0.2, 1.0, [0.5, 0.5], [0.0; 4], flat([0.4, 0.6]));
    assert!(close(o.alpha, 0.081_372_4), "{o:?}");
    assert!(close(o.u, 0.018_279_43), "{o:?}");
}

#[test]
fn the_noise_reads_chain_through_the_scrolling_coordinates() {
    let mut seen = Vec::new();
    let reads = [[0.5, 0.25], [0.75, 0.5], [0.0, 1.0]];
    let noise = |at: [f32; 2]| {
        seen.push(at);
        reads[seen.len() - 1]
    };
    let o = overlay(1.0, 1.0, [0.925, 0.5], [0.1, 0.2, 0.3, 0.4], noise);
    assert_eq!(seen.len(), 3);
    assert!(close2(seen[0], [0.1, 0.2]), "{seen:?}");
    assert!(close2(seen[1], [0.39, 0.295]), "{seen:?}");
    assert!(close2(seen[2], [0.085, 0.19]), "{seen:?}");
    // the second and third reads' y average to 0.75
    assert!(close(o.alpha, 0.218_658_9), "{o:?}");
    assert!(close(o.u, 0.057_006_68), "{o:?}");
}

#[test]
fn the_target_clamps_alpha_and_the_test_drops_the_faintest() {
    assert_eq!(coverage(1.5), 1.0);
    assert_eq!(coverage(0.5), 0.5);
    assert_eq!(coverage(0.0039), 0.0);
    assert_eq!(coverage(0.004), 0.004);
    assert_eq!(coverage(0.0), 0.0);
}

#[test]
fn the_ramp_is_scaled_in_linear_light_and_written_back_as_stored() {
    assert!(close(srgb_decode(64.0 / 255.0), 0.051_269_46));
    assert!(close(srgb_decode(0.02), 0.02 / 12.92));
    assert!(close(srgb_encode(0.5), 0.735_357));
    assert!(close(srgb_encode(0.002), 0.002 * 12.92));
    // just past the linear toe on both sides
    assert!(close(srgb_encode(0.01), 0.099_852_82));
    assert!(close(srgb_decode(0.05), 0.003_935_94));
    // the blue ramp's middle texel (00 40 8f) at scale 4
    let rgb = overlay_rgb(
        [0.0, srgb_decode(64.0 / 255.0), srgb_decode(143.0 / 255.0)],
        4.0,
    );
    assert!(close(rgb[0], 0.0) && close(rgb[1], 0.490_195_08), "{rgb:?}");
    assert!(close(rgb[2], 1.0), "{rgb:?}");
}

#[test]
fn screen_coordinates_run_right_and_down_from_the_top_left() {
    assert!(close2(screen_uv([0.5, -0.5, 0.3, 2.0]), [0.625, 0.625]));
    assert!(close2(screen_uv([0.0, 0.0, 0.7, 1.0]), [0.5, 0.5]));
    assert!(close2(screen_uv([1.0, 1.0, 0.0, 1.0]), [1.0, 0.0]));
}

#[test]
fn refraction_pulls_toward_the_middle_along_the_surface() {
    let (uv, along_t, along_b) = ([0.4, 0.6], [0.01, 0.002], [0.0, -0.02]);
    let r = refraction(0.5, [0.75, 0.5], uv, along_t, along_b).unwrap();
    assert!(close2(r.at, [0.32, 0.584]), "{r:?}");
    assert!(close(r.shade, 0.897_368_4), "{r:?}");
    // below the middle: v grows downward and the binormal points up
    let r = refraction(0.5, [0.5, 0.8], uv, along_t, along_b).unwrap();
    assert!(close2(r.at, [0.4, 0.496_32]), "{r:?}");
    assert!(close(r.shade, 0.857_894_7), "{r:?}");
}

#[test]
fn refraction_falls_off_toward_the_edge_and_stops_there() {
    let (uv, along_t, along_b) = ([0.4, 0.6], [0.01, 0.0], [0.0, -0.02]);
    let r = refraction(1.0, [0.995, 0.5], uv, along_t, along_b).unwrap();
    assert!(close(r.at[0], 0.4 - 0.474_403_5 * 32.0 * 0.01), "{r:?}");
    assert!(refraction(1.0, [1.0, 0.5], uv, along_t, along_b).is_some());
    assert!(refraction(0.5, [1.01, 0.5], uv, along_t, along_b).is_none());
}

#[test]
fn opening_takes_half_a_second() {
    let mut f = Fade::default();
    for _ in 0..4 {
        f.think(0.1);
    }
    assert!(close(f.open, 0.8), "{f:?}");
    f.think(0.2);
    assert_eq!(f.open, 1.0);
}

#[test]
fn static_falls_one_a_second_and_stops_at_nothing() {
    let mut f = Fade {
        open: 1.0,
        statics: 1.0,
    };
    f.think(0.25);
    assert!(close(f.statics, 0.75), "{f:?}");
    f.think(1.0);
    assert_eq!(f.statics, 0.0);
    assert_eq!(f.open, 1.0);
}

#[test]
fn moving_a_portal_reopens_it_and_fills_its_partner_with_static() {
    let mut moved = Fade {
        open: 1.0,
        statics: 0.25,
    };
    let mut partner = Fade {
        open: 1.0,
        statics: 0.0,
    };
    moved.moved(Some(&mut partner));
    assert_eq!(
        moved,
        Fade {
            open: 0.0,
            statics: 0.25
        }
    );
    assert_eq!(
        partner,
        Fade {
            open: 1.0,
            statics: 1.0
        }
    );
    let mut alone = Fade {
        open: 1.0,
        statics: 0.5,
    };
    alone.moved(None);
    assert_eq!(
        alone,
        Fade {
            open: 0.0,
            statics: 0.5
        }
    );
}

#[test]
fn a_camera_within_54_units_in_front_clears_the_static() {
    let p = Portal::new([100.0, 0.0, 0.0], X, Z, (HALF_WIDTH, HALF_HEIGHT));
    let at = |f: f32, r: f32| p.world([f, r, 0.0]);
    assert!(near(&p, at(50.0, 0.0)));
    assert!(!near(&p, at(55.0, 0.0)));
    assert!(near(&p, at(10.0, 53.0)));
    assert!(!near(&p, at(10.0, 54.0)));
    // up to a unit behind the plane still counts
    assert!(near(&p, at(-0.5, 0.0)));
    assert!(!near(&p, at(-2.0, 0.0)));
}

#[test]
fn the_kept_amount_is_clamped_like_the_constant_register() {
    let at = |keep: f32| overlay(1.0, keep, [0.925, 0.5], [0.0; 4], flat([0.4, 0.6]));
    assert_eq!(at(1.5), at(1.0));
    assert_eq!(at(-0.5), at(0.0));
    assert_ne!(at(1.0), at(0.0));
}
