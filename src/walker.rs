use portal::pair::Vec3;

use crate::chamber::Chamber;

/// Source's standing hull and eye height
pub const HALF_WIDTH: f32 = 16.0;
pub const HEIGHT: f32 = 72.0;
pub const EYE: f32 = 64.0;
/// the client's m_yaw and m_pitch defaults
const MOUSE_FACTOR: f32 = 0.022;
const PITCH_LIMIT: f32 = 89.0;
/// stand-in movement until Portal's own is ported
const SPEED: f32 = 175.0;
const GRAVITY: f32 = 600.0;
const JUMP_SPEED: f32 = 268.3;

#[derive(Default)]
pub struct Input {
    pub forward: f32,
    pub side: f32,
    pub jump: bool,
}

pub struct Walker {
    pub feet: Vec3,
    pub velocity: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub grounded: bool,
}

impl Walker {
    pub fn at(feet: Vec3) -> Walker {
        Walker {
            feet,
            velocity: [0.0; 3],
            yaw: 0.0,
            pitch: 0.0,
            grounded: false,
        }
    }

    pub fn eye(&self) -> Vec3 {
        [self.feet[0], self.feet[1], self.feet[2] + EYE]
    }

    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        [cp * cy, cp * sy, -sp]
    }

    /// AngleVectors with no roll
    pub fn right(&self) -> Vec3 {
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        [sy, -cy, 0.0]
    }

    pub fn up(&self) -> Vec3 {
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        [sp * cy, sp * sy, cp]
    }

    /// Mouse counts, right and down positive, as Source turns them.
    pub fn look(&mut self, dx: f32, dy: f32, sensitivity: f32) {
        self.yaw -= dx * sensitivity * MOUSE_FACTOR;
        self.pitch =
            (self.pitch + dy * sensitivity * MOUSE_FACTOR).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    pub fn step(&mut self, input: &Input, dt: f32, chamber: &Chamber) {
        let (sin, cos) = self.yaw.to_radians().sin_cos();
        let wish = [
            cos * input.forward + sin * input.side,
            sin * input.forward - cos * input.side,
        ];
        let len = wish[0].hypot(wish[1]);
        let scale = if len > 0.0 { SPEED / len } else { 0.0 };
        self.velocity[0] = wish[0] * scale;
        self.velocity[1] = wish[1] * scale;
        if input.jump && self.grounded {
            self.velocity[2] = JUMP_SPEED;
        }
        self.velocity[2] -= GRAVITY * dt;
        self.grounded = false;
        for axis in 0..3 {
            self.feet[axis] += self.velocity[axis] * dt;
            self.push_out(axis, chamber);
        }
    }

    fn bounds(&self) -> (Vec3, Vec3) {
        let [x, y, z] = self.feet;
        (
            [x - HALF_WIDTH, y - HALF_WIDTH, z],
            [x + HALF_WIDTH, y + HALF_WIDTH, z + HEIGHT],
        )
    }

    fn push_out(&mut self, axis: usize, chamber: &Chamber) {
        for block in &chamber.blocks {
            let (lo, hi) = self.bounds();
            let overlaps = (0..3).all(|i| lo[i] < block.max[i] && hi[i] > block.min[i]);
            if !overlaps {
                continue;
            }
            if self.velocity[axis] > 0.0 {
                self.feet[axis] -= hi[axis] - block.min[axis];
            } else {
                self.feet[axis] += block.max[axis] - lo[axis];
                self.grounded |= axis == 2;
            }
            self.velocity[axis] = 0.0;
        }
    }
}
