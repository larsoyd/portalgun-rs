// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::entity::io::Handle;
use portal::entity::{Env, World};
use portal::pair::Vec3;
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

fn chamber() -> World {
    let outputs = [
        ("OnDissolveBox", "box_gone"),
        ("OnDissolve", "gone"),
        ("OnFizzle", "fizzled"),
    ];
    let mut ents = vec![
        ent(
            "trigger_portal_cleanser",
            &[("targetname", "fryer"), ("spawnflags", "65")],
            &outputs,
        ),
        ent("prop_physics", &[("targetname", "box")], &[]),
        ent("prop_physics", &[("targetname", "crate")], &[]),
        ent("func_door", &[("targetname", "door")], &[]),
        ent("func_physbox", &[("targetname", "slab")], &[]),
    ];
    ents.extend(
        outputs
            .iter()
            .map(|(_, n)| ent("logic_relay", &[("targetname", n)], &[])),
    );
    World::spawn(&ents)
}

#[derive(Default)]
struct Fryer {
    inside: Vec<(Handle, Handle)>,
    portals_open: bool,
    fizzles: usize,
    dissolved: Vec<Handle>,
}

impl Env for Fryer {
    fn push(&mut self, _pusher: Handle, _delta: Vec3) -> Option<Handle> {
        None
    }

    fn movers(&mut self) -> Vec<(Handle, Vec<Handle>)> {
        self.inside.iter().map(|&(m, t)| (m, vec![t])).collect()
    }

    fn fizzle(&mut self, _player: Handle) -> bool {
        self.fizzles += 1;
        std::mem::take(&mut self.portals_open)
    }

    fn dissolve(&mut self, entity: Handle) {
        self.dissolved.push(entity);
    }
}

fn step(world: &mut World, env: &mut Fryer) -> Vec<String> {
    world.step(env);
    world
        .drain_fired()
        .into_iter()
        .filter(|f| f.caller == "fryer")
        .map(|f| f.target)
        .collect()
}

#[test]
fn the_box_is_reported_then_dissolved_and_gone_by_the_end_of_the_tick() {
    let mut world = chamber();
    let (fryer, cube) = (world.find("fryer").unwrap(), world.find("box").unwrap());
    let mut env = Fryer {
        inside: vec![(cube, fryer), (cube, fryer)],
        ..Fryer::default()
    };
    assert_eq!(step(&mut world, &mut env), ["box_gone", "gone"]);
    assert_eq!(env.dissolved, [cube]);
    assert!(world.find("box").is_none());
}

#[test]
fn other_props_dissolve_without_the_box_output() {
    let mut world = chamber();
    let (fryer, other) = (world.find("fryer").unwrap(), world.find("crate").unwrap());
    let mut env = Fryer {
        inside: vec![(other, fryer)],
        ..Fryer::default()
    };
    assert_eq!(step(&mut world, &mut env), ["gone"]);
}

#[test]
fn doors_pass_through_the_field_untouched() {
    let mut world = chamber();
    let (fryer, door) = (world.find("fryer").unwrap(), world.find("door").unwrap());
    let mut env = Fryer {
        inside: vec![(door, fryer)],
        ..Fryer::default()
    };
    assert!(step(&mut world, &mut env).is_empty());
    assert!(env.dissolved.is_empty());
}

#[test]
fn the_player_fizzles_portals_every_tick_but_only_reports_when_some_were_up() {
    let mut world = chamber();
    let player = world.add_player();
    let fryer = world.find("fryer").unwrap();
    let mut env = Fryer {
        inside: vec![(player, fryer)],
        portals_open: true,
        ..Fryer::default()
    };
    assert_eq!(step(&mut world, &mut env), ["fizzled"]);
    assert!(step(&mut world, &mut env).is_empty());
    assert_eq!(env.fizzles, 2);
}

#[test]
fn brush_objects_are_not_animating_and_never_dissolve() {
    let mut world = chamber();
    let (fryer, slab) = (world.find("fryer").unwrap(), world.find("slab").unwrap());
    let mut env = Fryer {
        inside: vec![(slab, fryer)],
        ..Fryer::default()
    };
    assert!(step(&mut world, &mut env).is_empty());
    assert!(world.find("slab").is_some());
}
