// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

mod door;
pub mod io;
mod logic;
mod trigger;

use std::collections::HashMap;
use std::fmt;

use crate::brush;
use crate::pair::Vec3;
use crate::vmf::Entity;
use door::Door;
use io::{name_matches, Event, Handle, OutputEvent, Queue, Target};
use logic::{Auto, Relay};
use trigger::{Filter, Trigger};

#[derive(Debug, Clone, PartialEq)]
pub struct Fired {
    pub time: f32,
    pub caller: String,
    pub target: String,
    pub input: String,
    pub param: String,
    pub handled: bool,
}

impl fmt::Display for Fired {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}({})", self.target, self.input, self.param)?;
        if !self.handled {
            write!(f, " unhandled")?;
        }
        Ok(())
    }
}

/// what the entity world asks of the game around it
pub trait Env {
    /// moves a pusher's hierarchy and riders by delta, or names what stopped it
    fn push(&mut self, pusher: Handle, delta: Vec3) -> Option<Handle>;
    /// movers whose boxes are inside a trigger that was just switched on
    fn overlapping(&mut self, _trigger: Handle) -> Vec<Handle> {
        Vec::new()
    }
    /// movers that ran their trigger check this tick, each with the triggers its box is in
    fn movers(&mut self) -> Vec<(Handle, Vec<Handle>)> {
        Vec::new()
    }
    /// fizzles the player's portals and pending shots, true when there was something to fizzle
    fn fizzle(&mut self, _player: Handle) -> bool {
        false
    }
    /// drops the prop if held and dissolves a copy of it in its place
    fn dissolve(&mut self, _entity: Handle) {}
}

pub struct Still;

impl Env for Still {
    fn push(&mut self, _pusher: Handle, _delta: Vec3) -> Option<Handle> {
        None
    }
}

#[derive(Debug, Clone, Copy)]
struct Link {
    mover: Handle,
    trigger: Handle,
    stamp: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sound {
    pub source: Handle,
    pub name: String,
}

#[derive(Debug)]
enum Kind {
    Door(Door),
    Relay(Relay),
    Auto(Auto),
    Trigger(Trigger),
    Filter(Filter),
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Body {
    None,
    Player,
    Physics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Think {
    Auto,
    WaitOver,
    Remove,
}

#[derive(Debug)]
struct Slot {
    classname: String,
    name: Option<String>,
    spawnflags: u32,
    outputs: HashMap<String, OutputEvent>,
    think: Option<(u32, Think)>,
    kind: Kind,
    body: Body,
    gone: bool,
    dissolving: bool,
}

impl Slot {
    fn label(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.classname)
    }
}

#[derive(Debug, Default)]
pub struct World {
    slots: Vec<Slot>,
    queue: Queue,
    tick: u32,
    doomed: Vec<Handle>,
    fired: Vec<Fired>,
    sounds: Vec<Sound>,
    links: Vec<Link>,
    stamps: HashMap<Handle, u32>,
    checked: Vec<Handle>,
    enabled: Vec<Handle>,
}

/// atoi, as the datadesc keyvalue parser reads integer and boolean fields
fn atoi(text: &str) -> i32 {
    let t = text.trim_start();
    let (sign, digits) = match t.as_bytes().first() {
        Some(b'-') => (-1, &t[1..]),
        Some(b'+') => (1, &t[1..]),
        _ => (1, t),
    };
    let end = digits
        .bytes()
        .position(|b| !b.is_ascii_digit())
        .unwrap_or(digits.len());
    sign * digits[..end].parse::<i32>().unwrap_or(0)
}

fn int_key(e: &Entity, key: &str) -> i32 {
    e.get(key).map_or(0, atoi)
}

/// atof, for float fields
fn float_key(e: &Entity, key: &str) -> f32 {
    let Some(text) = e.get(key) else { return 0.0 };
    let t = text.trim_start();
    let end = t
        .char_indices()
        .take_while(|&(i, c)| c.is_ascii_digit() || c == '.' || (i == 0 && (c == '-' || c == '+')))
        .count();
    t[..end].parse().unwrap_or(0.0)
}

impl World {
    pub const TICK: f32 = 0.015;

