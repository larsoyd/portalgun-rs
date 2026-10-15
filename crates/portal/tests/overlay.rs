// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::{Path, PathBuf};

use portal::overlay::Overlay;

fn steam() -> PathBuf {
    portal::install::root(None, None, Path::new(&std::env::var("HOME").unwrap()))
}

fn close(a: &Overlay, b: &Overlay) -> bool {
    let near = |x: f32, y: f32| (x - y).abs() < 1e-3;
    let flat = |o: &Overlay| {
        let mut v = vec![o.u[0], o.u[1], o.v[0], o.v[1], o.fade[0], o.fade[1]];
        v.extend(o.points.iter().flatten());
        v.extend(o.origin);
        v.extend(o.basis.iter().flatten());
        v
    };
    a.order == b.order && flat(a).iter().zip(flat(b)).all(|(&x, y)| near(x, y))
}

#[test]
fn every_accessor_of_the_first_chamber_compiles_to_its_overlay_record() {
    let map = portal::bsp::load(&steam(), "testchmb_a_00").unwrap();
    assert_eq!(map.overlays.len(), 71);
    let accessors: Vec<_> = map
        .entities
        .iter()
        .filter(|e| e.classname == "info_overlay_accessor")
        .collect();
    assert_eq!(accessors.len(), 13);
    for e in accessors {
        let id: usize = e.get("OverlayID").unwrap().parse().unwrap();
        let ours = Overlay::from_keys(e).unwrap();
        let theirs = &map.overlays[id];
        assert!(
            close(&ours, theirs),
            "overlay {id}\nkeys {ours:?}\nlump {theirs:?}"
        );
    }
}

