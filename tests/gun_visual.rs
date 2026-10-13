// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::prelude::*;
use portal::gun_fx::{GunFx, State};
use portalgun::gun_visual::Geometry;
use portalgun::units::METRES_PER_UNIT;

#[test]
fn glow_quads_keep_the_attachment_centre_and_source_size() {
    let mut fx = GunFx::new(0.0);
    fx.set_state(State::Holding, Some(1), 0.0);
    let sprite = fx.sprites(0.0)[0];
    let at = Vec3::new(7.0, 8.0, 9.0);
    let mut mesh = Geometry::default();
    mesh.sprite(&sprite, at, Vec3::Y, Vec3::Z);
    let centre = mesh.positions.iter().map(|&p| Vec3::from(p)).sum::<Vec3>() / 4.0;
    assert!((centre - at).length() < 0.000001);
    assert!(
        ((Vec3::from(mesh.positions[0]) - Vec3::from(mesh.positions[1])).length()
            - sprite.size * METRES_PER_UNIT)
            .abs()
            < 0.000001
    );
    assert_eq!(mesh.indices.len(), 6);
}

#[test]
fn beam_ribbons_use_noise_and_radius_and_premultiply_brightness() {
    let mut fx = GunFx::new(0.0);
    fx.set_state(State::Holding, None, 0.0);
    let beam = fx.beams()[0];
    let mut mesh = Geometry::default();
    mesh.beam(
        &beam,
        Vec3::ZERO,
        Vec3::X * 20.0 * METRES_PER_UNIT,
        Vec3::Z,
        Vec3::NEG_Z,
        &[0.0; 129],
        0.0,
    );
    assert_eq!(mesh.indices.len(), 7 * 6);
    assert_eq!(
        mesh.colors[0],
        [128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0, 1.0]
    );
    let n = mesh.positions.len();
    assert!(
        ((Vec3::from(mesh.positions[n - 2]) - Vec3::from(mesh.positions[n - 1])).length()
            - 4.0 * METRES_PER_UNIT)
            .abs()
            < 0.000001
    );
    assert_eq!(mesh.uvs[0], [1.0, 0.0]);
    assert_eq!(mesh.uvs[1], [0.0, 0.0]);
    assert!((mesh.uvs[n - 2][1] - 0.2).abs() < 0.000001);
    let straight = mesh.positions.clone();
    let mut jittered = Geometry::default();
    jittered.beam(
        &beam,
        Vec3::ZERO,
        Vec3::X * 20.0 * METRES_PER_UNIT,
        Vec3::Z,
        Vec3::NEG_Z,
        &[0.5; 129],
        0.0,
    );
    assert_ne!(straight, jittered.positions);
}

#[test]
fn portal_clipping_preserves_uvs_and_removes_the_hidden_half() {
    let mut fx = GunFx::new(0.0);
    fx.set_state(State::Ready, None, 0.0);
    let sprite = fx.sprites(0.0)[0];
    let mut mesh = Geometry::default();
    mesh.sprite(&sprite, Vec3::ZERO, Vec3::X, Vec3::Y);
    mesh.clip(Vec4::X);
    assert!(!mesh.indices.is_empty());
    for (p, uv) in mesh.positions.iter().zip(&mesh.uvs) {
        assert!(p[0] >= 0.0);
        assert!(uv[0] >= 0.5);
    }
    mesh.clip(Vec4::new(-1.0, 0.0, 0.0, -1.0));
    assert!(mesh.indices.is_empty());
}

#[test]
fn owned_gun_effect_assets_have_every_named_attachment_and_material() {
    let root = portal::install::root(
        None,
        std::env::var("PORTAL_DIR").ok().as_deref(),
        &std::path::PathBuf::from(std::env::var("HOME").unwrap()),
    );
    let install = portal::install::Install::open(&root).unwrap();
    for (name, mut fx) in [
        ("models/weapons/v_portalgun", GunFx::new(0.0)),
        ("models/weapons/w_portalgun", GunFx::world(0.0)),
    ] {
        let mdl =
            portal::mdl::Mdl::parse(install.search.read(&format!("{name}.mdl")).unwrap()).unwrap();
        fx.set_state(State::Holding, Some(1), 0.0);
        fx.think(Some(1), 0.01, 0.01, &mut || 0.0);
        for sprite in fx.sprites(0.1) {
            assert!(
                mdl.attachment(sprite.attachment).is_some(),
                "{name}: {}",
                sprite.attachment
            );
            portalgun::particle_material::MaterialSource::read_gun(
                &install.search,
                sprite.material,
            )
            .unwrap();
        }
        for beam in fx.beams() {
            assert!(mdl.attachment(beam.start).is_some());
            assert!(mdl.attachment(beam.end).is_some());
            portalgun::particle_material::MaterialSource::read_gun(&install.search, beam.material)
                .unwrap();
        }
    }
}