    pub fn spawn(ents: &[Entity]) -> World {
        World::spawn_with(ents, |e| {
            let polys: Vec<_> = e
                .solids
                .iter()
                .flat_map(|s| brush::faces(&s.sides))
                .collect();
            (!polys.is_empty()).then(|| brush::bounds(&polys))
        })
    }

    /// bounds gives a brush entity's model mins and maxs
    pub fn spawn_with(ents: &[Entity], bounds: impl Fn(&Entity) -> Option<(Vec3, Vec3)>) -> World {
        let mut world = World::default();
        for e in ents {
            world.slots.push(slot(e, bounds(e)));
        }
        world.activate();
        world
    }

    fn activate(&mut self) {
        let now = self.now();
        for i in 0..self.slots.len() {
            let h = Handle(i as u32);
            match &self.slots[i].kind {
                Kind::Auto(_) => self.set_think(h, now + logic::AUTO_DELAY, Think::Auto),
                Kind::Trigger(t) => {
                    let mut t = t.clone();
                    t.bind_filter(self);
                    self.slots[i].kind = Kind::Trigger(t);
                }
                _ => {}
            }
        }
    }

    pub fn now(&self) -> f32 {
        self.tick as f32 * Self::TICK
    }

    pub fn tick(&mut self) {
        self.step(&mut Still);
    }

    pub fn step(&mut self, env: &mut impl Env) {
        self.tick += 1;
        self.think();
        self.each_door(|w, h| door::push(w, h, env));
        for (mover, triggers) in env.movers() {
            self.check(env, mover, &triggers);
        }
        self.each_door(door::arrive);
        self.untouch();
        self.service(env);
        self.cleanup();
    }

    /// Translated from 0x4962c0, 0x5847d0 and 0x2b3450 (decompiled, server.so)
    fn check(&mut self, env: &mut impl Env, mover: Handle, triggers: &[Handle]) {
        if self.slot(mover).gone {
            return;
        }
        let stamp = self.stamps.entry(mover).or_default();
        *stamp += 1;
        let stamp = *stamp;
        if !self.checked.contains(&mover) {
            self.checked.push(mover);
        }
        for &t in triggers {
            if !self.slot(t).gone && self.trigger_enabled(t) {
                self.link(env, mover, t, stamp);
            }
        }
    }

    fn link(&mut self, env: &mut impl Env, mover: Handle, trigger: Handle, stamp: u32) {
        match self
            .links
            .iter_mut()
            .find(|l| l.mover == mover && l.trigger == trigger)
        {
            Some(l) => l.stamp = stamp,
            None => {
                self.links.push(Link {
                    mover,
                    trigger,
                    stamp,
                });
                trigger::start_touch(self, trigger, mover);
            }
        }
        trigger::touch(self, trigger, mover, env);
    }

    /// Translated from 0x4fda30 and 0x2b2aa0 (decompiled, server.so)
    fn untouch(&mut self) {
        for mover in std::mem::take(&mut self.checked) {
            let stamp = self.stamps.get(&mover).copied().unwrap_or_default();
            let stale: Vec<Link> = self
                .links
                .iter()
                .filter(|l| l.mover == mover && l.stamp != stamp)
                .copied()
                .collect();
            self.links.retain(|l| l.mover != mover || l.stamp == stamp);
            for l in stale {
                if !self.slot(l.trigger).gone {
                    trigger::end_touch(self, l.trigger, mover);
                }
            }
        }
    }

    /// Translated from 0x9c73b0 (decompiled, server.so), the engine links what is already inside
    fn link_inside(&mut self, env: &mut impl Env) {
        for t in std::mem::take(&mut self.enabled) {
            for mover in env.overlapping(t) {
                let stamp = self.stamps.get(&mover).copied().unwrap_or_default();
                self.link(env, mover, t, stamp);
            }
        }
    }