#[test]
fn the_compiled_basis_v_is_normal_cross_u_without_renormalising() {
    let map = portal::bsp::load(&steam(), "testchmb_a_00").unwrap();
    let o = &map.overlays[0];
    assert_eq!(o.basis, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    assert_eq!(o.points[0], [-96.0, -4.0]);
    assert_eq!((o.u, o.v, o.fade), ([0.0, 6.0], [1.0, 0.0], [-1.0, 0.0]));
}

#[test]
fn an_overlay_whose_v_runs_against_normal_cross_u_is_refused() {
    let mut e = portal::vmf::Entity {
        classname: "info_overlay".into(),
        keys: Vec::new(),
        outputs: Vec::new(),
        solids: Vec::new(),
    };
    for (k, v) in [
        ("BasisOrigin", "0 0 0"),
        ("BasisU", "1 0 0"),
        ("BasisV", "0 -1 0"),
        ("BasisNormal", "0 0 1"),
        ("uv0", "-8 -8 0"),
        ("uv1", "-8 8 0"),
        ("uv2", "8 8 0"),
        ("uv3", "8 -8 0"),
        ("StartU", "0"),
        ("EndU", "1"),
        ("StartV", "0"),
        ("EndV", "1"),
        ("sides", "1"),
        ("fademindist", "-1"),
    ] {
        e.keys.push((k.into(), v.into()));
    }
    let refused = Overlay::from_keys(&e).unwrap_err().to_string();
    assert!(refused.contains("BasisV"), "{refused}");
    e.keys.retain(|(k, _)| k != "BasisV");
    e.keys.push(("BasisV".into(), "0 1 0".into()));
    assert_eq!(Overlay::from_keys(&e).unwrap().basis[1], [0.0, 1.0, 0.0]);
}

fn floor(points: [[f32; 2]; 4]) -> Overlay {
    Overlay {
        faces: vec![1],
        order: 0,
        u: [0.0, 2.0],
        v: [1.0, 0.0],
        points,
        origin: [0.0, 0.0, 0.0],
        basis: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        fade: [-1.0, 0.0],
    }
}

const SQUARE: [[f32; 2]; 4] = [[-16.0, -16.0], [-16.0, 16.0], [16.0, 16.0], [16.0, -16.0]];
const UP: ([f32; 3], f32) = ([0.0, 0.0, 1.0], 0.0);

fn big_floor() -> Vec<[f32; 3]> {
    vec![
        [-64.0, -64.0, 0.0],
        [-64.0, 64.0, 0.0],
        [64.0, 64.0, 0.0],
        [64.0, -64.0, 0.0],
    ]
}

#[test]
fn a_floor_overlay_lies_a_tenth_above_the_face_inside_its_quad() {
    let polys = floor(SQUARE).fragments(&big_floor(), UP).unwrap();
    assert_eq!(polys.len(), 2, "one fragment per face triangle");
    for v in polys.iter().flatten() {
        assert!((v.pos[2] - 0.1).abs() < 1e-6, "{v:?}");
        assert!(
            v.pos[0].abs() <= 16.001 && v.pos[1].abs() <= 16.001,
            "{v:?}"
        );
        assert_eq!(v.normal, [0.0, 0.0, 1.0]);
    }
    let corner = |x: f32, y: f32| {
        polys
            .iter()
            .flatten()
            .find(|v| (v.pos[0] - x).abs() < 1e-3 && (v.pos[1] - y).abs() < 1e-3)
            .map(|v| (v.uv, v.st))
    };
    assert_eq!(corner(-16.0, -16.0), Some(([0.0, 1.0], [0.0, 0.0])));
    assert_eq!(corner(-16.0, 16.0), Some(([0.0, 0.0], [0.0, 1.0])));
    assert_eq!(corner(16.0, 16.0), Some(([2.0, 0.0], [1.0, 1.0])));
    assert_eq!(corner(16.0, -16.0), Some(([2.0, 1.0], [1.0, 0.0])));
}

#[test]
fn a_face_smaller_than_the_quad_cuts_the_overlay_to_its_own_outline() {
    let face = vec![
        [0.0, 0.0, 0.0],
        [0.0, 8.0, 0.0],
        [8.0, 8.0, 0.0],
        [8.0, 0.0, 0.0],
    ];
    let polys = floor(SQUARE).fragments(&face, UP).unwrap();
    let area: f32 = polys
        .iter()
        .map(|p| {
            (1..p.len() - 1)
                .map(|i| {
                    let (a, b, c) = (p[0].pos, p[i].pos, p[i + 1].pos);
                    ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() / 2.0
                })
                .sum::<f32>()
        })
        .sum();
    assert!((area - 64.0).abs() < 1e-3, "area {area}");
    let mid = polys
        .iter()
        .flatten()
        .find(|v| (v.pos[0] - 8.0).abs() < 1e-4 && (v.pos[1] - 8.0).abs() < 1e-4)
        .unwrap();
    assert!(
        (mid.uv[0] - 1.5).abs() < 1e-5 && (mid.uv[1] - 0.25).abs() < 1e-5,
        "{mid:?}"
    );
}

#[test]
fn face_triangles_at_four_squared_area_or_less_are_skipped() {
    let tiny = |leg: f32| vec![[0.0, 0.0, 0.0], [0.0, 2.0, 0.0], [leg, 0.0, 0.0]];
    assert!(floor(SQUARE).fragments(&tiny(1.0), UP).unwrap().is_empty());
    assert_eq!(floor(SQUARE).fragments(&tiny(1.0001), UP).unwrap().len(), 1);
}

#[test]
fn on_a_slope_the_overlay_drops_along_its_normal_onto_the_face() {
    let n = [0.0, -0.6, 0.8];
    let face = vec![
        [-64.0, -64.0, -48.0],
        [-64.0, 64.0, 48.0],
        [64.0, 64.0, 48.0],
        [64.0, -64.0, -48.0],
    ];
    let polys = floor(SQUARE).fragments(&face, (n, 0.0)).unwrap();
    let corner = polys.iter().flatten().find(|v| v.uv == [0.0, 1.0]).unwrap();
    let expected = [-16.0, -16.0 - 0.06, -12.0 + 0.08];
    assert!(
        (0..3).all(|i| (corner.pos[i] - expected[i]).abs() < 1e-4),
        "{corner:?}"
    );
    assert_eq!(corner.normal, n);
}

#[test]
fn a_skewed_parallelogram_keeps_its_corner_coordinates() {
    let polys = floor([[-16.0, -8.0], [-8.0, 8.0], [24.0, 8.0], [16.0, -8.0]])
        .fragments(&big_floor(), UP)
        .unwrap();
    let at = polys
        .iter()
        .flatten()
        .find(|v| (v.pos[0] + 8.0).abs() < 1e-3 && (v.pos[1] - 8.0).abs() < 1e-3);
    assert_eq!(at.map(|v| v.uv), Some([0.0, 0.0]));
}

#[test]
fn a_quad_that_is_not_a_parallelogram_is_refused() {
    let trapezoid = floor([[-16.0, -16.0], [-8.0, 16.0], [8.0, 16.0], [16.0, -16.0]]);
    let refused = trapezoid
        .fragments(&big_floor(), UP)
        .unwrap_err()
        .to_string();
    assert!(refused.contains("parallelogram"), "{refused}");
}

#[test]
fn opposite_edges_must_run_within_a_hundredth_of_parallel_and_a_tenth_of_length() {
    let (c, s) = (10f32.to_radians().cos(), 10f32.to_radians().sin());
    let turned = floor([
        [-16.0, -16.0],
        [-16.0, 16.0],
        [-16.0 + 32.0 * c, 16.0 + 32.0 * s],
        [16.0, -16.0],
    ]);
    assert!(
        turned.fragments(&big_floor(), UP).is_err(),
        "10 degrees apart"
    );
    let longer = floor([[-16.0, -16.0], [-16.0, 16.0], [16.15, 16.0], [16.0, -16.0]]);
    assert!(longer.fragments(&big_floor(), UP).is_err(), "0.15 longer");
    let close = floor([[-16.0, -16.0], [-16.0, 16.0], [16.05, 16.0], [16.0, -16.0]]);
    assert!(close.fragments(&big_floor(), UP).is_ok(), "0.05 longer");
}
