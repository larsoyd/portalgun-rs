// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::entity::World;
use portal::vmf::Entity;

fn ent(classname: &str, keys: &[(&str, &str)]) -> Entity {
    Entity {
        classname: classname.into(),
        keys: keys
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        outputs: Vec::new(),
        solids: Vec::new(),
    }
}

fn frames(world: &World, name: &str) -> Vec<u8> {
    world
        .all(name)
        .into_iter()
        .map(|h| world.frame(h))
        .collect()
}

#[test]
fn set_texture_index_sets_the_frame_on_every_entity_of_the_target_name() {
    let mut world = World::spawn(&[
        ent(
            "env_texturetoggle",
            &[("targetname", "toggle"), ("target", "lights")],
        ),
        ent("func_brush", &[("targetname", "lights")]),
        ent("info_overlay_accessor", &[("targetname", "lights")]),
        ent("func_brush", &[("targetname", "other")]),
    ]);
    world.input("toggle", "SetTextureIndex", Some("1"));
    world.tick();
    assert_eq!(frames(&world, "lights"), [1, 1]);
    assert_eq!(frames(&world, "other"), [0]);
    world.input("toggle", "SetTextureIndex", Some("257"));
    world.input("toggle", "IncrementTextureIndex", None);
    world.tick();
    assert_eq!(frames(&world, "lights"), [2, 2]);
    world.input("toggle", "SetTextureIndex", Some("lit"));
    world.tick();
    assert_eq!(frames(&world, "lights"), [0, 0]);
}

#[test]
fn a_one_shot_ambient_restarts_on_every_play_from_its_source() {
    let mut world = World::spawn(&[
        ent(
            "ambient_generic",
            &[
                ("targetname", "down"),
                ("message", "Portal.button_down"),
                ("spawnflags", "48"),
                ("SourceEntityName", "door"),
            ],
        ),
        ent("func_door", &[("targetname", "door")]),
    ]);
    let door = world.find("door").unwrap();
    world.input("down", "PlaySound", None);
    world.input("down", "PlaySound", None);
    world.input("down", "StopSound", None);
    world.tick();
    let sounds: Vec<_> = world
        .drain_sounds()
        .into_iter()
        .map(|s| (s.source, s.name, s.stop))
        .collect();
    let (stop, play) = (
        (door, "Portal.button_down".to_string(), true),
        (door, "Portal.button_down".to_string(), false),
    );
    assert_eq!(
        sounds,
        [
            stop.clone(),
            stop.clone(),
            play.clone(),
            stop.clone(),
            stop,
            play
        ]
    );
}

#[test]
fn a_looping_ambient_plays_once_and_stops_on_stop() {
    let mut world = World::spawn(&[ent(
        "ambient_generic",
        &[
            ("targetname", "hum"),
            ("message", "Portal.fluorescent_light_hum"),
            ("spawnflags", "16"),
        ],
    )]);
    let hum = world.find("hum").unwrap();
    world.input("hum", "PlaySound", None);
    world.input("hum", "PlaySound", None);
    world.input("hum", "StopSound", None);
    world.input("hum", "StopSound", None);
    world.tick();
    let sounds: Vec<_> = world
        .drain_sounds()
        .into_iter()
        .map(|s| (s.source == hum, s.stop))
        .collect();
    assert_eq!(sounds, [(true, true), (true, false), (true, true)]);
}
