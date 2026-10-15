// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::Path;

use portal::bsp::load;
use portal::entity::io::Handle;
use portal::entity::{Env, Shape, World};
use portal::install::root;
use portal::pair::Vec3;

/// the input lines the original printed at developer 2 standing on the a_00 button from tick 144 to 387
const ORIGINAL: &str = "\
2.16 floor_button_trigger_player_a00 exit_door_left_a00.Open()
2.16 floor_button_trigger_player_a00 floor_button_trigger_box_a00.Disable()
2.16 floor_button_trigger_player_a00 floor_button_door_a00.Open()
2.16 floor_button_trigger_player_a00 exit_door_right_a00.Open()
2.16 floor_button_door_a00 relay_boxplacementsuccess.Enable()
2.16 floor_button_door_a00 button1_down.PlaySound()
2.16 floor_button_door_a00 texturetoggle_exit_doorstate_a00.SetTextureIndex(1)
3.16 floor_button_trigger_player_a00 exit_door_left_a00.Open()
3.16 floor_button_trigger_player_a00 floor_button_trigger_box_a00.Disable()
3.16 floor_button_trigger_player_a00 floor_button_door_a00.Open()
3.16 floor_button_trigger_player_a00 exit_door_right_a00.Open()
4.17 floor_button_trigger_player_a00 exit_door_left_a00.Open()
4.17 floor_button_trigger_player_a00 floor_button_trigger_box_a00.Disable()
4.17 floor_button_trigger_player_a00 floor_button_door_a00.Open()
4.17 floor_button_trigger_player_a00 exit_door_right_a00.Open()
5.17 floor_button_trigger_player_a00 exit_door_left_a00.Open()
5.17 floor_button_trigger_player_a00 floor_button_trigger_box_a00.Disable()
5.17 floor_button_trigger_player_a00 floor_button_door_a00.Open()
5.17 floor_button_trigger_player_a00 exit_door_right_a00.Open()
5.82 floor_button_trigger_player_a00 exit_door_left_a00.Close()
5.82 floor_button_trigger_player_a00 floor_button_trigger_box_a00.Enable()
5.82 floor_button_trigger_player_a00 floor_button_door_a00.Close()
5.82 floor_button_trigger_player_a00 exit_door_right_a00.Close()
5.82 floor_button_door_a00 relay_boxplacementsuccess.Disable()
5.82 floor_button_door_a00 button1_up.PlaySound()
5.82 floor_button_door_a00 texturetoggle_exit_doorstate_a00.SetTextureIndex(0)";

struct Standing {
    player: Handle,
    trigger: Handle,
    on: bool,
}

impl Env for Standing {
    fn push(&mut self, _pusher: Handle, _delta: Vec3) -> Option<Handle> {
        None
    }

    fn movers(&mut self) -> Vec<(Handle, Vec<Handle>)> {
        vec![(
            self.player,
            if self.on {
                vec![self.trigger]
            } else {
                Vec::new()
            },
        )]
    }
}

#[test]
fn standing_on_the_first_button_matches_the_originals_input_log() {
    let steam = root(
        None,
        None,
        Path::new(&std::env::var_os("HOME").unwrap_or_default()),
    );
    let map = load(&steam, "testchmb_a_00").unwrap_or_else(|e| panic!("{e}"));
    let mut world = World::spawn_with(&map.entities, |e| Shape {
        bounds: e
            .get("model")
            .and_then(|m| m.strip_prefix('*'))
            .and_then(|i| map.models.get(i.parse::<usize>().ok()?).copied()),
        ..Shape::default()
    });
    let player = world.add_player();
    let trigger = world.find("floor_button_trigger_player_a00").unwrap();
    let mut env = Standing {
        player,
        trigger,
        on: false,
    };
    let mut lines = Vec::new();
    for tick in 1..=400 {
        env.on = (144..=387).contains(&tick);
        world.step(&mut env);
        for f in world.drain_fired() {
            if matches!(
                f.caller.as_str(),
                "floor_button_trigger_player_a00" | "floor_button_door_a00"
            ) {
                lines.push(format!(
                    "{:.2} {} {}.{}({})",
                    f.time, f.caller, f.target, f.input, f.param
                ));
            }
        }
    }
    assert_eq!(lines.join("\n"), ORIGINAL);
}
