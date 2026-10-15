// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use super::io::Handle;
use super::math::{angle_matrix, concat, invert, matrix_angles, Matrix};
use super::{Kind, World};
use crate::vmf::Entity;

const KEEP_ENTITIES: u32 = 0x1;
const PRESERVE_NAMES: u32 = 0x2;
const MAKER_CHECKS: u32 = 0x1 | 0x2 | 0x8 | 0x10;

#[derive(Debug, Clone)]
pub(super) struct Template {
    names: Vec<String>,
    entries: Vec<(Entity, Matrix)>,
}

impl Template {
    pub(super) fn new(e: &Entity) -> Template {
        let names = (1..=16)
            .filter_map(|i| e.get(&format!("Template{i:02}")))
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .collect();
        Template {
            names,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Maker {
    template: String,
}

impl Maker {
    pub(super) fn new(template: &str) -> Maker {
        Maker {
            template: template.to_string(),
        }
    }
}

/// Translated from 0x8dac10, 0x8db150 and 0x8dab90 (decompiled, server.so)
pub(super) fn build(world: &mut World, h: Handle) {
    let Kind::Template(t) = &world.slot(h).kind else {
        return;
    };
    let names = t.names.clone();
    let flags = world.slot(h).spawnflags;
    if flags & PRESERVE_NAMES == 0 {
        world.refuse(format!(
            "point_template {} needs name fixup, which is not ported",
            world.slot(h).label()
        ));
    }
    let (angles, origin) = world.slot(h).pose;
    let inverse = invert(&angle_matrix(angles, origin));
    let mut entries = Vec::new();
    for name in names {
        for e in world.lookup(&name, None, None) {
            let (a, o) = world.slot(e).pose;
            entries.push((
                world.slot(e).entity.clone(),
                concat(&inverse, &angle_matrix(a, o)),
            ));
            if flags & KEEP_ENTITIES == 0 {
                world.slot_mut(e).gone = true;
            }
        }
    }
    if let Kind::Template(t) = &mut world.slot_mut(h).kind {
        t.entries = entries;
    }
}

pub(super) fn check_maker(world: &mut World, h: Handle) {
    let s = world.slot(h);
    let speed = s
        .entity
        .get("PostSpawnSpeed")
        .map_or(0.0, |v| v.trim().parse().unwrap_or(0.0));
    if s.spawnflags & MAKER_CHECKS != 0 || speed != 0.0 {
        world.refuse(format!(
            "env_entity_maker {} uses spawn checks or a spawn speed, not ported",
            s.label()
        ));
    }
}

/// Translated from 0x512220, 0x510fb0, 0x510f00 and 0x8dad10 (decompiled, server.so)
pub(super) fn input(world: &mut World, h: Handle, input: &str) -> bool {
    if input != "forcespawn" {
        return false;
    }
    let Kind::Maker(m) = &world.slot(h).kind else {
        return false;
    };
    let template = world
        .lookup(&m.template.clone(), None, None)
        .into_iter()
        .find(|&t| matches!(world.slot(t).kind, Kind::Template(_)));
    let Some(Kind::Template(t)) = template.map(|t| &world.slot(t).kind) else {
        return true;
    };
    let entries = t.entries.clone();
    if entries.is_empty() {
        return true;
    }
    let (angles, origin) = world.slot(h).pose;
    let at = angle_matrix(angles, origin);
    for (e, local) in entries {
        let (angles, origin) = matrix_angles(&concat(&at, &local));
        world.instantiate(e, origin, angles);
    }
    world.fire(h, "onentityspawned", Some(h), 0.0);
    true
}
