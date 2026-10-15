// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::Path;

use portal::anim::cycle_rate;
use portal::install::root;
use portal::mdl::Mdl;
use portal::vpk::Pak;

fn model(name: &str) -> Mdl {
    let steam = root(
        None,
        None,
        Path::new(&std::env::var_os("HOME").unwrap_or_default()),
    );
    let pak = Pak::open(&steam.join("portal/portal_pak_dir.vpk")).unwrap_or_else(|e| panic!("{e}"));
    Mdl::parse(pak.read(name).unwrap_or_else(|e| panic!("{name}: {e}")))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn the_chamber_props_have_no_sequence_transition_graph() {
    for name in [
        "models/props/button_top_reference.mdl",
        "models/props/door_01_lftdoor_reference.mdl",
        "models/props/door_01_rtdoor_reference.mdl",
        "models/props/box_dropper.mdl",
        "models/props/box_dropper_cover.mdl",
        "models/props/metal_box.mdl",
    ] {
        let mdl = model(name);
        for s in &mdl.sequences {
            assert_eq!(s.nodes, [0, 0], "{name} {}", s.label);
        }
    }
}

#[test]
fn the_dropper_cover_opens_and_closes_once_each() {
    let mdl = model("models/props/box_dropper_cover.mdl");
    let seq = |label: &str| {
        let s = &mdl.sequences[mdl
            .sequence_named(label)
            .unwrap_or_else(|| panic!("{label}"))];
        (s.looping, 1.0 / cycle_rate(&mdl.anims[s.anim]))
    };
    let (open, close) = (seq("open"), seq("close"));
    assert!(!open.0 && !close.0);
    assert!(open.1 > 0.0 && close.1 > 0.0, "{open:?} {close:?}");
}
