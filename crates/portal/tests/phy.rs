// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::Path;

use portal::install::root;
use portal::phy::parse;
use portal::vpk::Pak;

fn metal_box() -> Vec<u8> {
    let steam = root(
        None,
        None,
        Path::new(&std::env::var_os("HOME").unwrap_or_default()),
    );
    let pak = Pak::open(&steam.join("portal/portal_pak_dir.vpk")).unwrap_or_else(|e| panic!("{e}"));
    pak.read("models/props/metal_box.phy")
        .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn the_metal_box_phy_gives_its_inertia_radius_and_solid_keys() {
    let phy = parse(&metal_box()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(phy.solids.len(), 1);
    let s = &phy.solids[0];
    assert_eq!(s.unit_inertia, [0.12165254, 0.12122901, 0.12122901]);
    assert_eq!(s.radius, 0.87988186);
    assert!(
        s.mass_center.iter().all(|c| c.abs() < 1e-3),
        "{:?}",
        s.mass_center
    );
    let p = &phy.params[0];
    assert_eq!(p.mass, 40.000004);
    assert_eq!(
        (p.damping, p.rotdamping, p.inertia, p.drag),
        (0.1, 0.0, 1.0, 0.0)
    );
    assert_eq!(p.surfaceprop, "metalpanel");
}

#[test]
fn a_phy_without_the_vphy_marker_is_refused() {
    let mut bytes = metal_box();
    bytes[20] = b'X';
    let err = parse(&bytes).unwrap_err().to_string();
    assert!(err.contains("VPHY"), "{err}");
}

#[test]
fn a_truncated_phy_is_refused() {
    let bytes = metal_box();
    assert!(parse(&bytes[..40]).is_err());
}
