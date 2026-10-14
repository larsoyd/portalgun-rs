// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::prelude::*;
use portal::install::Install;
use portal::pair::Portal;
use portalgun::effects::{Effect, ParticleEffects, ParticleWorld};
use portalgun::portals::{Colour, Pair};
use std::path::Path;

fn install() -> Install {
    Install::open(&portal::install::root(
        None,
        std::env::var("PORTAL_DIR").ok().as_deref(),
        Path::new(&std::env::var("HOME").unwrap()),
    ))
    .unwrap()
}
fn portal(at: f32) -> Portal {
    Portal::new(
        [at, 0.0, 64.0],
        [-1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    )
}
#[test]
fn effect_names_and_colour_mapping() {
    for (c, n) in [(Colour::Blue, 1), (Colour::Orange, 2)] {
        for (effect, suffix) in [
            (Effect::Particles, "particles"),
            (Effect::Edge, "edge"),
            (Effect::Success, "success"),
            (Effect::Near, "near"),
            (Effect::Overlap, "overlap"),
            (Effect::NoFit, "nofit"),
            (Effect::BadVolume, "badvolume"),
            (Effect::BadSurface, "badsurface"),
            (Effect::Close, "close"),
        ] {
            assert_eq!(effect.name(c), format!("portal_{n}_{suffix}"));
        }
    }
}
#[test]
fn persistent_creation_replacement_and_cleanup() {
    let mut fx = ParticleEffects::read(&install()).unwrap();
    let mut pair = Pair::default();
    pair.place(Colour::Blue, portal(100.0));
    fx.sync(&pair);
    assert_eq!(fx.persistent_count(), 2);
    let world = ParticleWorld::default();
    fx.simulate(0.1, &world);
    for i in 0..100 {
        pair.place(Colour::Blue, portal(i as f32));
        fx.sync(&pair);
        fx.simulate(0.01, &world);
        assert_eq!(fx.persistent_count(), 2);
    }
    pair.place(Colour::Orange, portal(200.0));
    fx.sync(&pair);
    assert_eq!(fx.persistent_count(), 4);
    pair.deactivate(Colour::Blue);
    pair.deactivate(Colour::Orange);
    fx.sync(&pair);
    for _ in 0..200 {
        fx.simulate(0.05, &world);
    }
    assert_eq!(fx.instance_count(), 0);
}
#[test]
fn attachment_frames_are_model_driven_and_source_space() {
    let fx = ParticleEffects::read(&install()).unwrap();
    let p = portal(100.0);
    let b = fx.attachment_frame(Colour::Blue, Effect::Edge, &p);
    let o = fx.attachment_frame(Colour::Orange, Effect::Edge, &p);
    assert_ne!(b.forward, o.forward);
    let spin = 20.0_f32.to_radians().sin();
    assert!((b.forward.dot(Vec3::from(p.up)) + spin).abs() < 0.001);
    assert!((o.forward.dot(Vec3::from(p.up)) - spin).abs() < 0.001);
    assert!((b.origin - Vec3::from(p.origin)).length() < 0.001);
    for c in Colour::ALL {
        let f = fx.attachment_frame(c, Effect::Particles, &p);
        assert!(((f.origin - Vec3::from(p.origin)).length() - 5.0).abs() < 0.001);
        assert!((f.origin - Vec3::from(p.origin) - Vec3::from(p.forward) * 5.0).length() < 0.002);
        assert!(f.up.dot(Vec3::from(p.forward)) > 0.999);
        assert!((f.forward.length() - 1.0).abs() < 0.001);
        assert!((f.forward.dot(f.up)).abs() < 0.001);
    }
}
#[test]
fn installed_systems_are_deterministic_and_stay_in_source_units() {
    let install = install();
    let mut a = ParticleEffects::read(&install).unwrap();
    let mut b = ParticleEffects::read(&install).unwrap();
    let mut pair = Pair::default();
    pair.place(Colour::Blue, portal(100.0));
    pair.place(Colour::Orange, portal(200.0));
    a.sync(&pair);
    b.sync(&pair);
    let world = ParticleWorld::default();
    for dt in [0.016, 0.033, 0.1, 0.025].into_iter().cycle().take(80) {
        a.simulate(dt, &world);
        b.simulate(dt, &world);
    }
    assert_eq!(a.snapshot(), b.snapshot());
    assert!(a.snapshot().len() > 100);
    assert!(a
        .snapshot()
        .iter()
        .all(|p| p.xyz.iter().all(|v| v.is_finite())));
}

#[test]
fn installed_materials_preserve_blends_and_sheets() {
    use portalgun::particle_material::{Blend, MaterialSource};
    let install = install();
    let blue = MaterialSource::read(
        &install.search,
        "particle/fire_particle_4/fire_particle_4.vmt",
    )
    .unwrap()
    .unwrap();
    assert_eq!(blue.blend, Blend::Premultiplied);
    assert!(blue.card.is_some());
    assert!(blue.sheet.is_some());
    let orange = MaterialSource::read(&install.search, "effects/portal_2_particle.vmt")
        .unwrap()
        .unwrap();
    assert_eq!(orange.blend, Blend::AddAlpha);
    assert!(orange.card.is_none());
    let energy = MaterialSource::read(&install.search, "effects/energyball.vmt")
        .unwrap()
        .unwrap();
    assert_eq!(energy.blend, Blend::Add);
    assert!(
        MaterialSource::read(&install.search, "particle/warp2_warp.vmt")
            .unwrap()
            .is_none()
    );
}

#[test]
fn impact_selection_offsets_and_transients_finish() {
    use portal::placement::{Failure, Placement};
    use portalgun::impacts::{selection, PortalImpact};
    assert_eq!(selection(&Placement::Failed(Failure::Missed)), None);
    assert_eq!(selection(&Placement::Failed(Failure::PassThrough)), None);
    for (failure, effect) in [
        (Failure::BadSurface, Effect::BadSurface),
        (Failure::CantFit, Effect::NoFit),
        (Failure::Overlap, Effect::Overlap),
    ] {
        assert_eq!(selection(&Placement::Failed(failure)), Some(effect));
    }
    let p = portal(100.0);
    assert_eq!(
        selection(&Placement::Placed {
            portal: p,
            quality: 0.1
        }),
        Some(Effect::Success)
    );
    let mut fx = ParticleEffects::read(&install()).unwrap();
    for c in Colour::ALL {
        for e in [
            Effect::Success,
            Effect::Near,
            Effect::Overlap,
            Effect::NoFit,
            Effect::BadVolume,
            Effect::BadSurface,
            Effect::Close,
        ] {
            let impact = PortalImpact::at(c, e, &p);
            let offset = if e == Effect::Near { 16.0 } else { 1.0 };
            assert!(
                (impact.frame.origin - Vec3::from(p.origin) - Vec3::from(p.forward) * offset)
                    .length()
                    < 0.001
            );
            assert!(
                (impact.frame.forward.cross(impact.frame.up) - impact.frame.right).length() < 0.001
            );
            fx.spawn(c, e, impact.frame, Some([0.0, 0.0, 64.0]));
        }
    }
    assert_eq!(fx.instance_count(), 14);
    for _ in 0..400 {
        fx.simulate(0.05, &ParticleWorld::default());
    }
    assert_eq!(fx.instance_count(), 0);
}

#[test]
fn a_projectile_stream_follows_its_head_and_finishes_after_it_stops() {
    use portalgun::effects::Frame;
    assert_eq!(
        Effect::Stream.name(Colour::Orange),
        "portal_2_projectile_stream"
    );
    let mut fx = ParticleEffects::read(&install()).unwrap();
    let frame = Frame {
        origin: Vec3::ZERO,
        forward: Vec3::X,
        right: -Vec3::Y,
        up: Vec3::Z,
    };
    let id = fx.spawn(Colour::Blue, Effect::Stream, frame, None);
    let head = [300.0, 0.0, 64.0];
    for step in 1..=10 {
        let t = step as f32 / 10.0;
        fx.move_point(id, head.map(|v| v * t));
        fx.simulate(0.015, &ParticleWorld::default());
    }
    let near = fx
        .snapshot()
        .iter()
        .filter(|p| (Vec3::from(p.xyz) - Vec3::from(head)).length() < 40.0)
        .count();
    assert!(near > 0, "no stream particles near the head");
    fx.stop(id);
    for _ in 0..200 {
        fx.simulate(0.05, &ParticleWorld::default());
    }
    assert_eq!(fx.instance_count(), 0);
}

#[test]
fn a_stream_point_has_no_orientation_whatever_the_aim() {
    let frame = portalgun::effects::Frame::stream([10.0, 20.0, 30.0]);
    assert_eq!(frame.origin, Vec3::new(10.0, 20.0, 30.0));
    assert_eq!(
        (frame.forward, frame.right, frame.up),
        (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO)
    );
}
