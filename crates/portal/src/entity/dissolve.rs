// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::pair::Vec3;

// dissolver spawn stores these for type 0, server.so 0x4f7670
const FADE_IN_START: f32 = 0.0;
const FADE_IN_LENGTH: f32 = 1.0;
const FADE_OUT_MODEL_START: f32 = 1.9;
const FADE_OUT_MODEL_LENGTH: f32 = f32::from_bits(0x3dcc_ccd0);
const FADE_OUT_START: f32 = 2.0;
const FADE_OUT_LENGTH: f32 = 0.0;
// client.so constants at 0x7f2f28, 0x7f2f30, 0x807fe0 times 0.5, 0x7f29b0, 0x7f2e04, 0x7f329c
const NEAR: f32 = 8.0;
const SWEEP: f32 = 32.0;
const SPREAD: f32 = 0.375;
const MIN_SITE: f32 = 5.0;
const PUFF_LIFE: f32 = 0.1;
const MIN_ALPHA: f32 = 0.001;
// the emitter's near clip band, set where 0x5355f0 makes it
const NEAR_FADE: [f32; 2] = [16.0, 64.0];

/// Translated from 0x3931b0 (decompiled, client.so), a dissolving model colour dt seconds in
pub fn colour(dt: f32) -> [u8; 4] {
    let fade = if dt > FADE_OUT_START {
        0.0
    } else if dt < FADE_IN_START {
        1.0
    } else if dt > FADE_IN_START && dt < FADE_IN_START + FADE_IN_LENGTH {
        1.0 - (dt - FADE_IN_START) / FADE_IN_LENGTH
    } else {
        0.0
    };
    let rgb = (255.0 * fade) as u8;
    let out = dt - FADE_OUT_MODEL_START;
    let alpha = if dt <= FADE_OUT_MODEL_START {
        255
    } else if out <= FADE_OUT_MODEL_LENGTH {
        (255.0 * (1.0 - out / FADE_OUT_MODEL_LENGTH)) as u8
    } else {
        0
    };
    [rgb, rgb, rgb, alpha]
}

pub trait Draw {
    fn float(&mut self, lo: f32, hi: f32) -> f32;
    fn int(&mut self, lo: i32, hi: i32) -> i32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sprite {
    Spark,
    Muzzle,
}

/// A CSimpleEmitter particle as the dissolver adds it, white, alpha 255 to 0 and size to 0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spark {
    pub sprite: Sprite,
    pub pos: Vec3,
    pub vel: Vec3,
    pub roll: f32,
    pub roll_rate: f32,
    pub life: f32,
    pub die: f32,
    pub start_size: u8,
}

impl Spark {
    /// Translated from 0x533ee0 (decompiled, client.so)
    pub fn alpha(&self) -> f32 {
        255.0 * (1.0 - self.life / self.die) / 255.0
    }

    /// Translated from 0x533f40 (decompiled, client.so)
    pub fn size(&self) -> f32 {
        self.start_size as f32 * (1.0 - self.life / self.die)
    }
}

/// Translated from 0x3942d0 (decompiled, client.so), the two box axes least facing the eye
pub fn site(centre: Vec3, axes: [Vec3; 3], extent: Vec3, eye: Vec3) -> [Vec3; 2] {
    let view = unit(sub(eye, centre));
    let facing = axes.map(|a| dot(a, view).abs());
    let wide = usize::from(facing[0] <= facing[1]);
    let (a, b, hidden) = if facing[wide] <= facing[2] && facing[2] != facing[wide] {
        (0, 1, 2)
    } else if facing[0] <= facing[1] {
        (2, 0, 1)
    } else {
        (1, 2, 0)
    };
    let flat = |axis: Vec3| unit(sub(axis, scale(view, dot(axis, view))));
    let (n1, n2) = (flat(axes[a]), flat(axes[b]));
    let across = |n: Vec3, own: usize| {
        (dot(axes[own], n).abs() * extent[own] + dot(axes[hidden], n).abs() * extent[hidden])
            .max(MIN_SITE)
    };
    [scale(n1, across(n1, a)), scale(n2, across(n2, b))]
}

