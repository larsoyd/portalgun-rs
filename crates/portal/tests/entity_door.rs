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

const EVENTS: [(&str, &str); 6] = [
    ("OnOpen", "open"),
    ("OnClose", "close"),
    ("OnFullyOpen", "fully_open"),
    ("OnFullyClosed", "fully_closed"),
    ("OnBlockedClosing", "blocked"),
    ("OnUnblockedClosing", "unblocked"),
];

fn door(keys: &[(&str, &str)], size: Vec3) -> World {
    let mut keys = keys.to_vec();
    keys.push(("targetname", "door"));
    let mut ents = vec![ent("func_door", &keys, &EVENTS)];
    ents.extend(
        EVENTS
            .iter()
            .map(|(_, n)| ent("logic_relay", &[("targetname", n)], &[])),
    );
    World::spawn_with(&ents, |e| {
        (e.classname == "func_door").then_some(([0.0; 3], size))
    })
}

fn button() -> World {
    door(
        &[
            ("speed", "25"),
            ("movedir", "90 0 0"),
            ("lip", "0"),
            ("wait", "-1"),
            ("origin", "0 0 100"),
        ],
        [58.0, 58.0, 8.0],
    )
}

#[derive(Default)]
struct Wall {
    blocking: Option<Handle>,
    pushes: Vec<Vec3>,
}

impl Env for Wall {
    fn push(&mut self, _pusher: Handle, delta: Vec3) -> Option<Handle> {
        self.pushes.push(delta);
        self.blocking
    }
}

fn outputs(world: &mut World) -> Vec<String> {
    world
        .drain_fired()
        .into_iter()
        .filter(|f| f.caller == "door")
        .map(|f| f.target)
        .collect()
}

fn z(world: &World) -> f32 {
    world.origin(world.find("door").unwrap())[2]
}

#[test]
fn the_button_door_sinks_six_units_at_twenty_five_and_lands_exactly() {
    let mut world = button();
    let mut env = Wall::default();
    world.input("door", "Open", None);
    world.step(&mut env);
    assert_eq!(outputs(&mut world), ["open"]);
    let mut landed = None;
    for tick in 2..40 {
        world.step(&mut env);
        if outputs(&mut world).contains(&"fully_open".to_string()) {
            landed = Some(tick);
            break;
        }
    }
    assert_eq!(landed, Some(17));
    assert_eq!(z(&world), 94.0);
    assert!(
        (env.pushes[1][2] + 0.375).abs() < 1e-5,
        "{:?}",
        env.pushes[1]
    );
}

#[test]
fn open_is_ignored_while_opening_and_close_reruns_while_closing() {
    let mut world = button();
    let mut env = Wall::default();
    world.input("door", "Open", None);
    world.input("door", "Open", None);
    for _ in 0..8 {
        world.step(&mut env);
    }
    assert_eq!(outputs(&mut world), ["open"]);
    world.input("door", "Close", None);
    world.step(&mut env);
    world.input("door", "Close", None);
    world.step(&mut env);
    assert_eq!(outputs(&mut world), ["close", "close"]);
    for _ in 0..10 {
        world.step(&mut env);
    }
    assert_eq!(outputs(&mut world), ["fully_closed"]);
    assert_eq!(z(&world), 100.0);
    world.input("door", "Close", None);
    world.step(&mut env);
    assert!(outputs(&mut world).is_empty());
}

#[test]
fn a_start_open_door_spawns_at_its_far_end_and_swaps_its_full_outputs() {
    let mut world = door(
        &[
            ("speed", "250"),
            ("movedir", "0 90 0"),
            ("lip", "12"),
            ("wait", "-1"),
            ("spawnflags", "1"),
        ],
        [8.0, 56.0, 104.0],
    );
    let h = world.find("door").unwrap();
    let at = world.origin(h);
    assert!(
        at[0].abs() < 1e-5 && at[1] == 42.0 && at[2] == 0.0,
        "{at:?}"
    );
    let mut env = Wall::default();
    world.input("door", "Close", None);
    for _ in 0..20 {
        world.step(&mut env);
    }
    assert_eq!(outputs(&mut world), ["close", "fully_open"]);
    assert_eq!(world.origin(h), [0.0; 3]);
}

#[test]
fn a_blocked_door_holds_still_and_reports_the_blocker_once() {
    let mut world = button();
    let mut env = Wall::default();
    world.input("door", "Open", None);
    world.step(&mut env);
    world.step(&mut env);
    world.input("door", "Close", None);
    world.step(&mut env);
    let before = z(&world);
    env.blocking = world.find("open");
    for _ in 0..5 {
        world.step(&mut env);
    }
    assert_eq!(z(&world), before);
    env.blocking = None;
    for _ in 0..30 {
        world.step(&mut env);
    }
    assert_eq!(
        outputs(&mut world),
        ["open", "close", "blocked", "unblocked", "fully_closed"]
    );
    assert_eq!(z(&world), 100.0);
}

#[test]
fn a_door_plays_its_arrival_sound_at_both_ends() {
    let mut world = door(
        &[
            ("speed", "250"),
            ("movedir", "0 270 0"),
            ("lip", "12"),
            ("wait", "-1"),
            ("noise2", "Doors.FullClose8"),
        ],
        [8.0, 56.0, 104.0],
    );
    let mut env = Wall::default();
    world.input("door", "Open", None);
    for _ in 0..20 {
        world.step(&mut env);
    }
    world.input("door", "Close", None);
    for _ in 0..20 {
        world.step(&mut env);
    }
    let sounds: Vec<String> = world.drain_sounds().into_iter().map(|s| s.name).collect();
    assert_eq!(sounds, ["Doors.FullClose8", "Doors.FullClose8"]);
}
