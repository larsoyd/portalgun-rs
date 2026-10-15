// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::vmf::Output;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Handle(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub target: String,
    pub input: String,
    pub param: Option<String>,
    pub delay: f32,
    pub times: i32,
}

impl Action {
    /// Translated from 0x4cd100 (decompiled, server.so)
    pub fn new(o: &Output) -> Action {
        let input = if o.input.is_empty() { "Use" } else { &o.input };
        Action {
            target: o.target.clone(),
            input: input.to_string(),
            param: (!o.param.is_empty()).then(|| o.param.clone()),
            delay: o.delay,
            times: if o.times == 0 { -1 } else { o.times },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Name(String),
    Entity(Handle),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub time: f32,
    pub target: Target,
    pub input: String,
    pub value: Option<String>,
    pub activator: Option<Handle>,
    pub caller: Option<Handle>,
}

#[derive(Debug, Default)]
pub struct Queue {
    events: Vec<Event>,
}

impl Queue {
    /// Translated from 0x4cdaf0 (decompiled, server.so)
    #[allow(clippy::too_many_arguments)]
    pub fn add(
        &mut self,
        target: Target,
        input: &str,
        value: Option<String>,
        now: f32,
        delay: f32,
        activator: Option<Handle>,
        caller: Option<Handle>,
    ) {
        let time = now + delay;
        let at = self.events.partition_point(|e| e.time <= time);
        let event = Event {
            time,
            target,
            input: input.to_string(),
            value,
            activator,
            caller,
        };
        self.events.insert(at, event);
    }

    /// Translated from 0x4ce380 (decompiled, server.so)
    pub fn pop_due(&mut self, now: f32) -> Option<Event> {
        let due = self.events.first().is_some_and(|e| now >= e.time);
        due.then(|| self.events.remove(0))
    }

    /// Translated from 0x4ceac0 (decompiled, server.so)
    pub fn cancel(&mut self, caller: Handle) {
        self.events.retain(|e| e.caller != Some(caller));
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

#[derive(Debug, Clone, Default)]
pub struct OutputEvent {
    actions: Vec<Action>,
}

impl OutputEvent {
    /// each action goes on the front of the list, so they fire last keyvalue first
    pub fn new<'a>(outputs: impl Iterator<Item = &'a Output>) -> OutputEvent {
        let mut actions: Vec<Action> = outputs.map(Action::new).collect();
        actions.reverse();
        OutputEvent { actions }
    }

    /// Translated from 0x4cdc80 (disassembled, server.so)
    pub fn fire(
        &mut self,
        queue: &mut Queue,
        now: f32,
        value: Option<&str>,
        activator: Option<Handle>,
        caller: Option<Handle>,
        delay: f32,
    ) {
        for a in &self.actions {
            let (value, delay) = match &a.param {
                Some(p) => (Some(p.clone()), a.delay),
                None => (value.map(str::to_string), a.delay + delay),
            };
            let target = Target::Name(a.target.clone());
            queue.add(target, &a.input, value, now, delay, activator, caller);
        }
        for a in &mut self.actions {
            if a.times != -1 {
                a.times -= 1;
            }
        }
        self.actions.retain(|a| a.times != 0);
    }

    /// Translated from 0x4cd3c0 (decompiled, server.so)
    pub fn max_delay(&self) -> f32 {
        self.actions
            .iter()
            .fold(0.0, |m, a| if a.delay > m { a.delay } else { m })
    }
}

/// Translated from 0x496b30 (decompiled, server.so), the !player case lives with the caller
pub fn name_matches(name: Option<&str>, query: &str) -> bool {
    let Some(name) = name else {
        return query.is_empty() || query.starts_with('*');
    };
    let (n, q) = (name.as_bytes(), query.as_bytes());
    for (i, &c) in n.iter().enumerate() {
        let Some(&qc) = q.get(i) else {
            return false;
        };
        let folds = if c < 0x5b {
            c + 0x20 == qc
        } else {
            c <= 0x7a && c - 0x20 == qc
        };
        if c != qc && !folds {
            return qc == b'*';
        }
    }
    q.get(n.len()).is_none_or(|&qc| qc == b'*')
}
