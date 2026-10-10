use portal::movement::{
    accelerate, air_accelerate, check_velocity, command, finish_gravity, fling_guard, flinging,
    friction, funnel, jump, leaves_ground, start_gravity, wish, Funnel, ACCELERATE, AIR_ACCELERATE,
    MAX_SPEED, WALK_SPEED,
};
use portal::pair::{through_point, Portal, Vec3};
use portal::teleport::{entered, exit_velocity, touching, view_through, Mover};

use crate::chamber::Chamber;

/// Source's standing hull and eye height
pub const HALF_WIDTH: f32 = 16.0;
pub const HEIGHT: f32 = 72.0;
pub const EYE: f32 = 64.0;
/// the client's m_yaw and m_pitch defaults
const MOUSE_FACTOR: f32 = 0.022;
const PITCH_LIMIT: f32 = 89.0;
/// sv_stepsize's default
const STEP: f32 = 18.0;
/// how far below the feet the ground check looks
const GROUND_REACH: f32 = 2.0;
/// a walk slower than this stops dead
const MIN_WALK: f32 = 1.0;
/// below this an opening's right axis counts as lined up with the world
const TURNED: f32 = 1e-4;
/// how far a box may already sink into a solid and still count as resting on it
const CONTACT: f32 = 0.01;

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
    /// the block stood on, by its index in the chamber
    pub ground: Option<usize>,
    /// m_flFallVelocity, the speed downward before the last landing
    pub fall: f32,
    /// set for the step a jump left the ground on
    pub jumped: bool,
    /// the fall speed, for the step the walker landed on
    pub landed: Option<f32>,
    /// jump held since the last jump or since leaving the ground, so holding it never repeats
    jump_held: bool,
}

