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
