// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::dmx::{Element, Value};
use portal::pcf::systems;

fn element(kind: &str, name: &str, attributes: Vec<(&str, Value)>) -> Element {
    Element {
        kind: kind.into(),
        name: name.into(),
        attributes: attributes.into_iter().map(|(n, v)| (n.into(), v)).collect(),
    }
}

fn elements() -> Vec<Element> {
    let refs = |r: &[usize]| Value::Array(r.iter().map(|&i| Value::Element(Some(i))).collect());
    vec![
        element(
            "DmeElement",
            "root",
            vec![("particleSystemDefinitions", refs(&[1, 4]))],
        ),
        element(
            "DmeParticleSystemDefinition",
            "portal_1_projectile_stream",
            vec![
                ("material", Value::String("effects\\energyball.vmt".into())),
                ("max_particles", Value::Int(64)),
                ("initial_particles", Value::Int(2)),
                ("radius", Value::Float(5.0)),
                ("color", Value::Color([255, 198, 0, 255])),
                ("maximum time step", Value::Float(0.2)),
                ("maximum sim tick rate", Value::Float(0.025)),
                ("minimum sim tick rate", Value::Float(0.01)),
                ("minimum rendered frames", Value::Int(1)),
                ("rotation", Value::Float(1.5)),
                ("rotation_speed", Value::Float(2.5)),
                ("sequence_number", Value::Int(3)),
                ("sequence_number 1", Value::Int(4)),
                ("operators", refs(&[2])),
                ("emitters", refs(&[3])),
                ("children", refs(&[5])),
            ],
        ),
        element(
            "DmeParticleOperator",
            "Movement Basic",
            vec![
                ("functionName", Value::String("Movement Basic".into())),
                ("drag", Value::Float(0.3)),
            ],
        ),
        element(
            "DmeParticleOperator",
            "emit",
            vec![
                ("functionName", Value::String("emit_continuously".into())),
                ("emission_rate", Value::Float(30.0)),
            ],
        ),
        element(
            "DmeParticleSystemDefinition",
            "portal_1_projectile_trail",
            vec![("max_particles", Value::Int(50))],
        ),
        element(
            "DmeParticleChild",
            "portal_1_projectile_trail",
            vec![
                ("child", Value::Element(Some(4))),
                ("delay", Value::Float(0.25)),
            ],
        ),
    ]
}

#[test]
fn systems_carry_their_functions_and_children() {
    let s = systems(&elements()).unwrap();
    assert_eq!(s.len(), 2);
    let stream = &s[0];
    assert_eq!(stream.name, "portal_1_projectile_stream");
    assert_eq!(stream.material, "effects\\energyball.vmt");
    assert_eq!((stream.max_particles, stream.initial_particles), (64, 2));
    assert_eq!(stream.radius, 5.0);
    assert_eq!(stream.color, [255, 198, 0, 255]);
    assert_eq!(stream.max_time_step, 0.2);
    assert_eq!(
        (stream.max_sim, stream.min_sim, stream.min_frames),
        (0.025, 0.01, 1)
    );
    assert_eq!((stream.rotation, stream.rotation_speed), (1.5, 2.5));
    assert_eq!((stream.sequence, stream.sequence1), (3, 4));
    assert_eq!(stream.operators.len(), 1);
    assert_eq!(stream.operators[0].name, "Movement Basic");
    assert_eq!(stream.operators[0].float("drag"), 0.3);
    assert_eq!(stream.emitters[0].name, "emit_continuously");
    assert!(stream.renderers.is_empty());
    assert_eq!(stream.children.len(), 1);
    assert_eq!(stream.children[0].system, 1);
    assert_eq!(stream.children[0].delay, 0.25);
    let trail = &s[1];
    assert_eq!(trail.max_particles, 50);
    assert_eq!(trail.material, "vgui/white", "client.so's defaults");
    assert_eq!((trail.radius, trail.max_time_step), (5.0, 0.1));
    assert_eq!(
        (trail.max_sim, trail.min_frames, trail.sequence),
        (0.0, 0, 0)
    );
    assert_eq!(trail.initial_particles, 0);
}

#[test]
fn a_child_naming_no_system_is_refused() {
    let mut e = elements();
    e[5].attributes[0].1 = Value::Element(Some(2));
    assert!(systems(&e).is_err());
}

#[test]
fn a_function_without_its_name_is_refused() {
    let mut e = elements();
    e[2].attributes.remove(0);
    assert!(systems(&e).is_err());
}
