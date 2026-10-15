// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use super::io::Handle;
use super::{Kind, World};

pub(super) const AUTO_DELAY: f32 = 0.2;
const REFIRE_SLACK: f64 = 0.001;
const ONLY_ONCE: u32 = 1;
const FAST_RETRIGGER: u32 = 2;
const AUTO_REMOVE: u32 = 1;

#[derive(Debug, Clone)]
pub(super) struct Relay {
    disabled: bool,
    waiting: bool,
}

impl Relay {
    pub(super) fn new(disabled: bool) -> Relay {
        Relay {
            disabled,
            waiting: false,
        }
    }
}

fn relay(world: &mut World, h: Handle) -> &mut Relay {
    match &mut world.slot_mut(h).kind {
        Kind::Relay(r) => r,
        _ => unreachable!("relay input on another class"),
    }
}

/// Translated from 0x8424e0 to 0x842630 (decompiled, server.so)
pub(super) fn relay_input(
    world: &mut World,
    h: Handle,
    input: &str,
    activator: Option<Handle>,
) -> bool {
    match input {
        "enable" => relay(world, h).disabled = false,
        "disable" => relay(world, h).disabled = true,
        "toggle" => relay(world, h).disabled ^= true,
        "enablerefire" => relay(world, h).waiting = false,
        "cancelpending" => {
            world.cancel(h);
            relay(world, h).waiting = false;
        }
        "trigger" => trigger(world, h, activator),
        _ => return false,
    }
    true
}

fn trigger(world: &mut World, h: Handle, activator: Option<Handle>) {
    let r = relay(world, h);
    if r.disabled || r.waiting {
        return;
    }
    world.fire(h, "ontrigger", activator, 0.0);
    let flags = world.slot(h).spawnflags;
    if flags & ONLY_ONCE != 0 {
        world.remove(h);
    } else if flags & FAST_RETRIGGER == 0 {
        relay(world, h).waiting = true;
        let delay = (world.max_delay(h, "ontrigger") as f64 + REFIRE_SLACK) as f32;
        world.queue_self(h, "EnableRefire", delay);
    }
}

#[derive(Debug, Clone)]
pub(super) struct Auto {
    globalstate: Option<String>,
}

impl Auto {
    pub(super) fn new(globalstate: Option<&str>) -> Auto {
        Auto {
            globalstate: globalstate.filter(|g| !g.is_empty()).map(str::to_string),
        }
    }

    /// Translated from 0x83aa10 (decompiled, server.so), a fresh map is load type 0, new game
    pub(super) fn think(&self, world: &mut World, h: Handle) {
        if self.globalstate.is_some() {
            return;
        }
        world.fire(h, "onnewgame", None, 0.0);
        world.fire(h, "onmapspawn", None, 0.0);
        if world.slot(h).spawnflags & AUTO_REMOVE != 0 {
            world.remove(h);
        }
    }
}
