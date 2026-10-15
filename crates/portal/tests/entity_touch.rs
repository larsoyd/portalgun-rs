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

fn world(classname: &str, keys: &[(&str, &str)]) -> World {
    let mut keys = keys.to_vec();
    keys.push(("targetname", "t"));
    let outputs = [
        ("OnStartTouch", "start"),
        ("OnEndTouch", "end"),
        ("OnEndTouchAll", "none"),
        ("OnTrigger", "fire"),
    ];
    let mut ents = vec![
        ent(classname, &keys, &outputs),
        ent("prop_physics", &[("targetname", "box")], &[]),
    ];
    ents.extend(
        outputs
            .iter()
            .map(|(_, n)| ent("logic_relay", &[("targetname", n)], &[])),
    );
    World::spawn(&ents)
}

/// the box sits inside the trigger while inside is set and reports every tick
struct Box {
    trigger: Handle,
    cube: Handle,
    inside: bool,
}

impl Env for Box {
    fn push(&mut self, _pusher: Handle, _delta: Vec3) -> Option<Handle> {
        None
    }

    fn overlapping(&mut self, trigger: Handle) -> Vec<Handle> {
        if self.inside && trigger == self.trigger {
            vec![self.cube]
        } else {
            Vec::new()
        }
    }

    fn movers(&mut self) -> Vec<(Handle, Vec<Handle>)> {
        let hits = if self.inside {
            vec![self.trigger]
        } else {
            Vec::new()
        };
        vec![(self.cube, hits)]
    }
}

fn boxed(world: &World, inside: bool) -> Box {
    Box {
        trigger: world.find("t").unwrap(),
        cube: world.find("box").unwrap(),
        inside,
    }
}

fn step(world: &mut World, env: &mut Box) -> Vec<String> {
    world.step(env);
    world
        .drain_fired()
        .into_iter()
        .filter(|f| f.caller == "t")
        .map(|f| f.target)
        .collect()
}

#[test]
fn a_new_link_starts_and_touches_at_once_and_a_stale_one_ends_that_tick() {
    let mut w = world("trigger_multiple", &[("spawnflags", "8"), ("wait", "1")]);
    let mut env = boxed(&w, true);
    assert_eq!(step(&mut w, &mut env), ["start", "fire"]);
    assert!(step(&mut w, &mut env).is_empty());
    env.inside = false;
    assert_eq!(step(&mut w, &mut env), ["end", "none"]);
}

#[test]
fn a_once_trigger_touched_on_the_tick_it_goes_away_stays_quiet() {
    let mut w = world("trigger_once", &[("spawnflags", "8")]);
    let mut env = boxed(&w, true);
    let fires: usize = (0..12)
        .map(|_| {
            step(&mut w, &mut env)
                .iter()
                .filter(|t| *t == "fire")
                .count()
        })
        .sum();
    assert_eq!(fires, 1);
    assert!(w.find("t").is_none());
}

#[test]
fn a_disabled_trigger_lets_go_when_the_mover_next_checks() {
    let mut w = world("trigger_multiple", &[("spawnflags", "8")]);
    let mut env = boxed(&w, true);
    step(&mut w, &mut env);
    w.input("t", "Disable", None);
    assert!(step(&mut w, &mut env).is_empty());
    assert_eq!(step(&mut w, &mut env), ["end", "none"]);
}

#[test]
fn enabling_a_trigger_around_a_resting_box_starts_its_touch_in_the_same_tick() {
    let mut w = world(
        "trigger_multiple",
        &[("spawnflags", "8"), ("StartDisabled", "1")],
    );
    let mut env = boxed(&w, true);
    assert!(step(&mut w, &mut env).is_empty());
    w.input("t", "Enable", None);
    assert_eq!(step(&mut w, &mut env), ["start", "fire"]);
}
