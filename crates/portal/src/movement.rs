// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::pair::{Portal, Vec3};

/// sv_gravity, sv_friction, sv_stopspeed, sv_accelerate, sv_maxspeed, sv_maxvelocity defaults
pub const GRAVITY: f32 = 600.0;
pub const FRICTION: f32 = 4.0;
pub const STOP_SPEED: f32 = 100.0;
pub const ACCELERATE: f32 = 10.0;
pub const MAX_SPEED: f32 = 320.0;
pub const MAX_VELOCITY: f32 = 3500.0;
/// Portal's air move passes a literal instead of sv_airaccelerate
pub const AIR_ACCELERATE: f32 = 15.0;
pub const AIR_SPEED_CAP: f32 = 60.0;
/// hl2_walkspeed, also measured on the pinned Portal player
pub const WALK_SPEED: f32 = 150.0;
/// cl_forwardspeed, cl_backspeed and cl_sidespeed
pub const KEY_SPEED: f32 = 450.0;
pub const JUMP_SPEED: f32 = 160.0;
/// above this upward speed the ground lets go
const NON_JUMP_VELOCITY: f32 = 140.0;
const MIN_FLING_SPEED: f32 = 300.0;
const STOP_EPSILON: f32 = 0.1;
const JUMP_BOOST: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Funnel {
    Off,
    /// right over the portal, the horizontal speed is dropped
    Stop,
    Wish([f32; 2]),
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn flat(v: Vec3) -> Vec3 {
    let len = v[0].hypot(v[1]);
    match len > 0.0 {
        true => [v[0] / len, v[1] / len, 0.0],
        false => [0.0; 3],
    }
}

/// Keys in -1..1 to forward and side move, cut to the max speed as CheckParameters does.
/// Translated from 0x273970 (decompiled, server.so)
pub fn command(forward: f32, side: f32, max: f32) -> (f32, f32) {
    let (f, s) = (forward * KEY_SPEED, side * KEY_SPEED);
    let spd = f * f + s * s;
    match spd != 0.0 && spd > max * max {
        true => (f * max / spd.sqrt(), s * max / spd.sqrt()),
        false => (f, s),
    }
}

/// The wanted velocity, from forward and right with their height dropped.
pub fn wish(yaw: f32, fmove: f32, smove: f32) -> [f32; 2] {
    let (sy, cy) = yaw.to_radians().sin_cos();
    [cy * fmove + sy * smove, sy * fmove - cy * smove]
}

/// Ground friction. Translated from 0x273eb0 (decompiled, server.so)
pub fn friction(v: Vec3, dt: f32) -> Vec3 {
    surface_friction(v, dt, 1.0)
}

pub fn surface_friction(v: Vec3, dt: f32, factor: f32) -> Vec3 {
    let speed = dot(v, v).sqrt();
    if speed < STOP_EPSILON {
        return v;
    }
    let drop = speed.max(STOP_SPEED) * factor * FRICTION * dt;
    let new = (speed - drop).max(0.0);
    match new != speed {
        true => v.map(|c| c * (new / speed)),
        false => v,
    }
}

/// Translated from 0x26d890 (decompiled, server.so)
pub fn accelerate(v: Vec3, dir: Vec3, speed: f32, accel: f32, dt: f32) -> Vec3 {
    push(v, dir, speed, speed, accel, dt)
}

/// The air wish is capped at 60 but the push still scales with the whole wish.
/// Translated from 0x2ce0b0 (decompiled, server.so)
pub fn air_accelerate(v: Vec3, dir: Vec3, speed: f32, accel: f32, dt: f32) -> Vec3 {
    push(v, dir, speed.min(AIR_SPEED_CAP), speed, accel, dt)
}

fn push(v: Vec3, dir: Vec3, target: f32, speed: f32, accel: f32, dt: f32) -> Vec3 {
    let add = target - dot(v, dir);
    if add <= 0.0 {
        return v;
    }
    let step = (accel * dt * speed).min(add);
    [0, 1, 2].map(|i| v[i] + step * dir[i])
}

/// Translated from 0x2cf2a0 (decompiled, server.so)
pub fn flinging(v: [f32; 2]) -> bool {
    v[0] * v[0] + v[1] * v[1] > MIN_FLING_SPEED * MIN_FLING_SPEED
}

/// Steering against a fling is dropped axis by axis so the fling survives.
/// Translated from 0x2cf2a0 (decompiled, server.so)
pub fn fling_guard(v: [f32; 2], mut wish: [f32; 2]) -> [f32; 2] {
    if !flinging(v) {
        return wish;
    }
    let half = MIN_FLING_SPEED * 0.5;
    for i in 0..2 {
        if (v[i] > half && wish[i] < 0.0) || (v[i] < -half && wish[i] > 0.0) {
            wish[i] = 0.0;
        }
    }
    wish
}

/// sv_player_funnel_into_portals: a player falling at a floor portal is steered to its middle.
/// Translated from 0x2cef30 (decompiled, server.so)
pub fn funnel(v: Vec3, wish: [f32; 2], feet: Vec3, eye: Vec3, p: &Portal) -> Funnel {
    if p.forward[2] < 0.8 || eye[2] > -0.1 {
        return Funnel::Off;
    }
    let d = [0, 1, 2].map(|i| p.origin[i] - feet[i]);
    let along = |axis: Vec3| {
        let k = dot(d, axis);
        (axis[0] * k).powi(2) + (axis[1] * k).powi(2)
    };
    if wish[0].abs() > 64.0
        || wish[1].abs() > 64.0
        || v[2] > -165.0
        || d[2] < -512.0
        || along(flat(p.right)) > 2304.0
        || along(flat(p.up)) > 6561.0005
    {
        return Funnel::Off;
    }
    if d[2] > -8.0 {
        return Funnel::Stop;
    }
    Funnel::Wish([wish[0] - v[0] + d[0] * 6.0, wish[1] - v[1] + d[1] * 6.0])
}

/// Half of a tick's gravity before the move, in double, then CheckVelocity.
/// Translated from 0x275c40 (decompiled, server.so)
pub fn start_gravity(vz: f32, dt: f32) -> f32 {
    clamp_speed((vz as f64 - GRAVITY as f64 * (dt as f64 * 0.5)) as f32)
}

/// Translated from 0x275040 (decompiled, server.so)
pub fn finish_gravity(vz: f32, dt: f32) -> f32 {
    clamp_speed((vz as f64 - (GRAVITY * dt) as f64 * 0.5) as f32)
}

/// The jump and the single player forward boost, which brakes above one and a half times the
/// max speed and speeds up a backwards jump. Translated from 0x2750e0 (decompiled, server.so)
pub fn jump(v: Vec3, yaw: f32, fmove: f32, max: f32) -> Vec3 {
    jump_on_surface(v, yaw, fmove, max, false, 1.0)
}

pub fn jump_on_surface(v: Vec3, yaw: f32, fmove: f32, max: f32, ducked: bool, factor: f32) -> Vec3 {
    let boost = if ducked { 0.1 } else { JUMP_BOOST };
    let (sy, cy) = yaw.to_radians().sin_cos();
    let top = max + max * boost;
    let mut add = (fmove * boost).abs();
    let new = add + v[0].hypot(v[1]);
    if new > top {
        add -= new - top;
    }
    if fmove < 0.0 {
        add = -add;
    }
    [
        v[0] + cy * add,
        v[1] + sy * add,
        if ducked {
            factor * JUMP_SPEED
        } else {
            v[2] + factor * JUMP_SPEED
        },
    ]
}

/// Translated from 0x2d06d0 (decompiled, server.so)
pub fn leaves_ground(vz: f32) -> bool {
    vz > NON_JUMP_VELOCITY
}

/// Translated from 0x274e40 (decompiled, server.so)
pub fn check_velocity(v: Vec3) -> Vec3 {
    v.map(clamp_speed)
}

fn clamp_speed(c: f32) -> f32 {
    match c.is_finite() {
        true => c.clamp(-MAX_VELOCITY, MAX_VELOCITY),
        false => 0.0,
    }
}

/// m_vecPunchAngle and its velocity.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Punch {
    pub angle: Vec3,
    pub velocity: Vec3,
}

