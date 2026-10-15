// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::{Path, PathBuf};

use portal::bsp::open;
use portal::entity::World;
use portal::install::root;
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

fn steam() -> PathBuf {
    root(
        None,
        None,
        Path::new(&std::env::var_os("HOME").unwrap_or_default()),
    )
}

fn near(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-3)
}

#[test]
fn the_first_chambers_maker_drops_a_named_box_below_it_and_keeps_the_map_box() {
    let ents = open(&steam(), "testchmb_a_00").unwrap_or_else(|e| panic!("{e}"));
    let mut world = World::spawn(&ents);
    assert_eq!(world.all("box").len(), 1);
    world.input("entity_box_maker_rm1", "ForceSpawn", None);
    world.tick();
    let spawned = world.drain_spawned();
    assert_eq!(spawned.len(), 1);
    let copy = world.entity(spawned[0]);
    assert_eq!(
        (copy.classname.as_str(), copy.get("targetname")),
        ("prop_physics", Some("box"))
    );
    assert!(
        near(world.pose(spawned[0]).0, [-859.301, -1025.3, 501.327]),
        "{:?}",
        world.pose(spawned[0])
    );
    assert!(
        near(world.pose(spawned[0]).1, [-0.336265, 0.488076, -0.912995]),
        "{:?}",
        world.pose(spawned[0])
    );
    assert_eq!(copy.outputs.len(), 6);
    assert_eq!(world.all("box").len(), 2);
}

fn spawner(template_flags: &str, maker_angles: &str) -> World {
    World::spawn(&[
        ent(
            "prop_physics",
            &[
                ("targetname", "cube"),
                ("origin", "10 0 0"),
                ("angles", "0 0 0"),
            ],
            &[],
        ),
        ent(
            "point_template",
            &[
                ("targetname", "tpl"),
                ("Template01", "cube"),
                ("spawnflags", template_flags),
                ("origin", "0 0 0"),
            ],
            &[],
        ),
        ent(
            "env_entity_maker",
            &[
                ("targetname", "maker"),
                ("EntityTemplate", "tpl"),
                ("origin", "100 0 50"),
                ("angles", maker_angles),
            ],
            &[("OnEntitySpawned", "spawned")],
        ),
        ent("logic_relay", &[("targetname", "spawned")], &[]),
    ])
}

#[test]
fn a_turned_maker_turns_the_template_offset_and_the_copy() {
    let mut world = spawner("3", "0 90 0");
    world.input("maker", "ForceSpawn", None);
    world.tick();
    let h = world.drain_spawned()[0];
    let (origin, angles) = world.pose(h);
    assert!(near(origin, [100.0, 10.0, 50.0]), "{origin:?}");
    assert!(near(angles, [0.0, 90.0, 0.0]), "{angles:?}");
    let fired: Vec<String> = world
        .drain_fired()
        .into_iter()
        .map(|f| format!("{}<{}", f.target, f.caller))
        .collect();
    assert!(fired.contains(&"spawned<maker".to_string()), "{fired:?}");
}

#[test]
fn a_template_without_the_keep_flag_takes_its_entity_out_of_the_map() {
    let world = spawner("2", "0 0 0");
    assert!(world.find("cube").is_none());
}

#[test]
fn a_template_that_does_not_preserve_names_is_refused() {
    let world = spawner("1", "0 0 0");
    assert!(
        world.refusals().iter().any(|r| r.contains("tpl")),
        "{:?}",
        world.refusals()
    );
}

fn props() -> World {
    World::spawn(&[
        ent(
            "prop_physics",
            &[("targetname", "box"), ("spawnflags", "256")],
            &[
                ("OnOutOfWorld", "out"),
                ("OnPlayerUse", "used"),
                ("OnPhysGunPickup", "gun_pickup"),
                ("OnPlayerPickup", "picked"),
                ("OnPhysGunOnlyPickup", "gun_only"),
                ("OnPhysGunDrop", "dropped"),
            ],
        ),
        ent("logic_relay", &[("targetname", "x")], &[]),
    ])
}

fn targets(world: &mut World) -> Vec<String> {
    world.tick();
    world.drain_fired().into_iter().map(|f| f.target).collect()
}

#[test]
fn a_prop_is_out_of_the_world_at_sixteen_thousand_units_or_two_thousand_a_second() {
    let mut world = props();
    let b = world.find("box").unwrap();
    world.prop_update(b, [16383.9, 0.0, -16383.9], [1999.9, -1999.9, 0.0]);
    assert!(targets(&mut world).is_empty());
    world.prop_update(b, [0.0, 16384.0, 0.0], [0.0; 3]);
    assert_eq!(targets(&mut world), ["out"]);
    world.prop_update(b, [0.0; 3], [0.0, 0.0, -2000.0]);
    assert_eq!(targets(&mut world), ["out"]);
}

#[test]
fn use_pickup_and_drop_fire_the_props_player_outputs() {
    let mut world = props();
    let player = world.add_player();
    let b = world.find("box").unwrap();
    world.player_use(b, player);
    assert_eq!(targets(&mut world), ["used"]);
    world.picked_up(b, player);
    assert_eq!(targets(&mut world), ["gun_pickup", "picked"]);
    world.dropped(b, player);
    assert_eq!(targets(&mut world), ["dropped"]);
}

#[test]
fn a_maker_pointing_straight_up_keeps_its_yaw_and_drops_the_roll() {
    let mut world = spawner("3", "-90 30 0");
    world.input("maker", "ForceSpawn", None);
    world.tick();
    let (origin, angles) = world.pose(world.drain_spawned()[0]);
    assert!(near(origin, [100.0, 0.0, 60.0]), "{origin:?}");
    assert!(near(angles, [-90.0, 30.0, 0.0]), "{angles:?}");
}
