// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

//! The parts of vphysics' IVP that move a single rigid body, in IVP's own metres and radians.

pub type Vec3 = [f32; 3];
pub type Quat = [f64; 4];

/// Translated from raw 0x2042f (disassembled, vphysics.so)
pub const TOLERANCE: f32 = (0.25 - 1e-4) * 0.0254;
const SLEEP_WINDOW: f32 = 0.3;

/// Translated from 0xdeba0 (decompiled, vphysics.so)
pub fn damping(rot_damp: Vec3, damp: f32, dt: f64) -> (Vec3, f64) {
    let r = rot_damp.map(|d| (d as f64 * dt) as f32);
    let angular = if r.iter().map(|v| v * v).sum::<f32>() >= 0.5 {
        r.map(|v| (-v).exp())
    } else {
        r.map(|v| 1.0 - v)
    };
    let s = dt * damp as f64;
    (angular, if s >= 0.25 { (-s).exp() } else { 1.0 - s })
}

/// Translated from 0x3c740 and 0x3c850 (decompiled, vphysics.so)
pub fn combine(a: f32, b: f32) -> f32 {
    (a * b).clamp(0.0, 1.0)
}

/// Translated from 0x12f9f0 (decompiled, vphysics.so)
pub fn rotation_steps(spin: f64, dt: f64) -> usize {
    let turn = spin * spin * dt * dt;
    if turn > 0.02777777943346238 {
        (turn * 143.99999141693152).sqrt() as usize + 1
    } else {
        1
    }
}

/// The normal push that holds a resting contact at twice the tolerance.
/// Translated from 0xe5790 (decompiled, vphysics.so)
pub fn normal_impulse(closing: f64, gap: f32, mass: f32) -> f64 {
    let error = 2.0 * TOLERANCE - gap;
    let gain = if error < 0.0 { 20.0 } else { 1.0 };
    let impulse = (closing + gain * error as f64) * mass as f64;
    if impulse > 0.0 {
        impulse
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Motion {
    pub position: [f64; 3],
    pub rotation: Quat,
    pub previous_rotation: Quat,
    pub spin: Vec3,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rest {
    Moving,
    Settling,
    Calm,
}

#[derive(Debug, Clone, Copy)]
struct Mark {
    position: Vec3,
    rotation: [f32; 4],
    time: f64,
}

impl Mark {
    fn at(position: [f64; 3], rotation: Quat, time: f64) -> Self {
        Self {
            position: position.map(|v| v as f32),
            rotation: rotation.map(|v| v as f32),
            time,
        }
    }

    fn travel(&self, position: [f64; 3]) -> f64 {
        (0..3)
            .map(|i| (position[i] - self.position[i] as f64).powi(2))
            .sum()
    }
}

fn turn(radius: f32, a: Quat, b: [f32; 4]) -> f64 {
    let d: f64 = (0..4).map(|i| a[i] * b[i] as f64).sum();
    let r = radius as f64;
    2.0 * r * r * (1.0 - d * d)
}

/// Two reference poses, one per sleep window and one held for four seconds.
pub struct SleepCheck {
    recent: Mark,
    held: Mark,
}

impl SleepCheck {
    pub fn new(motion: &Motion, now: f64) -> Self {
        let mark = Mark::at(motion.position, motion.rotation, now);
        Self {
            recent: mark,
            held: mark,
        }
    }

    /// Translated from 0xe0b20 (decompiled, vphysics.so)
    pub fn test(&mut self, m: &Motion, now: f64) -> Rest {
        let moved = self.recent.travel(m.position) > 0.000099999997
            || turn(m.radius, m.rotation, self.recent.rotation) > 0.000024999999;
        if !moved {
            if ((now - self.recent.time) as f32) <= SLEEP_WINDOW {
                return Rest::Settling;
            }
            let limit = (2.356194490192345 / SLEEP_WINDOW as f64) as f32;
            if m.spin.iter().map(|v| v * v).sum::<f32>() <= limit * limit {
                return Rest::Calm;
            }
            let last = m.previous_rotation.map(|v| v as f32);
            if turn(m.radius, m.rotation, last) <= 0.000024999999 {
                return Rest::Calm;
            }
        }
        self.recent = Mark::at(m.position, m.rotation, now);
        if self.held.travel(m.position) > 0.010000000708
            || turn(m.radius, m.previous_rotation, self.held.rotation) > 0.04000000283
        {
            self.held = Mark::at(m.position, m.previous_rotation, now);
        } else if (now - self.held.time) as f32 > 4.0 {
            return Rest::Calm;
        }
        Rest::Moving
    }
}