    fn each_door(&mut self, mut f: impl FnMut(&mut World, Handle)) {
        for i in 0..self.slots.len() {
            if matches!(self.slots[i].kind, Kind::Door(_)) && !self.slots[i].gone {
                f(self, Handle(i as u32));
            }
        }
    }

    fn emit(&mut self, source: Handle, name: &str) {
        if !name.is_empty() {
            self.sounds.push(Sound {
                source,
                name: name.to_string(),
            });
        }
    }

    pub fn drain_sounds(&mut self) -> Vec<Sound> {
        std::mem::take(&mut self.sounds)
    }

    /// where a moving brush entity is now
    pub fn origin(&self, h: Handle) -> Vec3 {
        match &self.slot(h).kind {
            Kind::Door(d) => d.origin,
            _ => [0.0; 3],
        }
    }

    fn think(&mut self) {
        for i in 0..self.slots.len() {
            let h = Handle(i as u32);
            let Some((at, think)) = self.slots[i].think else {
                continue;
            };
            if at > self.tick || self.slots[i].gone {
                continue;
            }
            self.slots[i].think = None;
            match (think, &self.slots[i].kind) {
                (Think::Auto, Kind::Auto(auto)) => auto.clone().think(self, h),
                (Think::Remove, _) => self.remove(h),
                _ => {}
            }
        }
    }

    fn set_think(&mut self, h: Handle, time: f32, think: Think) {
        self.slot_mut(h).think = Some((think_tick(time), think));
    }

    /// Translated from 0x249730 (decompiled, server.so)
    fn next_think(&self, h: Handle) -> f32 {
        self.slot(h)
            .think
            .map_or(-1.0, |(t, _)| t as f32 * Self::TICK)
    }

    fn service(&mut self, env: &mut impl Env) {
        let now = self.now();
        while let Some(event) = self.queue.pop_due(now) {
            self.deliver(event);
            self.link_inside(env);
        }
    }

    fn cleanup(&mut self) {
        for h in std::mem::take(&mut self.doomed) {
            self.slots[h.0 as usize].gone = true;
        }
    }

    /// Translated from 0x4ce380 (decompiled, server.so)
    fn deliver(&mut self, e: Event) {
        let targets = match &e.target {
            Target::Entity(h) => vec![*h],
            Target::Name(name) => self.lookup(name, e.activator, e.caller),
        };
        let caller = e
            .caller
            .map(|h| self.slot(h).label().to_string())
            .unwrap_or_default();
        let param = e.value.clone().unwrap_or_default();
        if targets.is_empty() {
            let Target::Name(name) = &e.target else {
                return;
            };
            self.record(&caller, name.clone(), &e.input, &param, false);
            return;
        }
        for h in targets {
            let handled = self.accept(h, &e.input, e.activator);
            let label = self.slot(h).label().to_string();
            self.record(&caller, label, &e.input, &param, handled);
        }
    }

    fn record(&mut self, caller: &str, target: String, input: &str, param: &str, handled: bool) {
        self.fired.push(Fired {
            time: self.now(),
            caller: caller.to_string(),
            target,
            input: input.to_string(),
            param: param.to_string(),
            handled,
        });
    }

    /// Translated from 0x4fc850 and 0x4fc770 (decompiled, server.so)
    fn lookup(&self, name: &str, activator: Option<Handle>, caller: Option<Handle>) -> Vec<Handle> {
        if let Some(special) = name.strip_prefix('!') {
            return self
                .special(special, activator, caller)
                .into_iter()
                .collect();
        }
        let live = || self.slots.iter().enumerate().filter(|(_, s)| !s.gone);
        let by_name: Vec<Handle> = live()
            .filter(|(_, s)| s.name.is_some() && name_matches(s.name.as_deref(), name))
            .map(|(i, _)| Handle(i as u32))
            .collect();
        if !by_name.is_empty() {
            return by_name;
        }
        live()
            .filter(|(_, s)| name_matches(Some(&s.classname), name))
            .map(|(i, _)| Handle(i as u32))
            .collect()
    }

