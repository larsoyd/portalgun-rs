// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::Path;

use portal::install::{root, Install};
use portalgun::chamber::Chamber;
use portalgun::walker::{Input, Walker};

const FLOOR: &str = "concrete/concrete_modular_floor001a";

fn side(points: &str) -> String {
    format!(
        "side {{ \"plane\" \"{points}\" \"material\" \"{FLOOR}\" \
         \"uaxis\" \"[1 0 0 0] 0.25\" \"vaxis\" \"[0 -1 0 0] 0.25\" }}"
    )
}

fn solid(sides: &[String]) -> String {
    format!("solid {{ \"id\" \"1\" {} }}", sides.concat())
}

fn floor() -> String {
    let (x0, x1, y0, y1, z0, z1) = (-1024, 1024, -512, 512, -16, 0);
    solid(&[
        side(&format!(
            "({x0} {y1} {z1}) ({x1} {y1} {z1}) ({x1} {y0} {z1})"
        )),
        side(&format!(
            "({x0} {y0} {z0}) ({x1} {y0} {z0}) ({x1} {y1} {z0})"
        )),
        side(&format!(
            "({x0} {y1} {z1}) ({x0} {y0} {z1}) ({x0} {y0} {z0})"
        )),
        side(&format!(
            "({x1} {y1} {z0}) ({x1} {y0} {z0}) ({x1} {y0} {z1})"
        )),
        side(&format!(
            "({x1} {y1} {z1}) ({x0} {y1} {z1}) ({x0} {y1} {z0})"
        )),
        side(&format!(
            "({x1} {y0} {z0}) ({x0} {y0} {z0}) ({x0} {y0} {z1})"
        )),
    ])
}

/// a wedge rising from x 0 to x run, its slope facing back along -x
fn ramp(run: i32, rise: i32) -> String {
    let (x0, x1, y0, y1, h) = (0, run, -128, 128, rise);
    solid(&[
        side(&format!("({x0} {y0} 0) ({x1} {y1} {h}) ({x1} {y0} {h})")),
        side(&format!("({x0} {y0} 0) ({x1} {y0} 0) ({x1} {y1} 0)")),
        side(&format!("({x1} {y1} 0) ({x1} {y0} 0) ({x1} {y0} {h})")),
        side(&format!("({x1} {y1} {h}) ({x0} {y1} 0) ({x1} {y1} 0)")),
        side(&format!("({x1} {y0} 0) ({x0} {y0} 0) ({x1} {y0} {h})")),
    ])
}

fn world(run: i32, rise: i32) -> Chamber {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    let install = Install::open(&dir).unwrap_or_else(|e| panic!("install at {dir:?}: {e}"));
    let text = format!("world {{ {} {} }}", floor(), ramp(run, rise));
    Chamber::from_vmf(&install.search, &text).unwrap()
}

fn walk(c: &Chamber, from: [f32; 3], seconds: f32) -> Walker {
    let mut w = Walker::at(from);
    let input = Input {
        forward: 1.0,
        ..Input::default()
    };
    for _ in 0..(seconds / 0.015) as usize {
        w.step(&input, 0.015, c, None);
    }
    w
}

/// height of the slope under x for a ramp of this run and rise
fn surface(x: f32, run: f32, rise: f32) -> f32 {
    x.clamp(0.0, run) * rise / run
}

#[test]
fn a_gentle_ramp_is_walked_up_on_the_ground() {
    let c = world(256, 128);
    let w = walk(&c, [-64.0, 0.0, 0.0], 1.6);
    assert!(w.grounded, "{:?}", w.feet);
    assert!(w.feet[0] > 60.0 && w.feet[0] < 240.0, "{:?}", w.feet);
    let below = surface(w.feet[0] + 16.0, 256.0, 128.0);
    assert!(
        (w.feet[2] - below).abs() < 1.0,
        "feet {:?} slope {below}",
        w.feet
    );
}

#[test]
fn forty_five_degrees_is_still_ground() {
    let c = world(128, 128);
    let w = walk(&c, [-64.0, 0.0, 0.0], 1.2);
    assert!(w.grounded, "{:?}", w.feet);
    assert!(w.feet[2] > 40.0, "{:?}", w.feet);
}

#[test]
fn a_wedge_steeper_than_the_ground_limit_cannot_be_climbed() {
    let c = world(64, 128);
    let w = walk(&c, [-64.0, 0.0, 0.0], 3.0);
    assert!(w.feet[2] < 19.0, "{:?}", w.feet);
    assert!(w.feet[0] < 24.0, "{:?}", w.feet);
}

#[test]
fn standing_still_on_a_ramp_does_not_slide() {
    let c = world(256, 128);
    let mut w = Walker::at([128.0, 0.0, 100.0]);
    for _ in 0..200 {
        w.step(&Input::default(), 0.015, &c, None);
    }
    let settled = w.feet;
    for _ in 0..200 {
        w.step(&Input::default(), 0.015, &c, None);
    }
    assert!(w.grounded, "{:?}", w.feet);
    let drift = ((w.feet[0] - settled[0]).powi(2) + (w.feet[2] - settled[2]).powi(2)).sqrt();
    assert!(
        drift < 0.01,
        "slid {drift} from {settled:?} to {:?}",
        w.feet
    );
}

#[test]
fn a_hull_over_the_top_edge_rests_on_the_edge_not_on_the_slope_extended() {
    let c = world(128, 128);
    let mut w = Walker::at([138.0, 0.0, 300.0]);
    for _ in 0..200 {
        w.step(&Input::default(), 0.015, &c, None);
    }
    assert!((w.feet[2] - 128.03125).abs() < 1e-3, "{:?}", w.feet);
}

#[test]
fn the_body_finds_the_slope_height_under_it() {
    let c = world(256, 128);
    let at = portalgun::walker::body_support([100.0, 0.0, 60.0], &c, None).unwrap();
    assert!((at - 50.0).abs() < 1e-3, "{at}");
}
