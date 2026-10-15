// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::Path;

use portal::install::{root, Install};
use portal::placement::World;
use portalgun::chamber::Chamber;

fn search() -> portal::vpk::SearchPath {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    Install::open(&dir)
        .unwrap_or_else(|e| panic!("install at {dir:?}: {e}"))
        .search
}

fn panel(x: f32, material: &str) -> String {
    let (a, b) = (x, x + 4.0);
    let side = |p: &str| {
        format!(
            "side {{ \"plane\" \"{p}\" \"material\" \"{material}\" \
             \"uaxis\" \"[0 1 0 0] 0.25\" \"vaxis\" \"[0 0 -1 0] 0.25\" }}"
        )
    };
    format!(
        "solid {{ \"id\" \"1\" {} {} {} {} {} {} }}",
        side(&format!("({a} 64 128) ({b} 64 128) ({b} -64 128)")),
        side(&format!("({a} -64 0) ({b} -64 0) ({b} 64 0)")),
        side(&format!("({a} 64 128) ({a} -64 128) ({a} -64 0)")),
        side(&format!("({b} 64 0) ({b} -64 0) ({b} -64 128)")),
        side(&format!("({b} 64 128) ({a} 64 128) ({a} 64 0)")),
        side(&format!("({b} -64 0) ({a} -64 0) ({a} -64 128)")),
    )
}

fn map() -> String {
    format!(
        "world {{ {} }} \
         entity {{ \"classname\" \"func_door\" \"targetname\" \"door\" {} }} \
         entity {{ \"classname\" \"trigger_multiple\" \"targetname\" \"pad\" {} }} \
         entity {{ \"classname\" \"info_player_start\" \"origin\" \"1 2 3\" }}",
        panel(0.0, "concrete/concrete_modular_wall001a"),
        panel(100.0, "metal/metalwall048b"),
        panel(200.0, "tools/toolstrigger"),
    )
}

#[test]
fn door_brushes_collide_as_owned_blocks_and_triggers_become_volumes() {
    let c = Chamber::from_vmf(&search(), &map()).unwrap();
    let owners: Vec<_> = c.blocks.iter().map(|b| b.owner).collect();
    assert_eq!(owners, [None, Some(1)]);
    assert_eq!(c.volumes.len(), 1);
    assert_eq!(
        (c.volumes[0].owner, c.volumes[0].min, c.volumes[0].max),
        (2, [200.0, -64.0, 0.0], [204.0, 64.0, 128.0])
    );
    assert_eq!(c.entities.len(), 4);
    assert!(
        c.cast([150.0, 0.0, 64.0], [1.0, 0.0, 0.0], 1000.0)
            .is_none(),
        "a trigger stops no shot"
    );
}

#[test]
fn moving_an_owner_carries_its_planes_faces_and_bounds() {
    let mut c = Chamber::from_vmf(&search(), &map()).unwrap();
    c.move_owner(1, [0.0, 0.0, 10.0]);
    let door = &c.blocks[1];
    assert_eq!(
        (door.min, door.max),
        ([100.0, -64.0, 10.0], [104.0, 64.0, 138.0])
    );
    assert!(door.faces().all(|s| s.points.iter().all(|p| p[2] >= 10.0)));
    let hit = c.cast([150.0, 0.0, 5.0], [-1.0, 0.0, 0.0], 1000.0).unwrap();
    assert_eq!(
        hit.distance, 146.0,
        "the ray now passes under the door to the wall"
    );
}

#[test]
fn brush_entities_the_port_does_not_run_are_refused() {
    let text = map().replace("func_door", "func_illusionary");
    let err = Chamber::from_vmf(&search(), &text)
        .err()
        .unwrap()
        .to_string();
    assert!(
        err.contains("refused") && err.contains("func_illusionary"),
        "{err}"
    );
}

#[test]
fn chambers_are_picked_by_name_and_unknown_names_are_refused() {
    assert!(Chamber::named(&search(), "sandbox").is_ok());
    let err = Chamber::named(&search(), "nope").err().unwrap().to_string();
    assert!(err.contains("refused") && err.contains("nope"), "{err}");
}

#[test]
fn the_entities_chamber_loads_with_doors_triggers_and_a_closed_room() {
    let c = Chamber::named(&search(), "entities").unwrap();
    let owners: Vec<&str> = c
        .blocks
        .iter()
        .filter_map(|b| b.owner)
        .map(|o| c.entities[o].get("targetname").unwrap())
        .collect();
    assert_eq!(owners, ["button_door", "exit_door_right", "exit_door_left"]);
    let triggers: Vec<&str> = c
        .volumes
        .iter()
        .map(|v| c.entities[v.owner].get("targetname").unwrap())
        .collect();
    assert_eq!(
        triggers,
        ["button_trigger_box", "button_trigger_player", "fryer"]
    );
    for b in c.blocks.iter().filter(|b| b.owner.is_none()) {
        assert!(b.sides.iter().all(|s| s.points.len() == 4), "{:?}", b.min);
    }
    let down = c
        .cast([-384.0, 0.0, 64.0], [0.0, 0.0, -1.0], 1000.0)
        .unwrap();
    assert_eq!(down.distance, 64.0);
}

#[test]
fn a_box_meets_a_trigger_only_when_it_reaches_inside() {
    let c = Chamber::from_vmf(&search(), &map()).unwrap();
    let pad = &c.volumes[0].planes;
    let half = [16.0, 16.0, 36.0];
    assert!(portalgun::chamber::box_meets(pad, [190.0, 0.0, 64.0], half));
    assert!(
        !portalgun::chamber::box_meets(pad, [184.0, 0.0, 64.0], half),
        "flush with the face is outside"
    );
    assert!(!portalgun::chamber::box_meets(
        pad,
        [150.0, 0.0, 64.0],
        half
    ));
    assert!(
        !portalgun::chamber::box_meets(pad, [190.0, 0.0, 164.0], half),
        "above the top"
    );
}