    /// Translated from 0x4fbcf0 (decompiled, server.so), !self is the event's caller
    fn special(
        &self,
        name: &str,
        activator: Option<Handle>,
        caller: Option<Handle>,
    ) -> Option<Handle> {
        match name.to_ascii_lowercase().as_str() {
            "activator" => activator,
            "caller" | "self" => caller,
            "player" | "pvsplayer" => self.first_of("player"),
            _ => None,
        }
    }

    fn first_of(&self, classname: &str) -> Option<Handle> {
        let i = self
            .slots
            .iter()
            .position(|s| !s.gone && s.classname == classname)?;
        Some(Handle(i as u32))
    }

    fn slot(&self, h: Handle) -> &Slot {
        &self.slots[h.0 as usize]
    }

    fn slot_mut(&mut self, h: Handle) -> &mut Slot {
        &mut self.slots[h.0 as usize]
    }

    fn accept(&mut self, h: Handle, input: &str, activator: Option<Handle>) -> bool {
        let lower = input.to_ascii_lowercase();
        if lower == "kill" {
            self.remove(h);
            return true;
        }
        match &self.slot(h).kind {
            Kind::Door(_) => door::input(self, h, &lower),
            Kind::Relay(_) => logic::relay_input(self, h, &lower, activator),
            Kind::Trigger(_) => trigger::input(self, h, &lower),
            Kind::Auto(_) | Kind::Filter(_) | Kind::Other => false,
        }
    }

    fn fire(&mut self, h: Handle, output: &str, activator: Option<Handle>, delay: f32) {
        let now = self.now();
        let Some(mut out) = self.slot_mut(h).outputs.remove(output) else {
            return;
        };
        out.fire(&mut self.queue, now, None, activator, Some(h), delay);
        self.slot_mut(h).outputs.insert(output.to_string(), out);
    }

    fn max_delay(&self, h: Handle, output: &str) -> f32 {
        self.slot(h)
            .outputs
            .get(output)
            .map_or(0.0, OutputEvent::max_delay)
    }

    fn queue_self(&mut self, h: Handle, input: &str, delay: f32) {
        let now = self.now();
        self.queue
            .add(Target::Entity(h), input, None, now, delay, Some(h), Some(h));
    }

    fn cancel(&mut self, h: Handle) {
        self.queue.cancel(h);
    }

    fn remove(&mut self, h: Handle) {
        if !self.doomed.contains(&h) {
            self.doomed.push(h);
        }
    }

    pub fn input(&mut self, target: &str, input: &str, param: Option<&str>) {
        let now = self.now();
        let target = Target::Name(target.to_string());
        self.queue.add(
            target,
            input,
            param.map(str::to_string),
            now,
            0.0,
            None,
            None,
        );
    }

    pub fn input_from(&mut self, target: &str, input: &str, param: Option<&str>, from: &str) {
        let from = self.find(from);
        let now = self.now();
        let target = Target::Name(target.to_string());
        self.queue.add(
            target,
            input,
            param.map(str::to_string),
            now,
            0.0,
            from,
            from,
        );
    }

    pub fn find(&self, name: &str) -> Option<Handle> {
        let i = self.slots.iter().position(|s| {
            !s.gone
                && s.name
                    .as_deref()
                    .is_some_and(|n| name_matches(Some(n), name))
        })?;
        Some(Handle(i as u32))
    }

    pub fn count(&self, classname: &str) -> usize {
        self.slots
            .iter()
            .filter(|s| !s.gone && s.classname == classname)
            .count()
    }

    pub fn add_player(&mut self) -> Handle {
        let player = Entity {
            classname: "player".into(),
            keys: Vec::new(),
            outputs: Vec::new(),
            solids: Vec::new(),
        };
        self.slots.push(slot(&player, None));
        Handle(self.slots.len() as u32 - 1)
    }

    pub fn start_touch(&mut self, trigger: Handle, other: Handle) {
        trigger::start_touch(self, trigger, other);
    }