impl Punch {
    /// Translated from 0x246190 (decompiled, server.so), with sv_suppress_viewpunch at its default of 0
    pub fn kick(&mut self, angle: Vec3) {
        self.velocity = [0, 1, 2].map(|i| self.velocity[i] + angle[i] * 20.0);
    }

    /// A damped spring back to zero, once per movement tick.
    /// Translated from 0x26f6e0 (decompiled, server.so)
    pub fn decay(&mut self, dt: f32) {
        let length = |v: Vec3| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]) as f64;
        if length(self.angle) <= 0.001 && length(self.velocity) <= 0.001 {
            *self = Self::default();
            return;
        }
        let damping = if dt <= 1.0 / 9.0 { 1.0 - 9.0 * dt } else { 0.0 };
        let spring = (65.0 * dt).clamp(0.0, 2.0);
        for i in 0..3 {
            self.angle[i] += self.velocity[i] * dt;
            self.velocity[i] = self.velocity[i] * damping - spring * self.angle[i];
        }
        let limits = [89.0, 179.0, 89.0];
        self.angle = [0, 1, 2].map(|i| self.angle[i].clamp(-limits[i], limits[i]));
    }

    /// PlayerRoughLandingEffects sets the roll outright and caps the pitch.
    /// Translated from 0x26f5a0 (disassembled, server.so)
    pub fn land(&mut self, fall: f32) {
        self.angle[2] = (fall as f64 * 0.013) as f32;
        self.angle[0] = self.angle[0].min(8.0);
    }
}

/// Whether a plane is ground, against the double 0.7 the original compares with.
pub fn standable(normal_z: f32) -> bool {
    normal_z as f64 >= 0.7
}
