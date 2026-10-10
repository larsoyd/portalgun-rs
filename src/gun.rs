use portal::pair::Vec3;
use portal::placement::{place, Placement, World};

use crate::portals::{Colour, Pair};
use crate::walker::Walker;

/// Both buttons wait this long after either shot
pub const REFIRE: f32 = 0.5;
/// a player's shot, a pedestal's flies at 1000
const SHOT_SPEED: f32 = 3000.0;
/// the client never lets the stream end sooner
const STREAM_MIN: f32 = 0.1;
/// m_fMaxRange1, the trace length
const RANGE: f32 = 56755.84;
/// where the stream leaves the gun, forward right and up of the eye
const MUZZLE: Vec3 = [30.0, 4.0, -5.0];

pub struct Shot {
    pub colour: Colour,
    pub placement: Placement,
    /// the trace's hit point, which times the flight
    pub landed: Vec3,
    /// where the stream ends, the portal's spot when it fits
    pub ends: Vec3,
    pub from: Vec3,
    pub aim: Vec3,
    pub launched: f32,
    pub arrives: f32,
    pub stream_ends: f32,
}

#[derive(Default)]
pub struct Gun {
    next_attack: f32,
    flying: Vec<Shot>,
}

impl Gun {
    // Translated from 0x2f5f00 (disassembly, server.so)
    pub fn deploy(&mut self, now: f32) {
        self.next_attack = now;
    }

    // Translated from 0x2f4000, 0x2f4480 and 0x935fc0 (disassembly, server.so)
    pub fn fire(
        &mut self,
        colour: Colour,
        now: f32,
        w: &Walker,
        world: &impl World,
        pair: &Pair,
    ) -> Option<&Shot> {
        if now < self.next_attack {
            return None;
        }
        self.next_attack = now + REFIRE;
        let (eye, dir) = (w.eye(), w.forward());
        let (placement, landed) = place(world, eye, dir, RANGE, pair.get(colour.other()));
        let from = tracer_origin(w);
        let ends = match placement {
            Placement::Placed { portal, .. } => portal.origin,
            Placement::Failed(_) => landed,
        };
        let arrives = now + flight_time(from, landed);
        self.flying.push(Shot {
            colour,
            placement,
            landed,
            ends,
            from,
            aim: along(from, dir, distance(from, ends)),
            launched: now,
            arrives,
            stream_ends: arrives.max(now + STREAM_MIN),
        });
        self.flying.last()
    }

    pub fn flying(&self) -> &[Shot] {
        &self.flying
    }

    pub fn land(&mut self, now: f32) -> Vec<Shot> {
        let (landed, flying) = self.flying.drain(..).partition(|s| s.arrives <= now);
        self.flying = flying;
        landed
    }
}

fn along(p: Vec3, d: Vec3, t: f32) -> Vec3 {
    [0, 1, 2].map(|i| p[i] + d[i] * t)
}

fn distance(a: Vec3, b: Vec3) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}

pub fn tracer_origin(w: &Walker) -> Vec3 {
    let (f, r, u) = (w.forward(), w.right(), w.up());
    [0, 1, 2].map(|i| w.eye()[i] + f[i] * MUZZLE[0] + r[i] * MUZZLE[1] + u[i] * MUZZLE[2])
}

pub fn flight_time(from: Vec3, to: Vec3) -> f32 {
    distance(from, to) / SHOT_SPEED
}

// Translated from 0x560a00 (disassembly, client.so)
pub fn stream_at(start: Vec3, aim: Vec3, end: Vec3, t: f32) -> Vec3 {
    [0, 1, 2].map(|i| start[i] * (1.0 - t) + aim[i] * t * (1.0 - t) + end[i] * t * t)
}
