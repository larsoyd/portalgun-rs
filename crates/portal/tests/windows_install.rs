// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::install::{root, Install};
use portal::particles::random::RandomTable;
use std::path::Path;

#[test]
fn the_windows_install_opens_and_supplies_the_particle_table() {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    let opened = Install::open(&dir);
    assert!(
        opened.is_ok(),
        "Windows install refused: {:?}",
        opened.err()
    );
    let install = opened.unwrap();
    let table = RandomTable::from_client(&install.client);
    assert!(
        table.is_ok(),
        "Windows particle table refused: {:?}",
        table.err()
    );
}
