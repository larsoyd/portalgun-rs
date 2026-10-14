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

fn slanted_portal(c: &Chamber) -> portal::pair::Portal {
    use portal::placement::{place, Placement};
    let eye: [f32; 3] = [-200.0, 0.0, 150.0];
    let target = [128.0, 0.0, 128.0];
    let d = [target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    match place(c, eye, d.map(|v| v / len), 10000.0, None).0 {
        Placement::Placed { portal, .. } => portal,
        other => panic!("no portal on the slope: {other:?}"),
    }
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4)
}

#[test]
fn a_portal_on_a_slope_faces_out_of_it_with_its_top_uphill() {
    let c = world(256, 256);
    let p = slanted_portal(&c);
    let h = std::f32::consts::FRAC_1_SQRT_2;
    assert!(close(p.forward, [-h, 0.0, h]), "{:?}", p.forward);
    assert!(close(p.up, [h, 0.0, h]), "{:?}", p.up);
    let along = p.origin[2] - p.origin[0];
    assert!(
        along.abs() < 0.1,
        "the centre sits on the slope, {:?}",
        p.origin
    );
}

fn fall_out_of_the_slope(crouch: bool) -> Walker {
    use portal::pair::Portal;
    let c = world(256, 256);
    let floor = Portal::new(
        [-512.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        (32.0, 54.0),
    );
    let pair = [floor, slanted_portal(&c)];
    let mut w = Walker::at([-512.0, 0.0, 200.0]);
    let input = Input {
        crouch,
        ..Input::default()
    };
    let mut out = None;
    for _ in 0..300 {
        if w.step(&input, 0.015, &c, Some(pair)).is_some() {
            out = Some(w.velocity);
            break;
        }
    }
    let v = out.unwrap_or_else(|| panic!("never crossed, feet {:?}", w.feet));
    assert!(v[2] >= 300.0, "a 45 degree exit counts as a floor, {v:?}");
    assert!(v[0] < 0.0, "out of the slope towards -x, {v:?}");
    for _ in 0..400 {
        w.step(&input, 0.015, &c, Some(pair));
    }
    w
}

#[test]
fn falling_through_a_floor_portal_out_of_a_slope_lands_clear_of_it() {
    for crouch in [false, true] {
        let w = fall_out_of_the_slope(crouch);
        assert!(w.grounded, "crouch {crouch} {:?}", w.feet);
        assert!(
            w.feet[2] < 1.0 && w.feet[0] < -16.0,
            "crouch {crouch} {:?}",
            w.feet
        );
    }
}
pub fn world_pub(run: i32, rise: i32) -> Chamber {
    world(run, rise)
}
pub fn slanted_pub(c: &Chamber) -> portal::pair::Portal {
    slanted_portal(c)
}
