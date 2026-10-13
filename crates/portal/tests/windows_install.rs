// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::install::{root, Install};
use portal::particles::random::RandomTable;
use std::path::Path;

#[test]
fn the_verified_install_supplies_its_binary_formats_particle_table() {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    let opened = Install::open(&dir);
    assert!(
        opened.is_ok(),
        "Verified install refused: {:?}",
        opened.err()
    );
    let install = opened.unwrap();
    let table = RandomTable::from_client(&install.client);
    assert!(table.is_ok(), "Particle table refused: {:?}", table.err());
}