/// Translated from 0x394c70 (decompiled, client.so), what one draw of a dissolving model adds
pub fn emit(
    dt: f32,
    sites: &[(Vec3, [Vec3; 2])],
    forward: Vec3,
    draw: &mut impl Draw,
) -> Vec<Spark> {
    let (fade, offset) = if dt > FADE_OUT_START {
        let out = dt - FADE_OUT_START;
        if out <= FADE_OUT_LENGTH {
            let out = out / FADE_OUT_LENGTH;
            (1.0 - out, NEAR - SWEEP * out)
        } else {
            (0.0, NEAR - SWEEP)
        }
    } else if dt < FADE_IN_START {
        (0.0, NEAR - SWEEP)
    } else if dt <= FADE_IN_START || dt >= FADE_IN_START + FADE_IN_LENGTH {
        (1.0, NEAR)
    } else {
        let f = (dt - FADE_IN_START) / FADE_IN_LENGTH;
        (f, SWEEP * (f - 1.0) + NEAR)
    };
    let scale_by = (dt / FADE_OUT_LENGTH).clamp(0.75, 1.0);
    let puffs = (fade * 3.0).clamp(0.0, 3.0) as i32;
    let size = |x: f32| (x as i32) as u8;
    let mut made = Vec::new();
    for &(centre, [a1, a2]) in sites {
        let (l1, l2) = (dot(a1, a1).sqrt(), dot(a2, a2).sqrt());
        let (n1, n2) = (scale(a1, 1.0 / l1), scale(a2, 1.0 / l2));
        let spot = |draw: &mut dyn FnMut(f32, f32) -> f32| {
            let r2 = draw(-SPREAD * l2, SPREAD * l2);
            let r1 = draw(-SPREAD * l1, SPREAD * l1);
            add(
                add(add(centre, scale(forward, offset)), scale(n2, r2)),
                scale(n1, r1),
            )
        };
        for _ in 0..2 {
            let pos = spot(&mut |lo, hi| draw.float(lo, hi));
            if draw.int(0, 2) != 0 {
                continue;
            }
            let z = draw.float(16.0, 64.0);
            let vel = [draw.float(-4.0, 4.0), draw.float(-4.0, 4.0), z];
            let mut start_size = if z <= 0.0 {
                size(scale_by * 2.0)
            } else {
                size(draw.float(4.0, 6.0) * scale_by)
            };
            let mut die = draw.float(0.4, 0.5);
            let roll_rate = if puffs == 0 {
                die *= 2.0;
                start_size = size(scale_by * 2.0);
                draw.float(-4.0, 4.0)
            } else {
                draw.float(-8.0, 8.0)
            };
            let roll = draw.int(0, 360) as f32;
            made.push(Spark {
                sprite: Sprite::Spark,
                pos,
                vel,
                roll,
                roll_rate,
                life: 0.0,
                die,
                start_size,
            });
        }
        for _ in 0..puffs {
            let pos = spot(&mut |lo, hi| draw.float(lo, hi));
            let z = draw.float(-64.0, 128.0);
            let vel = [draw.float(-4.0, 4.0), draw.float(-4.0, 4.0), z];
            let start_size = size(draw.float(8.0, 12.0) * scale_by);
            let roll = draw.int(0, 360) as f32;
            let roll_rate = draw.float(-2.0, 2.0);
            made.push(Spark {
                sprite: Sprite::Muzzle,
                pos,
                vel,
                roll,
                roll_rate,
                life: 0.0,
                die: PUFF_LIFE,
                start_size,
            });
        }
    }
    made
}

/// Translated from 0x534570 (decompiled, client.so), particles coast and go at their die time
pub fn step(sparks: &mut Vec<Spark>, dt: f32) {
    for s in sparks.iter_mut() {
        s.pos = add(s.pos, scale(s.vel, dt));
        s.life += dt;
        s.roll += s.roll_rate * dt;
    }
    sparks.retain(|s| s.life < s.die);
}

/// 0x5346d0, particles fade out between 64 and 16 units in front of the eye and are culled under 0.001
pub fn near_fade(alpha: f32, depth: f32) -> f32 {
    let alpha = if depth > NEAR_FADE[1] {
        alpha
    } else if depth > NEAR_FADE[0] {
        alpha * (depth - NEAR_FADE[0]) / (NEAR_FADE[1] - NEAR_FADE[0])
    } else {
        0.0
    };
    if alpha >= MIN_ALPHA {
        alpha
    } else {
        0.0
    }
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| a[i] + b[i])
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| a[i] - b[i])
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    (0..3).map(|i| a[i] * b[i]).sum()
}

fn scale(v: Vec3, f: f32) -> Vec3 {
    v.map(|c| c * f)
}

fn unit(v: Vec3) -> Vec3 {
    scale(v, 1.0 / dot(v, v).sqrt())
}
