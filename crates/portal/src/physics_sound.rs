// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

//! When a physics object plays its impact and scrape sounds, in Source units.

use crate::surfaces::Surface;

pub type Vec3 = [f32; 3];

/// Translated from 0x870170 (decompiled, server.so)
pub fn collision_volume(speed: f32, since_last: f32) -> Option<f32> {
    (since_last >= 0.05 && speed >= 70.0).then(|| (speed * speed / 102400.0).min(1.0))
}

/// The impacting object's own sound, soft when the surface it hit is softer than it can ring.
/// Translated from 0x871130 (decompiled, server.so)
pub fn impact_sound<'a>(own: &'a Surface, hit: &Surface, speed: f32) -> Option<&'a str> {
    if own.impact_hard.is_empty() {
        return None;
    }
    let soft = !own.impact_soft.is_empty()
        && (own.impact_hard_threshold > hit.hardness
            || own.hard_min_velocity > 0.0 && own.hard_min_velocity > speed);
    Some(if soft {
        &own.impact_soft
    } else {
        &own.impact_hard
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct Queued {
    pub own: usize,
    pub hit: usize,
    pub volume: f32,
    pub speed: f32,
    pub origin: Vec3,
}

/// Impacts gathered between flushes, one per surface.
#[derive(Debug, Default)]
pub struct ImpactQueue {
    entries: Vec<Queued>,
    elapsed: f32,
}

impl ImpactQueue {
    /// Translated from 0x86cba0 (decompiled, server.so)
    pub fn add(&mut self, own: usize, hit: usize, volume: f32, speed: f32, origin: Vec3) {
        let speed = (speed as f64 + 0.0001) as f32;
        let crowded = self.entries.len() > 4;
        if let Some(e) = self
            .entries
            .iter_mut()
            .rev()
            .find(|e| e.own == own || crowded)
        {
            if e.volume < volume {
                e.origin = origin;
                e.hit = hit;
            }
            e.volume += volume;
            e.speed = e.speed.max(speed);
            return;
        }
        self.entries.push(Queued {
            own,
            hit,
            volume,
            speed,
            origin,
        });
    }

    /// Translated from 0x8718e0 and 0x871130 (decompiled, server.so)
    pub fn flush(&mut self, frametime: f32) -> Vec<Queued> {
        self.elapsed += frametime;
        if self.elapsed <= 0.05 {
            return Vec::new();
        }
        self.elapsed = 0.0;
        let mut out: Vec<Queued> = self.entries.drain(..).rev().collect();
        for e in &mut out {
            e.volume = e.volume.min(1.0);
        }
        out
    }
}

/// The slip work a contact gathered, as game energy, when vphysics reports it.
/// Translated from raw 0x20d70 (disassembled, vphysics.so)
pub fn slip_energy(work: f64, inv_mass: f64, elapsed: f64) -> Option<f32> {
    let energy = work * inv_mass / elapsed;
    (energy > 0.05).then_some((energy * 1550.0032) as f32)
}

/// Translated from 0x2cef70 (decompiled, server.so)
pub fn scrape_sound<'a>(own: &'a Surface, hit: &Surface, energy: f32) -> Option<(&'a str, f32)> {
    if energy < 75.0 {
        return None;
    }
    let smooth = !own.scrape_smooth.is_empty() && hit.roughness < own.scrape_rough_threshold;
    let name = if smooth {
        &own.scrape_smooth
    } else {
        &own.scrape_rough
    };
    let e = energy * (1.0 / 15500.0);
    Some((name, e * e))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrapeStep {
    Nothing,
    Keep,
    Start { volume: f32 },
    Retarget { volume: f32, pitch: f32 },
}

/// One object's scrape loop.
#[derive(Debug, Default)]
pub struct Scrape {
    playing: bool,
    last_update: f32,
    last_sound: f32,
}

impl Scrape {
    /// Translated from 0x86c0e0 and 0x870590 (decompiled, server.so)
    pub fn friction(
        &mut self,
        now: f32,
        volume: Option<f32>,
        script: f32,
        pitch: (f32, f32),
    ) -> ScrapeStep {
        if self.playing && now < self.last_sound + 0.5 {
            self.last_update = now;
            return ScrapeStep::Keep;
        }
        let Some(volume) = volume.map(|v| v.clamp(0.0, 1.0)) else {
            return ScrapeStep::Nothing;
        };
        if volume <= 1.0 / 128.0 {
            return ScrapeStep::Nothing;
        }
        let step = if self.playing {
            ScrapeStep::Retarget {
                volume: volume * script,
                pitch: pitch.0 + (pitch.1 - pitch.0) * volume,
            }
        } else if volume * script > 0.1 {
            self.playing = true;
            ScrapeStep::Start {
                volume: volume * script,
            }
        } else {
            return ScrapeStep::Nothing;
        };
        self.last_update = now;
        self.last_sound = now;
        step
    }

    /// Translated from 0x86e370 (decompiled, server.so)
    pub fn expired(&mut self, now: f32) -> bool {
        let gone = self.playing && now - 0.1 > self.last_update;
        if gone {
            self.playing = false;
        }
        gone
    }
}
