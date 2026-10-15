// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::Path;

use bevy::prelude::*;
use portal::install::{root, Install};
use portalgun::viewmodel::{look, read_model};

#[test]
fn a_phong_texture_named_while_phong_is_off_is_never_loaded() {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    let files = Install::open(&dir).unwrap().search;
    let (mdl, _) = read_model(&files, "models/props/box_dropper").unwrap();
    let tube = (0..mdl.textures.len())
        .find(|&i| mdl.textures[i].eq_ignore_ascii_case("box_dropper_tube"))
        .unwrap_or_else(|| panic!("no tube material in {:?}", mdl.textures));
    let l = look(&files, &mdl, tube).unwrap();
    assert!(!l.phong.enabled);
    assert!(l.phong.exponent.is_some());
    let mut images = Assets::<Image>::default();
    let mut material = portalgun::bodyportal::material(StandardMaterial::default());
    portalgun::model_material::apply(&l, &files, &mut images, &mut material).unwrap();
    assert!(material.extension.exponent.is_none());
}