    pub fn touch(&mut self, trigger: Handle, other: Handle) {
        trigger::touch(self, trigger, other, &mut Still);
    }

    pub fn end_touch(&mut self, trigger: Handle, other: Handle) {
        trigger::end_touch(self, trigger, other);
    }

    pub fn trigger_enabled(&self, trigger: Handle) -> bool {
        matches!(&self.slot(trigger).kind, Kind::Trigger(t) if !t.disabled)
    }

    pub fn drain_fired(&mut self) -> Vec<Fired> {
        std::mem::take(&mut self.fired)
    }
}

/// Translated from 0x24d880 (decompiled, server.so)
fn think_tick(time: f32) -> u32 {
    (time / World::TICK + 0.5) as u32
}

fn trigger_of(e: &Entity, once: bool, multi: bool) -> Kind {
    let disabled = int_key(e, "StartDisabled") != 0;
    Kind::Trigger(Trigger::new(
        once,
        multi,
        disabled,
        float_key(e, "wait"),
        e.get("filtername"),
    ))
}

fn vec3_key(e: &Entity, key: &str) -> Vec3 {
    let mut parts = e.get(key).unwrap_or_default().split_whitespace();
    [0; 3].map(|_| parts.next().and_then(|p| p.parse().ok()).unwrap_or(0.0))
}

fn door_of(e: &Entity, bounds: Option<(Vec3, Vec3)>, spawnflags: u32) -> Kind {
    let (lo, hi) = bounds.unwrap_or_default();
    let movedir = vec3_key(e, "movedir");
    Kind::Door(Door::new(
        vec3_key(e, "origin"),
        [movedir[0], movedir[1]],
        [0, 1, 2].map(|i| hi[i] - lo[i]),
        float_key(e, "lip"),
        float_key(e, "speed"),
        float_key(e, "wait"),
        spawnflags,
        (
            e.get("noise2").unwrap_or_default(),
            e.get("closesound").unwrap_or_default(),
        ),
    ))
}

fn slot(e: &Entity, bounds: Option<(Vec3, Vec3)>) -> Slot {
    let mut outputs: HashMap<String, Vec<&crate::vmf::Output>> = HashMap::new();
    for o in &e.outputs {
        outputs
            .entry(o.event.to_ascii_lowercase())
            .or_default()
            .push(o);
    }
    let mut spawnflags = int_key(e, "spawnflags") as u32;
    let kind = match e.classname.as_str() {
        "func_door" => door_of(e, bounds, spawnflags),
        "logic_relay" => Kind::Relay(Relay::new(int_key(e, "StartDisabled") != 0)),
        "logic_auto" => Kind::Auto(Auto::new(e.get("globalstate"))),
        "trigger_multiple" | "trigger_once" => {
            spawnflags = trigger::spawnflags(spawnflags);
            trigger_of(e, e.classname == "trigger_once", true)
        }
        "trigger_portal_cleanser" => {
            spawnflags = trigger::spawnflags(spawnflags);
            trigger_of(e, false, false)
        }
        "filter_activator_name" | "filter_activator_class" => {
            let by_class = e.classname == "filter_activator_class";
            let name = e
                .get(if by_class {
                    "filterclass"
                } else {
                    "filtername"
                })
                .unwrap_or_default();
            Kind::Filter(Filter::new(int_key(e, "Negated") != 0, by_class, name))
        }
        _ => Kind::Other,
    };
    let body = match e.classname.as_str() {
        "player" => Body::Player,
        "prop_physics" | "prop_physics_override" => Body::Physics,
        _ => Body::None,
    };
    Slot {
        classname: e.classname.clone(),
        name: e
            .get("targetname")
            .filter(|n| !n.is_empty())
            .map(str::to_string),
        spawnflags,
        outputs: outputs
            .into_iter()
            .map(|(k, v)| (k, OutputEvent::new(v.into_iter())))
            .collect(),
        think: None,
        kind,
        body,
        gone: false,
        dissolving: false,
    }
}
