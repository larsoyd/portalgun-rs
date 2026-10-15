// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::entity::{Sequence, Shape, World};
use portal::vmf::{Entity, Output};

fn ent(classname: &str, keys: &[(&str, &str)], outputs: &[(&str, &str)]) -> Entity {
    Entity {
        classname: classname.into(),
        keys: keys
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        outputs: outputs
            .iter()
            .map(|&(event, target)| Output {
                event: event.into(),
                target: target.into(),
                input: "Enable".into(),
                param: String::new(),
                delay: 0.0,
                times: -1,
            })
            .collect(),
        solids: Vec::new(),
    }
}

fn cover(default: &str) -> World {
    let keys = [
        ("targetname", "cover"),
        ("model", "models/props/box_dropper_cover.mdl"),
        ("DefaultAnim", default),
    ];
    let ents = [
        ent(
            "prop_dynamic",
            &keys,
            &[("OnAnimationBegun", "begun"), ("OnAnimationDone", "done")],
        ),
        ent("logic_relay", &[("targetname", "begun")], &[]),
        ent("logic_relay", &[("targetname", "done")], &[]),
    ];
    World::spawn_with(&ents, |e| Shape {
        sequences: if e.classname == "prop_dynamic" {
            vec![
                Sequence {
                    name: "idle".into(),
                    duration: 2.0,
                    looping: true,
                },
                Sequence {
                    name: "open".into(),
                    duration: 1.0,
                    looping: false,
                },
            ]
        } else {
            Vec::new()
        },
        ..Shape::default()
    })
}

fn outputs(world: &mut World) -> Vec<String> {
    world
        .drain_fired()
        .into_iter()
        .filter(|f| f.caller == "cover")
        .map(|f| f.target)
        .collect()
}

#[test]
fn a_one_shot_animation_reports_done_on_the_think_after_it_reaches_the_end() {
    let mut world = cover("");
    let h = world.find("cover").unwrap();
    world.input("cover", "SetAnimation", Some("open"));
    world.tick();
    assert_eq!(outputs(&mut world), ["begun"]);
    let mut done = None;
    for tick in 2..200 {
        world.tick();
        if !outputs(&mut world).is_empty() {
            done = Some(tick);
            break;
        }
    }
    assert_eq!(done, Some(78));
    assert_eq!(world.animation(h), Some((1, 1.0)));
    for _ in 0..30 {
        world.tick();
    }
    assert!(outputs(&mut world).is_empty());
}

#[test]
fn the_default_animation_starts_at_spawn_and_comes_back_after_a_one_shot() {
    let mut world = cover("idle");
    let h = world.find("cover").unwrap();
    world.tick();
    assert_eq!(outputs(&mut world), ["begun"]);
    assert_eq!(world.animation(h).map(|a| a.0), Some(0));
    world.input("cover", "SetAnimation", Some("open"));
    let mut seen = Vec::new();
    for tick in 2..102 {
        world.tick();
        seen.extend(
            outputs(&mut world)
                .into_iter()
                .map(|o| format!("{o}@{tick}")),
        );
    }
    assert_eq!(seen, ["begun@2", "done@77", "begun@77"]);
    assert_eq!(world.animation(h).map(|a| a.0), Some(0));
}

#[test]
fn an_unknown_sequence_falls_back_to_the_first_without_a_begun_output() {
    let mut world = cover("");
    let h = world.find("cover").unwrap();
    world.input("cover", "SetAnimation", Some("open"));
    world.tick();
    assert_eq!(world.animation(h), Some((1, 0.0)));
    outputs(&mut world);
    world.input("cover", "SetAnimation", Some("explode"));
    world.tick();
    assert!(outputs(&mut world).is_empty());
    assert_eq!(world.animation(h).map(|a| a.0), Some(0));
}

#[test]
fn a_looping_animation_never_reports_done() {
    let mut world = cover("idle");
    for _ in 0..400 {
        world.tick();
    }
    assert_eq!(outputs(&mut world), ["begun"]);
}

#[test]
fn skin_visibility_and_collision_inputs_set_the_props_state() {
    let mut world = cover("");
    let h = world.find("cover").unwrap();
    assert_eq!(
        (world.skin(h), world.visible(h), world.solid(h)),
        (0, true, true)
    );
    world.input("cover", "Skin", Some("3"));
    world.input("cover", "Disable", None);
    world.input("cover", "DisableCollision", None);
    world.tick();
    assert_eq!(
        (world.skin(h), world.visible(h), world.solid(h)),
        (3, false, false)
    );
    world.input("cover", "Enable", None);
    world.input("cover", "EnableCollision", None);
    world.tick();
    assert_eq!((world.visible(h), world.solid(h)), (true, true));
}
