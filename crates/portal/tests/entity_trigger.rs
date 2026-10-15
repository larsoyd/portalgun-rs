// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::{Path, PathBuf};

use portal::bsp::open;
use portal::entity::World;
use portal::install::root;
use portal::vmf::{Entity, Output};

fn ent(classname: &str, keys: &[(&str, &str)], outputs: &[(&str, &str, &str, f32)]) -> Entity {
    Entity {
        classname: classname.into(),
        keys: keys
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        outputs: outputs
            .iter()
            .map(|&(event, target, input, delay)| Output {
                event: event.into(),
                target: target.into(),
                input: input.into(),
                param: String::new(),
                delay,
                times: -1,
            })
            .collect(),
        solids: Vec::new(),
    }
}

fn probe(name: &str) -> Entity {
    ent("logic_relay", &[("targetname", name)], &[])
}

fn ticks(world: &mut World, seconds: f32) -> Vec<String> {
    let mut all = Vec::new();
    for _ in 0..(seconds / World::TICK).round() as u32 {
        world.tick();
        all.extend(world.drain_fired().into_iter().map(|f| f.to_string()));
    }
    all
}

fn watched(classname: &str, keys: &[(&str, &str)]) -> Vec<Entity> {
    let outputs = [
        ("OnStartTouch", "start", "Enable", 0.0),
        ("OnStartTouchAll", "all", "Enable", 0.0),
        ("OnEndTouch", "end", "Enable", 0.0),
        ("OnEndTouchAll", "none", "Enable", 0.0),
        ("OnTrigger", "fire", "Enable", 0.0),
    ];
    let mut keys = keys.to_vec();
    keys.push(("targetname", "t"));
    vec![
        ent(classname, &keys, &outputs),
        ent(
            "filter_activator_name",
            &[
                ("targetname", "only_box"),
                ("filtername", "box"),
                ("Negated", "Allow"),
            ],
            &[],
        ),
        ent(
            "filter_activator_name",
            &[
                ("targetname", "not_box"),
                ("filtername", "box"),
                ("Negated", "1"),
            ],
            &[],
        ),
        ent("prop_physics", &[("targetname", "box")], &[]),
        ent("prop_physics", &[("targetname", "other")], &[]),
        probe("start"),
        probe("all"),
        probe("end"),
        probe("none"),
        probe("fire"),
    ]
}

fn names(fired: &[String]) -> Vec<&str> {
    fired.iter().map(|f| f.split('.').next().unwrap()).collect()
}

#[test]
fn physics_triggers_take_props_through_their_filter_and_ignore_the_player() {
    let mut world = World::spawn(&watched(
        "trigger_multiple",
        &[("spawnflags", "8"), ("filtername", "only_box")],
    ));
    let player = world.add_player();
    let (t, b, o) = (
        world.find("t").unwrap(),
        world.find("box").unwrap(),
        world.find("other").unwrap(),
    );
    world.start_touch(t, player);
    world.start_touch(t, o);
    assert!(ticks(&mut world, 0.015).is_empty());
    world.start_touch(t, b);
    world.touch(t, b);
    assert_eq!(names(&ticks(&mut world, 0.015)), ["start", "all", "fire"]);
}

#[test]
fn a_negated_filter_turns_the_name_test_around() {
    let mut world = World::spawn(&watched(
        "trigger_multiple",
        &[("spawnflags", "8"), ("filtername", "not_box")],
    ));
    let (t, b, o) = (
        world.find("t").unwrap(),
        world.find("box").unwrap(),
        world.find("other").unwrap(),
    );
    world.start_touch(t, b);
    assert!(ticks(&mut world, 0.015).is_empty());
    world.start_touch(t, o);
    assert_eq!(names(&ticks(&mut world, 0.015)), ["start", "all"]);
}

#[test]
fn client_triggers_take_the_player_only() {
    let mut world = World::spawn(&watched("trigger_multiple", &[("spawnflags", "1")]));
    let player = world.add_player();
    let (t, b) = (world.find("t").unwrap(), world.find("box").unwrap());
    world.start_touch(t, b);
    assert!(ticks(&mut world, 0.015).is_empty());
    world.start_touch(t, player);
    assert_eq!(names(&ticks(&mut world, 0.015)), ["start", "all"]);
}

