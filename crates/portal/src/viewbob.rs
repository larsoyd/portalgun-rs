use std::f64::consts::{PI, TAU};

use crate::pair::Vec3;

const BOB_SPEED: f32 = 320.0;
const BOB_RATE: f32 = 0.003125;
const BOB_CYCLE: f32 = 0.45;
const BOB_UP: f32 = 0.5;
const BOB_SCALE: f32 = 0.005;
const BOB_BASE: f64 = 0.3;
const BOB_SINE: f64 = 0.7;
const BOB_MIN: f32 = -7.0;
const BOB_MAX: f32 = 4.0;
const LAG_SPEED: f32 = 5.0;
const LAG_PUSH: [f32; 3] = [0.035, 0.03, 0.02];
const DOWN_PUSH: [f32; 3] = [5.0, 4.7, 4.0];

/// Source angles in degrees, positive pitch looks down.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Angles {
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
}

/// AngleVectors: forward, right and up.
pub fn angle_vectors(a: Angles) -> (Vec3, Vec3, Vec3) {
    let (sp, cp) = a.pitch.to_radians().sin_cos();
    let (sy, cy) = a.yaw.to_radians().sin_cos();
    let (sr, cr) = a.roll.to_radians().sin_cos();
    (
        [cp * cy, cp * sy, -sp],
        [-sr * sp * cy + cr * sy, -sr * sp * sy - cr * cy, -sr * cp],
        [cr * sp * cy + sr * sy, cr * sp * sy - sr * cy, cr * cp],
    )
}

fn along(o: Vec3, d: Vec3, s: f32) -> Vec3 {
    [0, 1, 2].map(|i| o[i] + d[i] * s)
}

/// The bob clock and the last offsets, shared by every weapon.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Bob {
    pub time: f32,
    pub last: f32,
    pub vertical: f32,
    pub lateral: f32,
}

/// One half of the bob: the phase within its cycle and the sine argument.
fn wave(time: f32, cycles: f32, period: f32, norm: f32, split: f32) -> f32 {
    let c = time - ((time * cycles).trunc() as i32) as f32 * period;
    let x = norm * c;
    match c < split {
        true => (x as f64 * TAU) as f32,
        false => ((x - BOB_UP) as f64 * TAU + PI) as f32,
    }
}

impl Bob {
    /// Translated from 0x284170 (disassembled, client.so)
    pub fn calc(&mut self, velocity: Vec3, now: f32, frametime: f32) {
        if frametime == 0.0 {
            return;
        }
        let speed = (velocity[0] * velocity[0] + velocity[1] * velocity[1])
            .sqrt()
            .clamp(-BOB_SPEED, BOB_SPEED);
        self.time += (now - self.last) * BOB_RATE * speed;
        self.last = now;
        let v = speed * BOB_SCALE;
        let shape = |arg: f32| {
            ((arg.sin() as f64 * (v as f64 * BOB_SINE) + v as f64 * BOB_BASE) as f32)
                .clamp(BOB_MIN, BOB_MAX)
        };
        let up = 1.0 / BOB_CYCLE;
        self.vertical = shape(wave(self.time, up, BOB_CYCLE, up, BOB_CYCLE * BOB_UP));
        let side = 2.0 * BOB_CYCLE;
        self.lateral = shape(wave(self.time, 2.0 * up, side, 1.0 / side, BOB_CYCLE));
    }
}

/// Translated from 0x284480 (disassembled, client.so)
pub fn add_bob(origin: Vec3, angles: Angles, vertical: f32, lateral: f32) -> (Vec3, Angles) {
    let (f, r, _) = angle_vectors(angles);
    let mut o = along(origin, f, vertical * 0.1);
    o[2] += vertical * 0.1;
    let a = Angles {
        pitch: angles.pitch - vertical * 0.4,
        yaw: angles.yaw - lateral * 0.3,
        roll: angles.roll + vertical * 0.5,
    };
    (along(o, r, lateral * 0.8), a)
}

/// The view model's trailing facing, m_vecLastFacing.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Lag {
    pub facing: Vec3,
}

impl Lag {
    pub fn reset(&mut self, angles: Angles) {
        self.facing = angle_vectors(angles).0;
    }

    /// Translated from 0x20e540 (disassembled, client.so). max is 0 while touching a portal.
    pub fn apply(
        &mut self,
        origin: &mut Vec3,
        angles: &mut Angles,
        original: Angles,
        max: f32,
        frametime: f32,
    ) {
        let (orig_o, orig_a) = (*origin, *angles);
        let (forward, _, _) = angle_vectors(*angles);
        if frametime != 0.0 {
            let d = [0, 1, 2].map(|i| forward[i] - self.facing[i]);
            let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            let speed = match max > 0.0 && len > max {
                true => len / max * LAG_SPEED,
                false => LAG_SPEED,
            };
            let old = self.facing;
            let moved = along(self.facing, d, speed * frametime);
            let n = 1.0 / (moved.iter().map(|v| v * v).sum::<f32>() + 1e-10).sqrt();
            self.facing = moved.map(|v| v * n);
            *origin = [0, 1, 2].map(|i| origin[i] + (old[i] - forward[i]) * LAG_SPEED);
        }
        let (f, r, u) = angle_vectors(original);
        let p = match original.pitch {
            p if p > 180.0 => p - 360.0,
            p if p < -180.0 => p + 360.0,
            p => p,
        };
        if max == 0.0 {
            (*origin, *angles) = (orig_o, orig_a);
        }
        let push = LAG_PUSH.map(|k| -p * k);
        *origin = along(along(along(*origin, f, push[0]), r, push[1]), u, push[2]);
    }
}

/// Translated from 0x53cca0 (disassembled, client.so). Looking down moves the gun out of the way.
pub fn look_down(eye: Vec3, angles: Angles) -> Vec3 {
    let (f, r, u) = angle_vectors(angles);
    if f[2] >= 0.0 {
        return eye;
    }
    let k = f[2] * f[2];
    along(
        along(along(eye, f, DOWN_PUSH[0] * k), r, DOWN_PUSH[1] * k),
        u,
        DOWN_PUSH[2] * k,
    )
}
