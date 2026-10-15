// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use super::io::Handle;
use super::{Kind, World};

const START_SILENT: u32 = 0x10;
const NOT_LOOPING: u32 = 0x20;

#[derive(Debug, Clone)]
pub(super) struct Ambient {
    message: String,
    source_name: Option<String>,
    source: Option<Handle>,
    looping: bool,
    active: bool,
}

impl Ambient {
    /// Translated from 0x98dc50 and 0x98d2b0 (decompiled, server.so)
    pub(super) fn new(message: &str, source: Option<&str>, spawnflags: u32) -> Ambient {
        let looping = spawnflags & NOT_LOOPING == 0;
        Ambient {
            message: message.to_string(),
            source_name: source.filter(|s| !s.is_empty()).map(str::to_string),
            source: None,
            looping,
            active: looping && spawnflags & START_SILENT == 0,
        }
    }

    /// Translated from 0x98e730 (decompiled, server.so)
    pub(super) fn bind_source(&mut self, world: &World, own: Handle) {
        self.source = Some(
            self.source_name
                .as_deref()
                .and_then(|n| world.find(n))
                .unwrap_or(own),
        );
    }
}

fn ambient(world: &mut World, h: Handle) -> &mut Ambient {
    match &mut world.slot_mut(h).kind {
        Kind::Ambient(a) => a,
        _ => unreachable!("ambient call on another class"),
    }
}

fn send(world: &mut World, h: Handle, stop: bool) {
    let a = ambient(world, h);
    let (source, name) = (a.source.unwrap_or(h), a.message.clone());
    world.sound(source, &name, stop);
}

/// Translated from 0x98ed90, 0x98edd0 and 0x98eab0 (decompiled, server.so), fades left out
pub(super) fn input(world: &mut World, h: Handle, input: &str) -> bool {
    let active = ambient(world, h).active;
    match input {
        "playsound" if !active => {
            send(world, h, true);
            toggle(world, h);
        }
        "stopsound" if active => toggle(world, h),
        "togglesound" => toggle(world, h),
        "playsound" | "stopsound" => {}
        _ => return false,
    }
    true
}

fn toggle(world: &mut World, h: Handle) {
    let a = ambient(world, h);
    if a.active {
        a.active = false;
        world.slot_mut(h).spawnflags |= START_SILENT;
        send(world, h, true);
        return;
    }
    if a.looping {
        a.active = true;
    } else {
        send(world, h, true);
    }
    send(world, h, false);
}
