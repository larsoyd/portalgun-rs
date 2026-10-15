// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::entity::{Fired, World};
use portal::vmf::{Entity, Output};

fn ent(
    classname: &str,
    keys: &[(&str, &str)],
    outputs: &[(&str, &str, &str, &str, f32, i32)],
) -> Entity {
    Entity {
        classname: classname.into(),
        keys: keys
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        outputs: outputs
            .iter()
            .map(|&(event, target, input, param, delay, times)| Output {
                event: event.into(),
                target: target.into(),
                input: input.into(),
                param: param.into(),
                delay,
                times,
            })
            .collect(),
        solids: Vec::new(),
    }
}

fn inputs(world: &mut World) -> Vec<String> {
    world
        .drain_fired()
        .into_iter()
        .map(|f: Fired| f.to_string())
        .collect()
}

fn run(world: &mut World, seconds: f32) -> Vec<String> {
    let mut all = Vec::new();
    let ticks = (seconds / World::TICK).round() as u32;
    for _ in 0..ticks {
        world.tick();
        all.extend(inputs(world));
    }
    all
}

fn probe(name: &str) -> Entity {
    ent("logic_relay", &[("targetname", name)], &[])
}

fn relay_world(spawnflags: &str) -> World {
    World::spawn(&[
        ent(
            "logic_relay",
            &[("targetname", "relay"), ("spawnflags", spawnflags)],
            &[("OnTrigger", "probe", "Enable", "", 0.5, -1)],
        ),
        probe("probe"),
    ])
}

#[test]
fn a_relay_ignores_triggers_until_its_outputs_have_had_time_to_fire() {
    let mut world = relay_world("0");
    world.input("relay", "Trigger", None);
    world.input("relay", "Trigger", None);
    assert_eq!(
        run(&mut world, 0.495),
        ["relay.Trigger()", "relay.Trigger()"]
    );
    assert_eq!(
        run(&mut world, 0.03),
        ["probe.Enable()", "relay.EnableRefire()"]
    );
    world.input("relay", "Trigger", None);
    assert_eq!(
        run(&mut world, 0.6),
        ["relay.Trigger()", "probe.Enable()", "relay.EnableRefire()"]
    );
}

#[test]
fn a_fast_retrigger_relay_queues_every_trigger() {
    let mut world = relay_world("2");
    world.input("relay", "Trigger", None);
    world.input("relay", "Trigger", None);
    assert_eq!(
        run(&mut world, 0.6),
        [
            "relay.Trigger()",
            "relay.Trigger()",
            "probe.Enable()",
            "probe.Enable()"
        ]
    );
}

#[test]
fn a_trigger_once_relay_removes_itself_after_firing() {
    let mut world = relay_world("1");
    world.input("relay", "Trigger", None);
    run(&mut world, 1.0);
    assert!(world.find("relay").is_none());
    world.input("relay", "Trigger", None);
    assert_eq!(run(&mut world, 1.0), ["relay.Trigger() unhandled"]);
}

#[test]
fn a_disabled_relay_stays_quiet_until_enabled_and_toggle_flips_it() {
    let mut world = World::spawn(&[
        ent(
            "logic_relay",
            &[
                ("targetname", "relay"),
                ("StartDisabled", "1"),
                ("spawnflags", "2"),
            ],
            &[("OnTrigger", "probe", "Enable", "", 0.0, -1)],
        ),
        probe("probe"),
    ]);
    world.input("relay", "Trigger", None);
    assert_eq!(run(&mut world, 0.03), ["relay.Trigger()"]);
    world.input("relay", "Toggle", None);
    world.input("relay", "Trigger", None);
    assert_eq!(
        run(&mut world, 0.03),
        ["relay.Toggle()", "relay.Trigger()", "probe.Enable()"]
    );
}

#[test]
fn cancel_pending_drops_what_the_relay_queued_and_rearms_it() {
    let mut world = relay_world("0");
    world.input("relay", "Trigger", None);
    run(&mut world, 0.015);
    world.input("relay", "CancelPending", None);
    world.input("relay", "Trigger", None);
    assert_eq!(
        run(&mut world, 0.6),
        [
            "relay.CancelPending()",
            "relay.Trigger()",
            "probe.Enable()",
            "relay.EnableRefire()"
        ]
    );
}

#[test]
fn an_unknown_name_falls_back_to_the_classname_then_reports_unhandled() {
    let mut world = World::spawn(&[probe("a"), probe("b")]);
    world.input("logic_relay", "Enable", None);
    world.input("nobody", "Enable", None);
    assert_eq!(
        run(&mut world, 0.015),
        ["a.Enable()", "b.Enable()", "nobody.Enable() unhandled"]
    );
}

#[test]
fn activator_and_caller_names_resolve_to_the_event_entities() {
    let mut world = World::spawn(&[
        ent(
            "logic_relay",
            &[("targetname", "relay"), ("spawnflags", "2")],
            &[
                ("OnTrigger", "!self", "Disable", "", 0.0, -1),
                ("OnTrigger", "!caller", "Enable", "", 0.0, -1),
                ("OnTrigger", "!activator", "Toggle", "", 0.0, -1),
            ],
        ),
        probe("starter"),
    ]);
    world.input_from("relay", "Trigger", None, "starter");
    assert_eq!(
        run(&mut world, 0.015),
        [
            "relay.Trigger()",
            "starter.Toggle()",
            "relay.Enable()",
            "relay.Disable()"
        ]
    );
}

#[test]
fn logic_auto_fires_new_game_then_map_spawn_two_tenths_after_spawn_and_removes_itself() {
    let mut world = World::spawn(&[
        ent(
            "logic_auto",
            &[("spawnflags", "1")],
            &[
                ("OnMapSpawn", "probe", "Enable", "", 0.0, -1),
                ("OnNewGame", "probe", "Disable", "", 0.0, -1),
            ],
        ),
        probe("probe"),
    ]);
    assert!(run(&mut world, 0.18).is_empty());
    assert_eq!(
        run(&mut world, 0.015),
        ["probe.Disable()", "probe.Enable()"]
    );
    assert_eq!(world.count("logic_auto"), 0);
}

#[test]
fn a_removed_entity_loses_its_name_at_once() {
    let mut world = relay_world("1");
    world.input("relay", "Trigger", None);
    world.input("relay", "Trigger", None);
    assert_eq!(
        run(&mut world, 0.6),
        [
            "relay.Trigger()",
            "relay.Trigger() unhandled",
            "probe.Enable()"
        ]
    );
}
