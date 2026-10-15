// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use super::io::Handle;
use super::{atoi, Kind, Sequence, Think, World};

const THINK_GAP: f32 = 0.1;
const END: f32 = 0.999;
const MAX_STEP: f32 = 0.2;
const MIN_STEP: f32 = 0.001;
const NO_COLLISION: u32 = 0x100;

#[derive(Debug, Clone)]
pub(super) struct Dynamic {
    sequences: Vec<Sequence>,
    default: Option<String>,
    sequence: usize,
    goal: usize,
    cycle: f32,
    rate: f32,
    anim_time: f32,
    pub(super) skin: i32,
    pub(super) hidden: bool,
    pub(super) solid: bool,
}

impl Dynamic {
    pub(super) fn new(
        sequences: Vec<Sequence>,
        default: Option<&str>,
        hidden: bool,
        spawnflags: u32,
    ) -> Dynamic {
        Dynamic {
            sequences,
            default: default.filter(|d| !d.is_empty()).map(str::to_string),
            sequence: 0,
            goal: 0,
            cycle: 0.0,
            rate: 1.0,
            anim_time: 0.0,
            skin: 0,
            hidden,
            solid: spawnflags & NO_COLLISION == 0,
        }
    }

    pub(super) fn animation(&self) -> (usize, f32) {
        (self.sequence, self.cycle)
    }

    /// Translated from 0x473490 and 0x46eda0 (decompiled, server.so)
    fn advance(&mut self, now: f32) {
        let interval = (now - self.anim_time).clamp(0.0, MAX_STEP);
        if interval <= MIN_STEP {
            return;
        }
        self.anim_time = now;
        let Some(seq) = self.sequences.get(self.sequence) else {
            return;
        };
        let speed = if seq.duration > 0.0 {
            1.0 / seq.duration
        } else {
            10.0
        };
        self.cycle += interval * self.rate * speed;
        if !(0.0..1.0).contains(&self.cycle) {
            self.cycle = match (seq.looping, self.cycle < 0.0) {
                (true, _) => self.cycle - self.cycle.trunc(),
                (false, true) => 0.0,
                (false, false) => 1.0,
            };
        }
    }

    fn done(&self) -> bool {
        let looping = self.sequences.get(self.sequence).is_some_and(|s| s.looping);
        let end = (self.rate > 0.0 && self.cycle >= END) || (self.rate < 0.0 && self.cycle <= 0.0);
        end && !looping
    }
}

fn dynamic(world: &mut World, h: Handle) -> &mut Dynamic {
    match &mut world.slot_mut(h).kind {
        Kind::Dynamic(d) => d,
        _ => unreachable!("prop_dynamic call on another class"),
    }
}

/// Translated from 0x9441a0 and 0x944250 (decompiled, server.so)
pub(super) fn set_animation(world: &mut World, h: Handle, name: &str) {
    let d = dynamic(world, h);
    let Some(seq) = d
        .sequences
        .iter()
        .position(|s| s.name.eq_ignore_ascii_case(name))
    else {
        d.sequence = 0;
        return;
    };
    set_sequence(world, h, seq);
    world.fire(h, "onanimationbegun", None, 0.0);
}

/// Translated from 0x9440c0 and 0x943a20 (decompiled, server.so), no model here has a sequence graph
fn set_sequence(world: &mut World, h: Handle, seq: usize) {
    let now = world.now();
    let d = dynamic(world, h);
    d.goal = seq;
    d.sequence = seq;
    d.cycle = 0.0;
    d.rate = 1.0;
    d.anim_time = now;
    if world.next_think(h) <= now {
        world.set_think(h, now + THINK_GAP, Think::Anim);
    }
}

/// Translated from 0x9442b0 (disassembled, server.so)
pub(super) fn think(world: &mut World, h: Handle) {
    let now = world.now();
    let d = dynamic(world, h);
    if !d.done() {
        world.set_think(h, now + THINK_GAP, Think::Anim);
    } else {
        world.fire(h, "onanimationdone", None, 0.0);
        if let Some(default) = dynamic(world, h).default.clone() {
            set_animation(world, h, &default);
        }
    }
    dynamic(world, h).advance(now);
}

/// Translated from 0x938960, 0x93bcf0, 0x938b00, 0x938b40 and 0x9383f0 (decompiled, server.so)
pub(super) fn input(world: &mut World, h: Handle, input: &str, value: Option<&str>) -> bool {
    match input {
        "setanimation" => set_animation(world, h, value.unwrap_or_default()),
        "setdefaultanimation" => dynamic(world, h).default = value.map(str::to_string),
        "skin" => dynamic(world, h).skin = value.map_or(0, atoi),
        "enable" | "turnon" => dynamic(world, h).hidden = false,
        "disable" | "turnoff" => dynamic(world, h).hidden = true,
        "enablecollision" => dynamic(world, h).solid = true,
        "disablecollision" => dynamic(world, h).solid = false,
        _ => return false,
    }
    true
}