impl Walker {
    pub fn at(feet: Vec3) -> Walker {
        Walker {
            feet,
            velocity: [0.0; 3],
            yaw: 0.0,
            pitch: 0.0,
            grounded: false,
            ground: None,
            fall: 0.0,
            jumped: false,
            landed: None,
            jump_held: false,
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

    /// One tick of FullWalkMove, then the portal touch.
    /// Translated from 0x275d90 (decompiled, server.so)
    pub fn step(
        &mut self,
        input: &Input,
        dt: f32,
        chamber: &Chamber,
        linked: Option<[Portal; 2]>,
    ) -> Option<usize> {
        let before = self.centre();
        self.jumped = false;
        self.landed = None;
        if !self.grounded {
            self.fall = -self.velocity[2];
        }
        let max = MAX_SPEED.min(WALK_SPEED);
        let (fmove, smove) = command(input.forward, input.side, max);
        self.velocity[2] = start_gravity(self.velocity[2], dt);
        match input.jump {
            true => self.check_jump(fmove, max, dt),
            false => self.jump_held = false,
        }
        if self.grounded {
            self.velocity[2] = 0.0;
            self.velocity = friction(self.velocity, dt);
        }
        self.velocity = check_velocity(self.velocity);
        match self.grounded {
            true => self.walk_move(fmove, smove, max, dt, chamber, linked.as_ref()),
            false => self.air_move(fmove, smove, max, dt, chamber, linked.as_ref()),
        }
        self.categorize(chamber, linked.as_ref());
        self.velocity = check_velocity(self.velocity);
        self.velocity[2] = finish_gravity(self.velocity[2], dt);
        if self.grounded {
            self.velocity[2] = 0.0;
        }
        if self.grounded && self.fall > 0.0 {
            self.landed = Some(self.fall);
            self.fall = 0.0;
        }
        let pair = linked?;
        let i = (0..2)
            .filter(|&i| touching(&pair[i], before, self.half()))
            .find(|&i| entered(&pair[i], before, self.centre(), self.half()))?;
        self.carry(&pair[i], &pair[1 - i]);
        Some(i)
    }

    /// Translated from 0x2750e0 (decompiled, server.so)
    fn check_jump(&mut self, fmove: f32, max: f32, dt: f32) {
        if self.grounded && !self.jump_held {
            self.velocity = jump(self.velocity, self.yaw, fmove, max);
            self.velocity[2] = finish_gravity(self.velocity[2], dt);
            self.grounded = false;
            self.ground = None;
            self.jumped = true;
        }
        self.jump_held = true;
    }

    /// Translated from 0x274230 (decompiled, server.so)
    fn walk_move(
        &mut self,
        fmove: f32,
        smove: f32,
        max: f32,
        dt: f32,
        chamber: &Chamber,
        linked: Option<&[Portal; 2]>,
    ) {
        let (dir, speed) = direction(wish(self.yaw, fmove, smove));
        self.velocity[2] = 0.0;
        self.velocity = accelerate(self.velocity, dir, speed.min(max), ACCELERATE, dt);
        self.velocity[2] = 0.0;
        if self.velocity[0].hypot(self.velocity[1]) < MIN_WALK {
            self.velocity = [0.0; 3];
            return;
        }
        self.travel(&[0, 1], dt, true, chamber, linked);
        if let Some((n, top)) = self.ground_below(STEP, chamber, linked) {
            self.feet[2] = top;
            self.ground = Some(n);
        }
    }

    /// Translated from 0x2cf2a0 (decompiled, server.so)
    fn air_move(
        &mut self,
        fmove: f32,
        smove: f32,
        max: f32,
        dt: f32,
        chamber: &Chamber,
        linked: Option<&[Portal; 2]>,
    ) {
        let flat = [self.velocity[0], self.velocity[1]];
        let mut want = wish(self.yaw, fmove, smove);
        if flinging(flat) {
            want = fling_guard(flat, want);
        } else {
            for p in linked.into_iter().flatten() {
                match funnel(self.velocity, want, self.feet, self.forward(), p) {
                    Funnel::Wish(w) => want = w,
                    Funnel::Stop => (self.velocity[0], self.velocity[1]) = (0.0, 0.0),
                    Funnel::Off => {}
                }
            }
        }
        let (dir, speed) = direction(want);
        self.velocity = air_accelerate(self.velocity, dir, speed.min(max), AIR_ACCELERATE, dt);
        self.travel(&[0, 1, 2], dt, false, chamber, linked);
    }

    /// Moves along each axis in turn and backs out of what it runs into, a stand-in for the
    /// swept hull trace.
    fn travel(
        &mut self,
        axes: &[usize],
        dt: f32,
        walking: bool,
        chamber: &Chamber,
        linked: Option<&[Portal; 2]>,
    ) {
        for &axis in axes {
            let was = self.feet[axis];
            self.feet[axis] += self.velocity[axis] * dt;
            self.push_out(axis, was, walking, chamber, linked);
        }
    }

    /// On the ground unless rising faster than the ground holds or nothing is 2 units below.
    /// Translated from 0x2d06d0 (decompiled, server.so)
    fn categorize(&mut self, chamber: &Chamber, linked: Option<&[Portal; 2]>) {
        let below = match leaves_ground(self.velocity[2]) {
            true => None,
            false => self.ground_below(GROUND_REACH, chamber, linked),
        };
        self.grounded = below.is_some();
        self.ground = below.map(|(n, _)| n);
        if let Some((_, top)) = below {
            self.feet[2] = top;
        }
    }

    /// The highest block top under the hull within reach below the feet.
    fn ground_below(
        &self,
        reach: f32,
        chamber: &Chamber,
        linked: Option<&[Portal; 2]>,
    ) -> Option<(usize, f32)> {
        let (lo, hi) = self.bounds();
        solids(chamber, linked)
            .into_iter()
            .filter(|(_, min, max)| {
                (0..2).all(|i| lo[i] < max[i] - CONTACT && hi[i] > min[i] + CONTACT)
                    && max[2] <= self.feet[2] + CONTACT
                    && max[2] >= self.feet[2] - reach
            })
            .map(|(n, _, max)| (n, max[2]))
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }

    pub fn centre(&self) -> Vec3 {
        [self.feet[0], self.feet[1], self.feet[2] + HEIGHT / 2.0]
    }

    pub fn half(&self) -> Vec3 {
        [HALF_WIDTH, HALF_WIDTH, HEIGHT / 2.0]
    }

    fn carry(&mut self, from: &Portal, to: &Portal) {
        let c = through_point(from, to, self.centre());
        self.feet = [c[0], c[1], c[2] - HEIGHT / 2.0];
        self.velocity = exit_velocity(from, to, self.velocity, Mover::Player);
        self.grounded = false;
        self.ground = None;
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

    /// Back out of each solid on the side the walker came from along the axis, or the nearer
    /// side when it was already inside. Only a walk steps up onto what it meets.
    fn push_out(
        &mut self,
        axis: usize,
        was: f32,
        walking: bool,
        chamber: &Chamber,
        linked: Option<&[Portal; 2]>,
    ) {
        for (_, min, max) in solids(chamber, linked) {
            let (lo, hi) = self.bounds();
            let overlaps = (0..3).all(|i| lo[i] < max[i] - CONTACT && hi[i] > min[i] + CONTACT);
            if !overlaps {
                continue;
            }
            let rise = max[2] - self.feet[2];
            if walking && axis != 2 && rise > 0.0 && rise <= STEP {
                self.feet[2] = max[2];
                continue;
            }
            let moved = self.feet[axis] - was;
            let from_below = hi[axis] - moved <= min[axis] + CONTACT;
            let from_above = lo[axis] - moved >= max[axis] - CONTACT;
            let a = match (from_below, from_above) {
                (false, false) => (0..3)
                    .min_by(|&a, &b| {
                        let depth = |i: usize| (hi[i] - min[i]).min(max[i] - lo[i]);
                        depth(a).total_cmp(&depth(b))
                    })
                    .unwrap_or(axis),
                _ => axis,
            };
            let (down, up) = (hi[a] - min[a], max[a] - lo[a]);
            if from_below || (!from_above && down < up) {
                self.feet[a] -= down;
            } else {
                self.feet[a] += up;
            }
            self.velocity[a] = 0.0;
        }
    }
}

/// A wish split into its direction and speed.
fn direction(w: [f32; 2]) -> (Vec3, f32) {
    let speed = w[0].hypot(w[1]);
    match speed > 0.0 {
        true => ([w[0] / speed, w[1] / speed, 0.0], speed),
        false => ([0.0; 3], 0.0),
    }
}

/// The chamber's blocks by index. A linked portal carves its opening through the block it sits
/// on, a stand-in for the collision the game builds behind each portal.
fn solids(chamber: &Chamber, linked: Option<&[Portal; 2]>) -> Vec<(usize, Vec3, Vec3)> {
    chamber
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(n, block)| {
            let on = linked.into_iter().flatten().filter(|p| {
                let q = [0, 1, 2].map(|i| p.origin[i] - p.forward[i]);
                (0..3).all(|i| q[i] >= block.min[i] && q[i] <= block.max[i])
            });
            on.fold(vec![(block.min, block.max)], |pieces, p| {
                pieces
                    .into_iter()
                    .flat_map(|(lo, hi)| carve(lo, hi, p))
                    .collect::<Vec<_>>()
            })
            .into_iter()
            .map(move |(lo, hi)| (n, lo, hi))
        })
        .collect()
}

/// A block less a box of a portal's opening cut right through it along the portal's normal.
/// An opening turned in its plane gets the largest square that fits inside it.
fn carve(min: Vec3, max: Vec3, p: &Portal) -> Vec<(Vec3, Vec3)> {
    let normal = (0..3)
        .max_by(|&a, &b| p.forward[a].abs().total_cmp(&p.forward[b].abs()))
        .unwrap_or(0);
    let turn: Vec<f32> = (0..3)
        .filter(|&i| i != normal)
        .map(|i| p.right[i].abs())
        .collect();
    let reach = match turn.iter().copied().fold(1.0, f32::min) < TURNED {
        true => [0, 1, 2].map(|i| p.right[i].abs() * p.half.0 + p.up[i].abs() * p.half.1),
        false => [p.half.0.min(p.half.1) / turn.iter().sum::<f32>(); 3],
    };
    let mut cut_lo = [0, 1, 2].map(|i| p.origin[i] - reach[i]);
    let mut cut_hi = [0, 1, 2].map(|i| p.origin[i] + reach[i]);
    (cut_lo[normal], cut_hi[normal]) = (min[normal], max[normal]);
    let (mut lo, mut hi) = (min, max);
    let mut pieces = Vec::new();
    for a in 0..3 {
        if cut_lo[a] > lo[a] {
            let mut top = hi;
            top[a] = cut_lo[a].min(hi[a]);
            pieces.push((lo, top));
            lo[a] = top[a];
        }
        if cut_hi[a] < hi[a] {
            let mut bottom = lo;
            bottom[a] = cut_hi[a].max(lo[a]);
            pieces.push((bottom, hi));
            hi[a] = bottom[a];
        }
    }
    pieces
}
