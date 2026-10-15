// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use super::io::{name_matches, Handle};
use super::{Body, Env, Kind, Think, World};

const CLIENTS: u32 = 0x1;
const NPCS: u32 = 0x2;
const PHYSICS: u32 = 0x8;
const ONLY_IN_VEHICLES: u32 = 0x20;
const EVERYTHING: u32 = 0x40;
const DEFAULT_WAIT: f32 = 0.2;
const REMOVE_DELAY: f32 = 0.1;

#[derive(Debug, Clone)]
pub(super) struct Trigger {
    pub(super) disabled: bool,
    multi: bool,
    wait: f32,
    filter_name: Option<String>,
    filter: Option<Handle>,
    touching: Vec<Handle>,
}

impl Trigger {
    /// Translated from 0x9c79a0 and 0x9c7a00 (decompiled, server.so)
    pub(super) fn new(
        once: bool,
        multi: bool,
        disabled: bool,
        wait: f32,
        filter: Option<&str>,
    ) -> Trigger {
        let wait = match (once, wait) {
            (true, _) => -1.0,
            (false, 0.0) => DEFAULT_WAIT,
            (false, w) => w,
        };
        Trigger {
            disabled,
            multi,
            wait,
            filter_name: filter.filter(|f| !f.is_empty()).map(str::to_string),
            filter: None,
            touching: Vec::new(),
        }
    }

    pub(super) fn bind_filter(&mut self, world: &World) {
        self.filter = self.filter_name.as_deref().and_then(|n| world.find(n));
    }
}

/// Translated from 0x9c68a0 (decompiled, server.so)
pub(super) fn spawnflags(flags: u32) -> u32 {
    let mut flags = flags;
    if flags & 0x810 != 0 {
        flags |= NPCS;
    }
    if flags & ONLY_IN_VEHICLES != 0 || flags & 0x200 != 0 {
        flags |= CLIENTS;
    }
    flags
}

#[derive(Debug, Clone)]
pub(super) struct Filter {
    negated: bool,
    by_class: bool,
    name: String,
}

impl Filter {
    pub(super) fn new(negated: bool, by_class: bool, name: &str) -> Filter {
        Filter {
            negated,
            by_class,
            name: name.to_string(),
        }
    }

    /// Translated from 0x55b1d0, 0x55c000 and 0x55bf50 (decompiled, server.so)
    fn passes(&self, world: &World, other: Handle) -> bool {
        let o = world.slot(other);
        let pass = if self.by_class {
            name_matches(Some(&o.classname), &self.name)
        } else if self.name.eq_ignore_ascii_case("!player") {
            o.body == Body::Player
        } else {
            name_matches(o.name.as_deref(), &self.name)
        };
        pass != self.negated
    }
}

fn trigger(world: &mut World, h: Handle) -> &mut Trigger {
    match &mut world.slot_mut(h).kind {
        Kind::Trigger(t) => t,
        _ => unreachable!("trigger call on another class"),
    }
}

/// Translated from 0x9c6d30 (decompiled, server.so), no NPC or vehicle activators exist here
fn passes(world: &World, h: Handle, other: Handle) -> bool {
    let flags = world.slot(h).spawnflags;
    let body = world.slot(other).body;
    let class = flags & EVERYTHING != 0
        || (flags & CLIENTS != 0 && body == Body::Player)
        || (flags & PHYSICS != 0 && body == Body::Physics);
    if !class || (body == Body::Player && flags & ONLY_IN_VEHICLES != 0) {
        return false;
    }
    let Kind::Trigger(t) = &world.slot(h).kind else {
        return false;
    };
    match t.filter.map(|f| &world.slot(f).kind) {
        Some(Kind::Filter(f)) => f.passes(world, other),
        _ => true,
    }
}

/// Translated from 0x9ce5c0 (decompiled, server.so)
pub(super) fn start_touch(world: &mut World, h: Handle, other: Handle) {
    if !passes(world, h, other) {
        return;
    }
    let t = trigger(world, h);
    let first = if t.touching.contains(&other) {
        false
    } else {
        t.touching.push(other);
        t.touching.len() == 1
    };
    world.fire(h, "onstarttouch", Some(other), 0.0);
    if first {
        world.fire(h, "onstarttouchall", Some(other), 0.0);
    }
}

/// Translated from 0x9ce290 (decompiled, server.so)
pub(super) fn end_touch(world: &mut World, h: Handle, other: Handle) {
    let t = trigger(world, h);
    let Some(at) = t.touching.iter().position(|&o| o == other) else {
        return;
    };
    t.touching.remove(at);
    world.fire(h, "onendtouch", Some(other), 0.0);
    let live: Vec<Handle> = trigger(world, h).touching.clone();
    let alive: Vec<Handle> = live.into_iter().filter(|&o| !world.slot(o).gone).collect();
    let none_left = alive.is_empty();
    trigger(world, h).touching = alive;
    if none_left {
        world.fire(h, "onendtouchall", Some(other), 0.0);
    }
}

pub(super) fn touch(world: &mut World, h: Handle, other: Handle, env: &mut impl Env) {
    if world.slot(h).classname == "trigger_portal_cleanser" {
        cleanse(world, h, other, env);
    } else {
        multi_touch(world, h, other);
    }
}

const UNCLEANSED: [&str; 7] = [
    "func_door",
    "func_door_rotating",
    "prop_door_rotating",
    "func_tracktrain",
    "env_ghostanimating",
    "physicsshadowclone",
    "prop_energy_ball",
];

/// Translated from 0x932a00 (disassembled, server.so)
fn cleanse(world: &mut World, h: Handle, other: Handle, env: &mut impl Env) {
    if !passes(world, h, other) {
        return;
    }
    if world.slot(other).body == Body::Player {
        if env.fizzle(other) {
            world.fire(h, "onfizzle", Some(other), 0.0);
        }
        return;
    }
    let o = world.slot(other);
    let animating = o.body == Body::Physics || o.classname.starts_with("prop_");
    if !animating
        || o.dissolving
        || UNCLEANSED
            .iter()
            .any(|c| name_matches(Some(&o.classname), c))
    {
        return;
    }
    if name_matches(o.name.as_deref(), "box") {
        world.fire(h, "ondissolvebox", Some(other), 0.0);
    }
    env.dissolve(other);
    world.slot_mut(other).dissolving = true;
    world.remove(other);
    world.fire(h, "ondissolve", Some(other), 0.0);
}

/// Translated from 0x9c8e90 and 0x9c8cf0 (decompiled, server.so)
fn multi_touch(world: &mut World, h: Handle, other: Handle) {
    if !trigger(world, h).multi || !passes(world, h, other) {
        return;
    }
    let now = world.now();
    if world.next_think(h) > now {
        return;
    }
    world.fire(h, "ontrigger", Some(other), 0.0);
    let wait = trigger(world, h).wait;
    if wait <= 0.0 {
        trigger(world, h).multi = false;
        world.set_think(h, now + REMOVE_DELAY, Think::Remove);
    } else {
        world.set_think(h, now + wait, Think::WaitOver);
    }
}

/// Translated from 0x9c73b0 and 0x9c3250 (decompiled, server.so)
pub(super) fn input(world: &mut World, h: Handle, input: &str) -> bool {
    let t = trigger(world, h);
    match input {
        "enable" => {
            if t.disabled {
                world.enabled.push(h);
            }
            trigger(world, h).disabled = false;
        }
        "disable" => t.disabled = true,
        "toggle" => t.disabled ^= true,
        _ => return false,
    }
    true
}
