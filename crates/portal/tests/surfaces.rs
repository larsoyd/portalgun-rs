// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::Path;

use portal::install::{root, Install};
use portal::keyvalues;
use portal::surfaces::{Surface, Surfaces};

fn install() -> Install {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    Install::open(&dir).unwrap_or_else(|e| panic!("install at {dir:?}: {e}"))
}

#[test]
fn concrete_and_metal_step_on_their_own_sounds() {
    let s = Surfaces::read(&install().search).unwrap();
    let concrete = s.get("concrete");
    assert_eq!(concrete.step_left, "Concrete.StepLeft");
    assert_eq!(concrete.step_right, "Concrete.StepRight");
    assert_eq!(concrete.material, b'C');
    let metal = s.get("metal");
    assert_eq!(
        metal.step_left, "SolidMetal.StepLeft",
        "metal takes solidmetal's steps"
    );
    assert_eq!(metal.material, b'M');
}

#[test]
fn an_unknown_surface_is_the_default() {
    let s = Surfaces::read(&install().search).unwrap();
    assert_eq!(s.get("no_such_surface"), s.get("default"));
    assert_eq!(s.get("CONCRETE"), s.get("concrete"));
}

#[test]
fn a_surface_starts_from_its_base_or_the_default() {
    let text = r#""default" { "stepleft" "D.L" "stepright" "D.R" "gamematerial" "C" }
        "plain" { "stepleft" "P.L" }
        "child" { "base" "plain" "gamematerial" "T" }"#;
    let s = Surfaces::from_files(&[keyvalues::parse(text).unwrap()]);
    assert_eq!(
        s.get("plain"),
        &Surface {
            step_left: "P.L".into(),
            step_right: "D.R".into(),
            material: b'C',
            ..Surface::default()
        }
    );
    assert_eq!(s.get("child").step_left, "P.L");
    assert_eq!(s.get("child").material, b'T');
}

#[test]
fn physics_properties_inherit_and_override_independently() {
    let text = r#""default" { "friction" "0.8" "jumpfactor" "1.25" "maxspeedfactor" "0.75" }
        "ice" { "friction" "0.1" }
        "child" { "base" "ice" "jumpfactor" "0.5" }"#;
    let s = Surfaces::from_files(&[keyvalues::parse(text).unwrap()]);
    assert_eq!(s.get("child").friction, 0.1);
    assert_eq!(s.get("child").jump_factor, 0.5);
    assert_eq!(s.get("child").max_speed_factor, 0.75);
    assert_eq!(Surfaces::from_files(&[]).get("missing").friction, 1.0);
}

#[test]
fn repeated_surface_definitions_retain_properties_and_apply_base_in_order() {
    let first =
        keyvalues::parse(r#""default" { "friction" "0.8" } "custom" { "jumpfactor" "1.5" }"#)
            .unwrap();
    let second = keyvalues::parse(
        r#""custom" { "maxspeedfactor" "0.5" } "ordered" { "friction" "0.2" "base" "custom" }"#,
    )
    .unwrap();
    let s = Surfaces::from_files(&[first, second]);
    assert_eq!(s.get("custom").jump_factor, 1.5);
    assert_eq!(s.get("ordered").friction, 0.8);
    assert_eq!(s.get("ordered").max_speed_factor, 0.5);
}

#[test]
fn metal_panels_bounce_at_a_fifth_and_grip_at_point_eight() {
    let s = Surfaces::read(&install().search).unwrap();
    let panel = s.get("metalpanel");
    assert_eq!(panel.elasticity, 0.2);
    assert_eq!(panel.friction, 0.8);
    assert_eq!(s.get("default").elasticity, 0.25);
}

#[test]
fn elasticity_inherits_and_falls_back_to_the_physics_parser_default() {
    let text = r#""default" { "elasticity" "0.25" }
        "rubber" { "elasticity" "2" }
        "child" { "base" "rubber" }"#;
    let s = Surfaces::from_files(&[keyvalues::parse(text).unwrap()]);
    assert_eq!(s.get("child").elasticity, 2.0);
    assert_eq!(Surfaces::from_files(&[]).get("missing").elasticity, 0.001);
}

#[test]
fn metal_panels_inherit_solid_metal_impacts_and_the_default_audio_factors() {
    let s = Surfaces::read(&install().search).unwrap();
    let panel = s.get("metalpanel");
    assert_eq!(panel.impact_hard, "SolidMetal.ImpactHard");
    assert_eq!(panel.impact_soft, "SolidMetal.ImpactSoft");
    assert_eq!(panel.scrape_rough, "SolidMetal.ScrapeRough");
    assert_eq!(panel.scrape_smooth, "SolidMetal.ScrapeSmooth");
    assert_eq!(panel.hardness, 1.0);
    assert_eq!(panel.roughness, 0.1);
    assert_eq!(panel.scrape_rough_threshold, 0.5);
    assert_eq!(panel.impact_hard_threshold, 0.5);
    assert_eq!(panel.hard_min_velocity, 0.0);
    assert_eq!(s.get("concrete").roughness, 1.0);
}
