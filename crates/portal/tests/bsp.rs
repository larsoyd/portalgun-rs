// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::{Path, PathBuf};

use portal::bsp::{entities, open};
use portal::install::root;
use portal::vmf::Entity;

fn steam() -> PathBuf {
    root(
        None,
        None,
        Path::new(&std::env::var_os("HOME").unwrap_or_default()),
    )
}

fn named<'a>(ents: &'a [Entity], name: &str) -> &'a Entity {
    let found: Vec<_> = ents
        .iter()
        .filter(|e| e.get("targetname") == Some(name))
        .collect();
    assert_eq!(found.len(), 1, "{name}");
    found[0]
}

#[test]
fn reads_the_intro_fade_and_exit_elevator_of_the_first_chamber() {
    let ents = open(&steam(), "testchmb_a_00").unwrap_or_else(|e| panic!("{e}"));
    let fade = named(&ents, "fade_intro");
    assert_eq!(fade.classname, "env_fade");
    assert_eq!(
        [
            fade.get("duration"),
            fade.get("holdtime"),
            fade.get("spawnflags")
        ],
        [Some("4"), Some("3"), Some("1")]
    );
    let train = named(&ents, "body");
    assert_eq!(train.classname, "func_tracktrain");
    assert_eq!(train.get("startspeed"), Some("65"));
    let z = |n: &str| named(&ents, n).origin().unwrap()[2];
    assert_eq!(z("a00-a01_track_elevator_p1"), 664.0);
    assert_eq!(z("a00-a01_track_elevator_p4"), 1240.0);
    let starts: Vec<_> = named(&ents, "a00-a01_trigger_elevator_move")
        .outputs
        .iter()
        .filter(|o| o.input == "StartForward")
        .map(|o| (o.event.as_str(), o.target.as_str(), o.delay))
        .collect();
    assert_eq!(starts, [("OnStartTouch", "body", 5.0)]);
}

#[test]
fn refuses_a_map_it_has_no_hash_for() {
    let err = open(&steam(), "testchmb_a_15").unwrap_err().to_string();
    assert!(
        err.contains("refused") && err.contains("testchmb_a_15"),
        "{err}"
    );
}

#[test]
fn refuses_a_changed_map() {
    let dir = std::env::temp_dir().join(format!("portal-bsp-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("portal/maps")).unwrap();
    let mut bytes = std::fs::read(steam().join("portal/maps/testchmb_a_00.bsp")).unwrap();
    bytes[100] ^= 1;
    std::fs::write(dir.join("portal/maps/testchmb_a_00.bsp"), bytes).unwrap();
    let err = open(&dir, "testchmb_a_00").unwrap_err().to_string();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(err.contains("sha256"), "{err}");
}

#[test]
fn refuses_anything_but_version_20() {
    let mut bytes = b"VBSP\x13\0\0\0".to_vec();
    bytes.resize(1036, 0);
    let err = entities(&bytes).unwrap_err().to_string();
    assert!(err.contains("version 19"), "{err}");
    let err = entities(b"IBSP\x14\0\0\0").unwrap_err().to_string();
    assert!(err.contains("VBSP"), "{err}");
}