#[test]
fn a_multiple_trigger_refires_every_wait_while_something_stays_inside() {
    let mut world = World::spawn(&watched(
        "trigger_multiple",
        &[("spawnflags", "64"), ("wait", "1")],
    ));
    let (t, b) = (world.find("t").unwrap(), world.find("box").unwrap());
    let mut fires = Vec::new();
    for i in 0..140 {
        world.touch(t, b);
        if ticks(&mut world, 0.015)
            .iter()
            .any(|f| f.starts_with("fire."))
        {
            fires.push(i);
        }
    }
    assert_eq!(fires, [0, 67, 134]);
}

#[test]
fn a_zero_wait_means_a_fifth_of_a_second() {
    let mut world = World::spawn(&watched("trigger_multiple", &[("spawnflags", "64")]));
    let (t, b) = (world.find("t").unwrap(), world.find("box").unwrap());
    let mut fires = Vec::new();
    for i in 0..30 {
        world.touch(t, b);
        if ticks(&mut world, 0.015)
            .iter()
            .any(|f| f.starts_with("fire."))
        {
            fires.push(i);
        }
    }
    assert_eq!(fires, [0, 13, 26]);
}

#[test]
fn a_once_trigger_fires_one_time_and_is_gone_a_tenth_later() {
    let mut world = World::spawn(&watched("trigger_once", &[("spawnflags", "64")]));
    let (t, b) = (world.find("t").unwrap(), world.find("box").unwrap());
    let mut fires = 0;
    for _ in 0..20 {
        if world.find("t").is_some() {
            world.touch(t, b);
        }
        fires += ticks(&mut world, 0.015)
            .iter()
            .filter(|f| f.starts_with("fire."))
            .count();
    }
    assert_eq!(fires, 1);
    assert!(world.find("t").is_none());
}

#[test]
fn touch_all_outputs_fire_on_the_first_in_and_the_last_out() {
    let mut world = World::spawn(&watched("trigger_multiple", &[("spawnflags", "64")]));
    let (t, b, o) = (
        world.find("t").unwrap(),
        world.find("box").unwrap(),
        world.find("other").unwrap(),
    );
    world.start_touch(t, b);
    world.start_touch(t, o);
    world.start_touch(t, b);
    assert_eq!(
        names(&ticks(&mut world, 0.015)),
        ["start", "all", "start", "start"]
    );
    world.end_touch(t, b);
    assert_eq!(names(&ticks(&mut world, 0.015)), ["end"]);
    world.end_touch(t, b);
    assert!(ticks(&mut world, 0.015).is_empty());
    world.end_touch(t, o);
    assert_eq!(names(&ticks(&mut world, 0.015)), ["end", "none"]);
}

#[test]
fn a_disabled_trigger_ignores_touches_until_enabled() {
    let mut world = World::spawn(&watched(
        "trigger_multiple",
        &[("spawnflags", "64"), ("StartDisabled", "1")],
    ));
    let (t, b) = (world.find("t").unwrap(), world.find("box").unwrap());
    assert!(!world.trigger_enabled(t));
    world.input("t", "Enable", None);
    ticks(&mut world, 0.015);
    assert!(world.trigger_enabled(t));
    world.start_touch(t, b);
    assert_eq!(names(&ticks(&mut world, 0.015)), ["start", "all"]);
    world.input("t", "Disable", None);
    ticks(&mut world, 0.015);
    assert!(!world.trigger_enabled(t));
}

fn steam() -> PathBuf {
    root(
        None,
        None,
        Path::new(&std::env::var_os("HOME").unwrap_or_default()),
    )
}

#[test]
fn a_box_on_the_first_chambers_button_opens_both_exit_doors_and_the_button() {
    let ents = open(&steam(), "testchmb_a_00").unwrap_or_else(|e| panic!("{e}"));
    let mut world = World::spawn(&ents);
    world.add_player();
    let trigger = world.find("floor_button_trigger_box_a00").unwrap();
    let box_ = world.find("box").unwrap();
    world.start_touch(trigger, box_);
    world.touch(trigger, box_);
    world.tick();
    let fired: Vec<String> = world
        .drain_fired()
        .into_iter()
        .filter(|f| f.caller == "floor_button_trigger_box_a00")
        .map(|f| format!("{}.{}", f.target, f.input))
        .collect();
    assert_eq!(
        fired,
        [
            "logic_no_play_cleanser_dialog.Enable",
            "soundscape_cleanser.Enable",
            "soundscape_box_training.Disable",
            "logic_play_cleanser_dialog.Disable",
            "exit_door_left_a00.Open",
            "floor_button_trigger_player_a00.Disable",
            "floor_button_door_a00.Open",
            "exit_door_right_a00.Open",
        ]
    );
}
