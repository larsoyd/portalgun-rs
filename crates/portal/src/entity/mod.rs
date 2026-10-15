// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

pub mod io;
mod logic;
mod trigger;

use std::collections::HashMap;
use std::fmt;

use crate::vmf::Entity;
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

#[derive(Debug)]
enum Kind {
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
        let mut world = World::default();
        for e in ents {
            world.slots.push(slot(e));
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
        self.tick += 1;
        self.think();
        self.service();
        self.cleanup();
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

    fn service(&mut self) {
        let now = self.now();
        while let Some(event) = self.queue.pop_due(now) {
            self.deliver(event);
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
        self.slots.push(slot(&player));
        Handle(self.slots.len() as u32 - 1)
    }

    pub fn start_touch(&mut self, trigger: Handle, other: Handle) {
        trigger::start_touch(self, trigger, other);
    }

    pub fn touch(&mut self, trigger: Handle, other: Handle) {
        trigger::touch(self, trigger, other);
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

fn slot(e: &Entity) -> Slot {
    let mut outputs: HashMap<String, Vec<&crate::vmf::Output>> = HashMap::new();
    for o in &e.outputs {
        outputs
            .entry(o.event.to_ascii_lowercase())
            .or_default()
            .push(o);
    }
    let mut spawnflags = int_key(e, "spawnflags") as u32;
    let kind = match e.classname.as_str() {
        "logic_relay" => Kind::Relay(Relay::new(int_key(e, "StartDisabled") != 0)),
        "logic_auto" => Kind::Auto(Auto::new(e.get("globalstate"))),
        "trigger_multiple" | "trigger_once" => {
            spawnflags = trigger::spawnflags(spawnflags);
            trigger_of(e, e.classname == "trigger_once", true)
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
    }
}
