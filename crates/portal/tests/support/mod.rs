// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

#![allow(dead_code)]

use portal::dmx::{Element, Value};
use portal::particles::collection::{Collection, ControlPoint, Particle};
use portal::particles::random::{RandomTable, SimdRandom};
use portal::pcf::Function;

pub fn function(name: &str, params: Vec<(&str, Value)>) -> Function {
    Function {
        name: name.into(),
        params: Element {
            kind: "DmeParticleOperator".into(),
            name: name.into(),
            attributes: params.into_iter().map(|(k, v)| (k.into(), v)).collect(),
        },
    }
}

pub fn particle(id: u32, creation: f32, life: f32) -> Particle {
    let mut p = Particle {
        id,
        creation,
        life,
        alpha: 1.0,
        radius: 2.0,
        ..Particle::default()
    };
    p.initial.alpha = 1.0;
    p.initial.radius = 2.0;
    p
}

pub fn collection(
    table: &RandomTable,
    simd: SimdRandom,
    particles: Vec<Particle>,
    now: f32,
) -> Collection {
    Collection {
        particles,
        cur_time: now,
        dt: 0.1,
        prev_dt: 0.1,
        control_points: [ControlPoint::default(); 64],
        highest_cp: 0,
        seed: 0,
        op_offset: 0,
        kills: Vec::new(),
        table: table.clone(),
        simd,
        lock_fade: None,
        query_count: 0,
        unique_id: 0,
        template: Particle::default(),
        max_allowed: 1000,
    }
}

pub fn flat(v: f32) -> RandomTable {
    RandomTable::from_values(vec![v; 4096])
}

pub fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

/// T[i] = (i mod 8) / 8, so draws can be told apart by where they read.
pub fn ramp() -> RandomTable {
    RandomTable::from_values((0..4096).map(|i| (i & 7) as f32 / 8.0).collect())
}
