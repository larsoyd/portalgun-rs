use portal::pair::{through_point, Portal, Vec3};
use portal::teleport::{entered, exit_velocity, meets_opening, view_through, Mover};

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

    pub fn step(
        &mut self,
        input: &Input,
        dt: f32,
        chamber: &Chamber,
        linked: Option<[Portal; 2]>,
    ) -> Option<usize> {
        let before = self.centre();
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
            self.push_out(axis, chamber, linked.as_ref());
        }
        let pair = linked?;
        let i = (0..2).find(|&i| entered(&pair[i], before, self.centre(), self.half()))?;
        self.carry(&pair[i], &pair[1 - i]);
        Some(i)
    }

    fn centre(&self) -> Vec3 {
        [self.feet[0], self.feet[1], self.feet[2] + HEIGHT / 2.0]
    }

    fn half(&self) -> Vec3 {
        [HALF_WIDTH, HALF_WIDTH, HEIGHT / 2.0]
    }

    fn carry(&mut self, from: &Portal, to: &Portal) {
        let c = through_point(from, to, self.centre());
        self.feet = [c[0], c[1], c[2] - HEIGHT / 2.0];
        self.velocity = exit_velocity(from, to, self.velocity, Mover::Player);
        let (f, _) = view_through(from, to, self.forward(), self.up());
        self.yaw = f[1].atan2(f[0]).to_degrees();
        self.pitch = -f[2].clamp(-1.0, 1.0).asin().to_degrees();
    }

    fn bounds(&self) -> (Vec3, Vec3) {
        let [x, y, z] = self.feet;
        (
            [x - HALF_WIDTH, y - HALF_WIDTH, z],
            [x + HALF_WIDTH, y + HALF_WIDTH, z + HEIGHT],
        )
    }

    /// The surface a portal sits on lets the walker into the opening, a stand-in for the carved
    /// collision the game builds behind each portal.
    fn push_out(&mut self, axis: usize, chamber: &Chamber, linked: Option<&[Portal; 2]>) {
        let (centre, half) = (self.centre(), self.half());
        let open: Vec<&Portal> = linked
            .into_iter()
            .flatten()
            .filter(|p| meets_opening(p, centre, half))
            .collect();
        for block in &chamber.blocks {
            let behind = |p: &&Portal| {
                let q = [0, 1, 2].map(|i| p.origin[i] - p.forward[i]);
                (0..3).all(|i| q[i] >= block.min[i] && q[i] <= block.max[i])
            };
            if open.iter().any(|p| behind(p)) {
                continue;
            }
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
