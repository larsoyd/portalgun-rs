// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::Path;

use portal::install::{root, Install};
use portal::pair::Portal;
use portalgun::portals::{overlay_look, Colour, Pair};

fn install() -> Install {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    Install::open(&dir).unwrap_or_else(|e| panic!("install at {dir:?}: {e}"))
}

fn wall(x: f32) -> Portal {
    Portal::new(
        [x, 0.0, 64.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    )
}

#[test]
fn each_colour_reads_its_static_overlay_material() {
    let i = install();
    let blue = overlay_look(&i.search, Colour::Blue).unwrap();
    assert_eq!(blue.noise, "models/portals/noise-blur-256x256");
    assert_eq!(blue.ramp, "models/portals/portal-blue-color");
    assert_eq!(blue.scale, 4.0);
    let orange = overlay_look(&i.search, Colour::Orange).unwrap();
    assert_eq!(orange.ramp, "models/portals/portal-orange-color");
}

#[test]
fn colours_pair_up_and_the_scripts_call_orange_red() {
    assert_eq!(Colour::Blue.other(), Colour::Orange);
    assert_eq!(Colour::Orange.other(), Colour::Blue);
    assert_eq!(
        (Colour::Blue.sound(), Colour::Orange.sound()),
        ("blue", "red")
    );
}

#[test]
fn a_pair_links_once_both_are_open_and_moving_one_restarts_its_opening() {
    let mut p = Pair::default();
    p.place(Colour::Blue, wall(-384.0));
    assert!(p.linked().is_none());
    assert_eq!(p.fade[1].statics, 0.0, "no partner to turn to static yet");
    for _ in 0..100 {
        p.think(0.01);
    }
    assert_eq!(p.fade[0].open, 1.0);
    p.place(Colour::Orange, wall(384.0));
    assert!(p.linked().is_some());
    assert_eq!(p.fade[1].open, 0.0);
    assert_eq!(p.fade[0].statics, 1.0, "the partner turns to static");
    let before = p.changes;
    p.place(Colour::Blue, wall(-300.0));
    assert_eq!(p.fade[0].open, 0.0);
    assert_eq!(p.changes, before + 1);
    assert_eq!(p.get(Colour::Blue).unwrap().origin[0], -300.0);
}

#[test]
fn the_face_sits_a_quarter_unit_out_and_winds_toward_the_viewer() {
    use portalgun::portals::face_quad;
    let p = wall(-384.0);
    let (corners, uvs) = face_quad(&p);
    for c in &corners {
        assert!((c[0] - (-383.75)).abs() < 1e-4, "{c:?}");
    }
    let [a, d, c] = [0, 1, 2].map(|i| corners[i]);
    let e1 = [0, 1, 2].map(|k| d[k] - a[k]);
    let e2 = [0, 1, 2].map(|k| c[k] - a[k]);
    let n = [
        e1[1] * e2[2] - e1[2] * e2[1],
        e1[2] * e2[0] - e1[0] * e2[2],
        e1[0] * e2[1] - e1[1] * e2[0],
    ];
    assert!(
        n[0] > 0.0 && n[1].abs() < 1e-3 && n[2].abs() < 1e-3,
        "{n:?}"
    );
    assert_eq!(uvs[0], [0.0, 0.0]);
    assert!(corners[0][2] > p.origin[2], "uv 0,0 is the top");
    assert!(
        corners[3][1] > corners[0][1],
        "u grows to the viewer's right"
    );
}

fn facing(x: f32, toward: f32) -> Portal {
    Portal::new(
        [x, 0.0, 64.0],
        [toward, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    )
}

#[test]
fn the_moved_portal_decides_the_doubler_and_its_partner_always_may() {
    let mut p = Pair::default();
    p.place(Colour::Blue, facing(-384.0, 1.0));
    assert_eq!(p.usable, [false, false], "a lone portal has no doubler");
    p.place(Colour::Orange, facing(384.0, -1.0));
    assert_eq!(p.usable, [true, true]);
    p.place(Colour::Orange, facing(384.0, 1.0));
    assert_eq!(p.usable, [true, false], "the partner keeps its mark");
    p.place(Colour::Blue, facing(-384.0, 1.0));
    assert_eq!(p.usable, [false, true]);
    let mut q = Pair::default();
    q.place(Colour::Blue, facing(0.0, 1.0));
    let askew = Portal::new(
        [10.0, 100.0, 64.0],
        [-0.766, 0.643, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    );
    q.place(Colour::Orange, askew);
    assert_eq!(q.usable, [true, false], "blue is not in front of orange");
}

#[test]
fn the_depth_doubler_masks_the_copied_screen_with_the_portal_mask() {
    use portalgun::portals::doubler_mask;
    assert_eq!(
        doubler_mask(&install().search).unwrap(),
        "models/portals/portal_mask"
    );
}
