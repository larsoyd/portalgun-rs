use std::path::Path;

use portal::install::{root, Install};
use portal::pair::Portal;
use portalgun::portals::{overlay_look, Colour, Pair};

fn install() -> Install {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    Install::open(&dir).unwrap_or_else(|e| panic!("install at {dir:?}: {e}"))
}

fn wall(x: f32) -> Portal {
    Portal::new([x, 0.0, 64.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], (32.0, 54.0))
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
    assert_eq!((Colour::Blue.sound(), Colour::Orange.sound()), ("blue", "red"));
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
