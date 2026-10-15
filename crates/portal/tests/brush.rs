// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::brush::{bounds, faces};
use portal::vmf::parse;

fn side(points: &str, material: &str) -> String {
    format!(
        "side {{ \"id\" \"1\" \"plane\" \"{points}\" \"material\" \"{material}\" \
         \"uaxis\" \"[1 0 0 8] 0.25\" \"vaxis\" \"[0 -1 0 0] 0.5\" }}"
    )
}

fn solid(sides: &[(&str, &str)]) -> Vec<portal::vmf::Side> {
    let body: String = sides.iter().map(|(p, m)| side(p, m)).collect();
    let text = format!("world {{ solid {{ \"id\" \"1\" {body} }} }}");
    parse(&text).unwrap().world.remove(0).sides
}

/// 128 by 64 by 32 box from the origin, Hammer's own six planes
fn cube() -> Vec<portal::vmf::Side> {
    solid(&[
        ("(0 64 32) (128 64 32) (128 0 32)", "top"),
        ("(0 0 0) (128 0 0) (128 64 0)", "bottom"),
        ("(0 64 32) (0 0 32) (0 0 0)", "west"),
        ("(128 64 0) (128 0 0) (128 0 32)", "east"),
        ("(128 64 32) (0 64 32) (0 64 0)", "north"),
        ("(128 0 0) (0 0 0) (0 0 32)", "south"),
    ])
}

/// ramp rising 64 over 64 along x, the slope faces up and back
fn ramp() -> Vec<portal::vmf::Side> {
    solid(&[
        ("(0 0 0) (64 64 64) (64 0 64)", "slope"),
        ("(0 0 0) (64 0 0) (64 64 0)", "bottom"),
        ("(64 64 0) (64 0 0) (64 0 64)", "east"),
        ("(64 64 64) (0 64 0) (64 64 0)", "north"),
        ("(64 0 0) (0 0 0) (64 0 64)", "south"),
    ])
}

fn area(points: &[[f32; 3]]) -> f32 {
    let mut n = [0.0f32; 3];
    for i in 1..points.len() - 1 {
        let (a, b) = (sub(points[i], points[0]), sub(points[i + 1], points[0]));
        let c = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        n = [n[0] + c[0], n[1] + c[1], n[2] + c[2]];
    }
    (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() / 2.0
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn winds_outward(points: &[[f32; 3]], normal: [f32; 3]) -> bool {
    let (a, b) = (sub(points[1], points[0]), sub(points[2], points[0]));
    let c = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    c[0] * normal[0] + c[1] * normal[1] + c[2] * normal[2] > 0.0
}

#[test]
fn a_box_gives_six_quads_wound_outward() {
    let sides = cube();
    let polys = faces(&sides);
    assert_eq!(polys.len(), 6);
    let expect = [
        128.0 * 64.0,
        128.0 * 64.0,
        64.0 * 32.0,
        64.0 * 32.0,
        128.0 * 32.0,
        128.0 * 32.0,
    ];
    for (p, want) in polys.iter().zip(expect) {
        assert_eq!(p.points.len(), 4, "side {}", sides[p.side].material);
        assert!(
            (area(&p.points) - want).abs() < 0.01,
            "side {} area {}",
            sides[p.side].material,
            area(&p.points)
        );
        assert!(
            winds_outward(&p.points, sides[p.side].plane.normal),
            "side {}",
            sides[p.side].material
        );
        for q in &p.points {
            assert!(
                q.iter().all(|c| (c - c.round()).abs() < 1e-3),
                "{q:?} off the grid"
            );
        }
    }
    assert_eq!(bounds(&polys), ([0.0, 0.0, 0.0], [128.0, 64.0, 32.0]));
}

#[test]
fn a_ramp_has_a_slanted_top_and_triangle_ends() {
    let sides = ramp();
    let polys = faces(&sides);
    assert_eq!(polys.len(), 5);
    let slope = &sides[polys[0].side];
    let h = std::f32::consts::FRAC_1_SQRT_2;
    assert!(
        (slope.plane.normal[0] + h).abs() < 1e-6 && (slope.plane.normal[2] - h).abs() < 1e-6,
        "{:?}",
        slope.plane.normal
    );
    assert!((area(&polys[0].points) - 64.0 * 64.0 * 2f32.sqrt()).abs() < 0.01);
    let ends: Vec<usize> = polys
        .iter()
        .filter(|p| p.points.len() == 3)
        .map(|p| p.side)
        .collect();
    assert_eq!(ends, [3, 4]);
    assert!(polys
        .iter()
        .all(|p| winds_outward(&p.points, sides[p.side].plane.normal)));
}

#[test]
fn a_side_that_never_touches_the_solid_gives_no_face() {
    let mut sides = cube();
    let extra = solid(&[("(0 64 64) (128 64 64) (128 0 64)", "above")]);
    sides.extend(extra);
    let polys = faces(&sides);
    assert_eq!(polys.len(), 6);
    assert!(polys.iter().all(|p| sides[p.side].material != "above"));
}

#[test]
fn texture_coordinates_follow_the_side_axes_in_texels() {
    let sides = cube();
    let top = &faces(&sides)[0];
    for (p, uv) in top.points.iter().zip(&top.uv) {
        assert_eq!(uv[0], p[0] / 0.25 + 8.0, "{p:?}");
        assert_eq!(uv[1], -p[1] / 0.5, "{p:?}");
    }
}
