use portal::movement::{
    accelerate, air_accelerate, check_velocity, command, finish_gravity, fling_guard, flinging,
    funnel, jump_on_surface, leaves_ground, start_gravity, surface_friction, wish, Funnel,
    ACCELERATE, AIR_ACCELERATE, MAX_SPEED, WALK_SPEED,
};
use portal::pair::{crossing, through_point, Portal, Vec3};
use portal::teleport::{entered, exit_velocity, touching, view_through, Mover};

use crate::chamber::Chamber;
use crate::collision::{self, Trace};
use portal::surfaces::Surface;

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

#[derive(Default)]
pub struct Input {
    pub forward: f32,
    pub side: f32,
    pub jump: bool,
    pub jump_pressed: bool,
    pub crouch: bool,
}

#[derive(Debug, Clone)]
pub struct Duck {
    pub ducked: bool,
    pub ducking: bool,
    pub in_duck_jump: bool,
    pub eye: f32,
    timer: f32,
    jump_timer: f32,
    eye_timer: f32,
    held: bool,
}

impl Default for Duck {
    fn default() -> Self {
        Self {
            ducked: false,
            ducking: false,
            in_duck_jump: false,
            eye: EYE,
            timer: 0.0,
            jump_timer: 0.0,
            eye_timer: 0.0,
            held: false,
        }
    }
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
    pub duck: Duck,
    surfaces: Vec<Surface>,
    surface_friction: f32,
    surface: Surface,
    categorize_on_spawn: bool,
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
            duck: Duck::default(),
            surfaces: Vec::new(),
            surface_friction: 1.0,
            surface: Surface::default(),
            categorize_on_spawn: true,
        }
    }

    pub fn set_surfaces(&mut self, surfaces: Vec<Surface>) {
        self.surfaces = surfaces;
    }

    pub fn eye(&self) -> Vec3 {
        [self.feet[0], self.feet[1], self.feet[2] + self.duck.eye]
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
        if self.categorize_on_spawn {
            self.categorize(chamber, linked.as_ref());
            self.categorize_on_spawn = false;
        }
        if self.velocity[2] > 250.0 {
            self.grounded = false;
            self.ground = None;
        }
        self.jumped = false;
        self.landed = None;
        if !self.grounded {
            self.fall = -self.velocity[2];
        }
        let max = MAX_SPEED.min(WALK_SPEED) * self.surface.max_speed_factor.min(1.0);
        let (mut fmove, mut smove) = command(input.forward, input.side, max);
        if self.duck.ducked && self.grounded {
            fmove *= 1.0 / 3.0;
            smove *= 1.0 / 3.0;
        }
        self.update_duck(input.crouch, dt, chamber, linked.as_ref());
        let before = self.centre();
        self.velocity[2] = start_gravity(self.velocity[2], dt);
        if input.jump_pressed {
            self.jump_held = false;
        }
        match input.jump {
            true => self.check_jump(fmove, max, dt),
            false => self.jump_held = false,
        }
        if self.grounded {
            self.velocity[2] = 0.0;
            self.velocity = surface_friction(self.velocity, dt, self.surface_friction);
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
        self.carry(&pair[i], &pair[1 - i], before, chamber, &pair)
            .then_some(i)
    }

    /// Translated from 0x2750e0 (decompiled, server.so)
    fn check_jump(&mut self, fmove: f32, max: f32, dt: f32) {
        if !self.grounded {
            self.jump_held = true;
            return;
        }
        if self.jump_held || (self.duck.ducking && self.duck.ducked) || self.duck.eye_timer > 0.0 {
            return;
        }
        let ducked = self.duck.ducked;
        self.velocity = jump_on_surface(
            self.velocity,
            self.yaw,
            fmove,
            max,
            ducked,
            self.surface.jump_factor,
        );
        if self.duck.ducking && !ducked {
            self.velocity[2] = self.surface.jump_factor * portal::movement::JUMP_SPEED;
        }
        self.velocity[2] = finish_gravity(self.velocity[2], dt);
        self.grounded = false;
        self.ground = None;
        self.jumped = true;
        self.duck.jump_timer = 510.0;
        self.duck.in_duck_jump = true;
        self.jump_held = true;
    }

    fn duck_eye(&mut self, amount: f32) {
        let q = amount * amount * (3.0 - 2.0 * amount);
        self.duck.eye = 28.0 * q + EYE * (1.0 - q);
    }

    fn finish_duck(&mut self, jumping: bool, chamber: &Chamber, linked: Option<&[Portal; 2]>) {
        if self.duck.ducked {
            return;
        }
        self.duck.ducked = true;
        self.duck.ducking = false;
        self.duck.eye = 28.0;
        if jumping || !self.grounded {
            self.feet[2] += 36.0;
        }
        let original = self.feet;
        if self.trace(original, original, chamber, linked).start_solid {
            for _ in 0..36 {
                self.feet[2] += 1.0;
                if !self
                    .trace(self.feet, self.feet, chamber, linked)
                    .start_solid
                {
                    break;
                }
            }
            if self
                .trace(self.feet, self.feet, chamber, linked)
                .start_solid
            {
                self.feet = original;
            }
        }
        self.categorize(chamber, linked);
    }

    fn standing_trace(
        &self,
        to: Vec3,
        chamber: &Chamber,
        linked: Option<&[Portal; 2]>,
        stationary: bool,
    ) -> Trace {
        collision::trace(
            if stationary { to } else { self.feet },
            to,
            [-HALF_WIDTH, -HALF_WIDTH, 0.0],
            [HALF_WIDTH, HALF_WIDTH, HEIGHT],
            &solids(chamber, linked),
        )
    }

    fn unduck_jump(&mut self, chamber: &Chamber, linked: Option<&[Portal; 2]>) {
        let p = self.feet;
        let tr = self.trace(p, [p[0], p[1], p[2] - 36.0], chamber, linked);
        if tr.fraction >= 1.0
            || self
                .standing_trace(tr.end, chamber, linked, true)
                .start_solid
        {
            return;
        }
        self.feet = tr.end;
        self.duck.ducked = false;
        self.duck.ducking = false;
        self.duck.in_duck_jump = false;
        self.duck.timer = 0.0;
        self.duck.jump_timer = 0.0;
        self.duck.eye = 28.0 + 36.0 * tr.fraction;
        self.duck.eye_timer = 800.0 + 200.0 * (1.0 - tr.fraction);
        self.categorize(chamber, linked);
    }

    /// Translated from 0x27b6e0 and 0x27aa30 (disassembled, server.so).
    fn update_duck(
        &mut self,
        held: bool,
        dt: f32,
        chamber: &Chamber,
        linked: Option<&[Portal; 2]>,
    ) {
        for timer in [
            &mut self.duck.timer,
            &mut self.duck.jump_timer,
            &mut self.duck.eye_timer,
        ] {
            *timer = (*timer - dt * 1000.0).max(0.0);
        }
        if self.duck.eye_timer > 0.0 {
            let seconds = (1000.0 - self.duck.eye_timer).max(0.0) * 0.001;
            if seconds as f64 > 0.2 {
                self.duck.eye_timer = 0.0;
                self.duck.eye = EYE;
            } else {
                self.duck_eye((1.0 - seconds as f64 * 5.0) as f32);
            }
        }
        let pressed = held && !self.duck.held;
        let released = !held && self.duck.held;
        self.duck.held = held;
        let in_duck = self.duck.ducked;
        let in_air = !self.grounded;
        let jumping = self.duck.jump_timer > 0.0;
        let recovering = self.duck.eye_timer > 0.0;
        if !(held || self.duck.ducking || in_duck || jumping) {
            if !recovering {
                self.duck.eye = EYE;
            }
            return;
        }
        if held || jumping {
            if pressed && !in_duck && !jumping && !recovering {
                self.duck.timer = 1000.0;
                self.duck.ducking = true;
            }
            if self.duck.ducking && !jumping && !recovering {
                let seconds = (1000.0 - self.duck.timer).max(0.0) * 0.001;
                if seconds as f64 > 0.4 || in_duck || in_air {
                    self.finish_duck(false, chamber, linked);
                } else {
                    self.duck_eye(seconds * 2.5);
                }
            }
            if jumping {
                if !in_duck {
                    self.finish_duck(true, chamber, linked);
                } else if !held {
                    self.unduck_jump(chamber, linked);
                }
            }
        } else {
            if self.duck.in_duck_jump {
                self.unduck_jump(chamber, linked);
            }
            if recovering {
                return;
            }
            if released {
                if in_duck {
                    self.duck.timer = 1000.0;
                } else if self.duck.ducking && !self.duck.ducked {
                    self.duck.timer = 800.0 + (1000.0 - self.duck.timer) * 0.5;
                }
            }
            let mut to = self.feet;
            if !self.grounded {
                to[2] -= 36.0;
            }
            let tr = self.standing_trace(to, chamber, linked, false);
            if !tr.start_solid && tr.fraction == 1.0 {
                if self.duck.ducking || self.duck.ducked {
                    let seconds = (1000.0 - self.duck.timer).max(0.0) * 0.001;
                    if seconds as f64 > 0.2 || in_air {
                        self.feet = to;
                        self.duck.ducked = false;
                        self.duck.ducking = false;
                        self.duck.in_duck_jump = false;
                        self.duck.timer = 0.0;
                        self.duck.eye = EYE;
                        self.categorize(chamber, linked);
                    } else {
                        self.duck_eye((1.0 - seconds as f64 * 5.0) as f32);
                        self.duck.ducking = true;
                    }
                }
            } else if self.duck.timer != 1000.0 {
                self.duck.eye = 28.0;
                self.duck.timer = 1000.0;
                self.duck.ducked = true;
                self.duck.ducking = false;
            }
        }
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
        self.velocity = accelerate(
            self.velocity,
            dir,
            speed.min(max),
            ACCELERATE * self.surface_friction,
            dt,
        );
        self.velocity[2] = 0.0;
        if self.velocity[0].hypot(self.velocity[1]) < MIN_WALK {
            self.velocity = [0.0; 3];
            return;
        }
        self.travel(dt, true, chamber, linked);
        self.stay_on_ground(chamber, linked);
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
        self.velocity = air_accelerate(
            self.velocity,
            dir,
            speed.min(max),
            AIR_ACCELERATE * self.surface_friction,
            dt,
        );
        self.travel(dt, false, chamber, linked);
    }

    fn trace(
        &self,
        from: Vec3,
        to: Vec3,
        chamber: &Chamber,
        linked: Option<&[Portal; 2]>,
    ) -> Trace {
        collision::trace(
            from,
            to,
            [-HALF_WIDTH, -HALF_WIDTH, 0.0],
            [HALF_WIDTH, HALF_WIDTH, self.height()],
            &solids(chamber, linked),
        )
    }

    fn travel(&mut self, dt: f32, walking: bool, chamber: &Chamber, linked: Option<&[Portal; 2]>) {
        let start = self.feet;
        let velocity = self.velocity;
        let cast = |a, b| self.trace(a, b, chamber, linked);
        let target = collision::add(start, collision::scale(velocity, dt));
        if walking && cast(start, target).fraction == 1.0 {
            self.feet = target;
            return;
        }
        let (down, down_v) = collision::slide(start, velocity, dt, cast);
        if !walking {
            self.feet = down;
            self.velocity = down_v;
            return;
        }
        let up = cast(start, [start[0], start[1], start[2] + STEP + 0.03125]);
        let (mut raised, mut raised_v) = collision::slide(
            if up.start_solid || up.all_solid {
                start
            } else {
                up.end
            },
            velocity,
            dt,
            cast,
        );
        let descend = cast(raised, [raised[0], raised[1], raised[2] - STEP - 0.03125]);
        if descend.normal[2] >= 0.7 && !descend.start_solid && !descend.all_solid {
            raised = descend.end;
            raised_v[2] = down_v[2];
        } else {
            self.feet = down;
            self.velocity = down_v;
            return;
        }
        let distance = |p: Vec3| (p[0] - start[0]).powi(2) + (p[1] - start[1]).powi(2);
        if distance(down) > distance(raised) {
            self.feet = down;
            self.velocity = down_v;
        } else {
            self.feet = raised;
            self.velocity = raised_v;
        }
    }

    fn stay_on_ground(&mut self, chamber: &Chamber, linked: Option<&[Portal; 2]>) {
        let p = self.feet;
        let up = self.trace(p, [p[0], p[1], p[2] + 2.0], chamber, linked);
        let down = self.trace(up.end, [p[0], p[1], p[2] - STEP], chamber, linked);
        if down.fraction > 0.0
            && down.fraction < 1.0
            && !down.start_solid
            && down.normal[2] >= 0.7
            && (p[2] - down.end[2]).abs() > 0.015625
        {
            self.feet = down.end;
        }
    }

    fn categorize(&mut self, chamber: &Chamber, linked: Option<&[Portal; 2]>) {
        let p = self.feet;
        let tr = self.trace(p, [p[0], p[1], p[2] - GROUND_REACH], chamber, linked);
        self.grounded =
            !leaves_ground(self.velocity[2]) && tr.block.is_some() && tr.normal[2] >= 0.7;
        self.ground = if self.grounded { tr.block } else { None };
        if let Some(n) = self.ground {
            self.velocity[2] = 0.0;
            self.surface = self.surfaces.get(n).cloned().unwrap_or_default();
            self.surface_friction = (self.surface.friction * 1.25).min(1.0);
        } else if self.velocity[2] > 0.0 && !leaves_ground(self.velocity[2]) {
            self.surface_friction = 0.25;
        }
        if self.grounded && !tr.start_solid && !tr.all_solid && tr.fraction > 0.0 {
            self.feet = tr.end;
        }
    }

    pub fn height(&self) -> f32 {
        if self.duck.ducked {
            36.0
        } else {
            HEIGHT
        }
    }

    pub fn centre(&self) -> Vec3 {
        [
            self.feet[0],
            self.feet[1],
            self.feet[2] + self.height() / 2.0,
        ]
    }

    pub fn half(&self) -> Vec3 {
        [HALF_WIDTH, HALF_WIDTH, self.height() / 2.0]
    }

    fn carry(
        &mut self,
        from: &Portal,
        to: &Portal,
        before: Vec3,
        chamber: &Chamber,
        linked: &[Portal; 2],
    ) -> bool {
        let old_centre = self.centre();
        let old_duck = self.duck.clone();
        let mut centre = old_centre;
        let force = from.forward[2].abs() > 0.0
            && ((from.forward[2].abs() - 1.0).abs() >= 0.01
                || (to.forward[2].abs() - 1.0).abs() >= 0.01);
        if force && !self.duck.ducked {
            self.duck.ducked = true;
            self.duck.in_duck_jump = true;
            centre[2] += if from.forward[2] > 0.0 { -16.0 } else { 16.0 };
        }
        let offset = collision::sub(self.feet, centre);
        let target = collision::add(through_point(from, to, centre), offset);
        let entry = crossing(from, before, old_centre).map_or(old_centre, |fraction| {
            collision::add(
                before,
                collision::scale(collision::sub(old_centre, before), fraction),
            )
        });
        let adjusted = collision::add(entry, collision::sub(centre, old_centre));
        let exit = collision::add(through_point(from, to, adjusted), offset);
        if self.trace(exit, exit, chamber, Some(linked)).start_solid {
            self.duck = old_duck;
            self.feet = collision::add(
                collision::add(entry, collision::sub(self.feet, old_centre)),
                collision::scale(from.forward, 0.03125),
            );
            self.velocity = collision::clip(self.velocity, from.forward);
            return false;
        }
        self.velocity = exit_velocity(from, to, self.velocity, Mover::Player);
        self.grounded = false;
        self.ground = None;
        let mut normals = Vec::new();
        let (end, _) = collision::slide(exit, collision::sub(target, exit), 1.0, |a, b| {
            let tr = self.trace(a, b, chamber, Some(linked));
            if tr.fraction < 1.0 {
                normals.push(tr.normal);
            }
            tr
        });
        self.feet = end;
        for normal in normals {
            self.velocity = collision::clip(self.velocity, normal);
        }
        let (f, _) = view_through(from, to, self.forward(), self.up());
        self.yaw = f[1].atan2(f[0]).to_degrees();
        self.pitch = -f[2].clamp(-1.0, 1.0).asin().to_degrees();
        true
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
