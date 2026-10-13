// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::movement::{
    accelerate, air_accelerate, check_velocity, command, finish_gravity, fling_guard, flinging,
    funnel, jump_on_surface, leaves_ground, standable, start_gravity, surface_friction, wish,
    Funnel, Punch, ACCELERATE, AIR_ACCELERATE, MAX_SPEED, WALK_SPEED,
};
use portal::pair::{crossing, through_point, Portal, Vec3};
use portal::teleport::{
    entered, exit_velocity, fix_roll, in_front, keeps_leaving, nearer, pitch_reorients, touching,
    view_through, Mover, Reorient, RollFix,
};
use portal::viewbob::{angle_vectors, vector_angles, Angles};

use crate::chamber::Chamber;
use crate::collision::{self, Trace};
use crate::simulator::Space;
use portal::surfaces::Surface;

/// the hull and view table at 0xa84110 in server.so
pub const HALF_WIDTH: f32 = 16.0;
pub const HEIGHT: f32 = 72.0;
pub const EYE: f32 = 64.0;
/// the client's m_yaw and m_pitch defaults
const MOUSE_FACTOR: f32 = 0.022;
/// cl_pitchdown and cl_pitchup, both registered with 89 at 0x1cd3c0 in client.so
const PITCH_LIMIT: f32 = 89.0;
/// sv_stepsize's default, registered at 0xdf8580 in server.so
const STEP: f32 = 18.0;
/// how far below the feet the ground check looks, 0xa74774 in server.so
const GROUND_REACH: f32 = 2.0;
/// a walk slower than this stops dead, 0xa74730 in server.so
const MIN_WALK: f32 = 1.0;
/// mask 0x200400b, what TestPlayerPosition and FindClosestPassableSpace collide with
const FCPS_TRIES: usize = 100;
const FCPS_PENALTY: f32 = 100.0;

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
    /// jump animation events raised so far
    pub jumps: u64,
    pub roll: f32,
    pub reorient: Reorient,
    pub punch: Punch,
    /// the fall speed, for the step the walker landed on
    pub landed: Option<f32>,
    /// jump held since the last jump or since leaving the ground, so holding it never repeats
    jump_held: bool,
    pub duck: Duck,
    surfaces: Vec<Surface>,
    surface_friction: f32,
    surface: Surface,
    categorize_on_spawn: bool,
    /// the portal whose simulator holds the player, m_hPortalEnvironment
    pub owner: Option<usize>,
    /// set by each teleport, makes the roll recovery pitch instead
    pub pitch_reorientation: bool,
    /// velocity, ground and duck when UpdateStepSound reads them, before the move
    pub before_move: (Vec3, bool, bool),
    touching: [bool; 2],
    /// IN_DUCK forced by a tilted teleport until the next move ends
    forced_duck: bool,
    stuck_count: usize,
    stuck_time: f32,
    last_fcps_warning: Option<f32>,
    clock: f32,
    command: u64,
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
            jumps: 0,
            roll: 0.0,
            reorient: Reorient::default(),
            punch: Punch::default(),
            landed: None,
            jump_held: false,
            duck: Duck::default(),
            surfaces: Vec::new(),
            surface_friction: 1.0,
            surface: Surface::default(),
            categorize_on_spawn: true,
            owner: None,
            pitch_reorientation: false,
            before_move: ([0.0; 3], false, false),
            touching: [false; 2],
            forced_duck: false,
            stuck_count: 0,
            stuck_time: 0.0,
            last_fcps_warning: None,
            clock: 0.0,
            command: 0,
        }
    }

    pub fn set_surfaces(&mut self, surfaces: Vec<Surface>) {
        self.surfaces = surfaces;
    }

    pub fn eye(&self) -> Vec3 {
        [self.feet[0], self.feet[1], self.feet[2] + self.duck.eye]
    }

    pub fn forward(&self) -> Vec3 {
        angle_vectors(self.angles()).0
    }

    pub fn right(&self) -> Vec3 {
        angle_vectors(self.angles()).1
    }

    pub fn up(&self) -> Vec3 {
        angle_vectors(self.angles()).2
    }

    pub fn angles(&self) -> Angles {
        Angles {
            pitch: self.pitch,
            yaw: self.yaw,
            roll: self.roll,
        }
    }

    fn set_angles(&mut self, a: Angles) {
        (self.pitch, self.yaw, self.roll) = (a.pitch, a.yaw, a.roll);
    }

    /// The view carried through a portal, roll and all.
    pub fn turn(&mut self, from: &Portal, to: &Portal) {
        let (f, u) = view_through(from, to, self.forward(), self.up());
        self.set_angles(vector_angles(f, u));
    }

    /// The client's per-frame roll recovery, with cl_reorient_in_air at its default of 1.
    pub fn fix_view(&mut self, dt: f32, coin: bool) {
        let how = RollFix {
            grounded: self.grounded,
            in_air: true,
            pitch_reorientation: self.pitch_reorientation,
            coin,
            frametime: dt,
        };
        let a = self.angles();
        let a = fix_roll(&mut self.reorient, a, how);
        self.set_angles(a);
    }

    /// Mouse counts, right and down positive. Upside down both turn the other way.
    /// Translated from 0x4fae50 (disassembly, client.so)
    pub fn look(&mut self, dx: f32, dy: f32, sensitivity: f32) {
        let flip = if self.reorient.upside_down { -1.0 } else { 1.0 };
        self.yaw -= flip * dx * sensitivity * MOUSE_FACTOR;
        self.pitch =
            (self.pitch + flip * dy * sensitivity * MOUSE_FACTOR).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// One tick of PlayerMove and FullWalkMove, then the portal touches.
    /// Translated from 0x27ab40 and 0x275d90 (decompiled, server.so)
    pub fn step(
        &mut self,
        input: &Input,
        dt: f32,
        chamber: &Chamber,
        linked: Option<[Portal; 2]>,
    ) -> Option<usize> {
        let space = Space::new(chamber, linked.as_ref());
        let space = &space;
        self.command += 1;
        self.clock += dt;
        if self.categorize_on_spawn {
            self.categorize(space);
            self.categorize_on_spawn = false;
        }
        // DecayPunchAngle, through slot 0x38 at 0x273970 (disassembly, server.so)
        self.punch.decay(dt);
        if self.stuck_check_due(dt) && self.check_stuck(space) {
            self.forced_duck = false;
            return self.touch_portals(space);
        }
        // sv_optimizedmovement lets go of the ground above 250, 0x27ae02 (disassembly, server.so)
        if self.velocity[2] > 250.0 {
            self.grounded = false;
            self.ground = None;
        }
        self.jumped = false;
        self.landed = None;
        if !self.grounded {
            self.fall = -self.velocity[2];
        }
        self.before_move = (self.velocity, self.grounded, self.duck.ducked);
        let max = MAX_SPEED.min(WALK_SPEED) * self.surface.max_speed_factor.min(1.0);
        let (mut fmove, mut smove) = command(input.forward, input.side, max);
        // SpeedCrop, 0x26de80 (decompiled, server.so)
        if self.duck.ducked && self.grounded {
            fmove *= 1.0 / 3.0;
            smove *= 1.0 / 3.0;
        }
        self.update_duck(input.crouch || self.forced_duck, dt, space);
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
            true => self.walk_move(fmove, smove, max, dt, space),
            false => self.air_move(fmove, smove, max, dt, space),
        }
        self.categorize(space);
        self.velocity = check_velocity(self.velocity);
        self.velocity[2] = finish_gravity(self.velocity[2], dt);
        if self.grounded {
            self.velocity[2] = 0.0;
        }
        if self.grounded && self.fall > 0.0 {
            self.landed = Some(self.fall);
            self.fall = 0.0;
        }
        // ProcessMovement unforces IN_DUCK after the move, 0x2ce200 (decompiled, server.so)
        self.forced_duck = false;
        self.touch_portals(space)
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
        // the animation event 6 Portal adds at 0x2ce350 (decompiled, server.so)
        self.jumps += 1;
        self.duck.jump_timer = 510.0;
        self.duck.in_duck_jump = true;
        self.jump_held = true;
    }

    /// Translated from 0x27a960 (decompiled, server.so)
    fn duck_eye(&mut self, amount: f32) {
        let q = amount * amount * (3.0 - 2.0 * amount);
        self.duck.eye = 28.0 * q + EYE * (1.0 - q);
    }

    /// Translated from 0x279490 and 0x279370 (decompiled, server.so)
    fn finish_duck(&mut self, jumping: bool, space: &Space) {
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
        if self.trace(original, original, space).start_solid {
            for _ in 0..36 {
                self.feet[2] += 1.0;
                if !self.trace(self.feet, self.feet, space).start_solid {
                    break;
                }
            }
            if self.trace(self.feet, self.feet, space).start_solid {
                self.feet = original;
            }
        }
        self.categorize(space);
    }

    fn standing_trace(&self, to: Vec3, space: &Space, stationary: bool) -> Trace {
        space.trace(
            self.owner,
            if stationary { to } else { self.feet },
            to,
            [-HALF_WIDTH, -HALF_WIDTH, 0.0],
            [HALF_WIDTH, HALF_WIDTH, HEIGHT],
        )
    }

    /// Translated from 0x27a3f0, 0x27b220 and 0x279bc0 (decompiled, server.so)
    fn unduck_jump(&mut self, space: &Space) {
        let p = self.feet;
        let tr = self.trace(p, [p[0], p[1], p[2] - 36.0], space);
        if tr.fraction >= 1.0 || self.standing_trace(tr.end, space, true).start_solid {
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
        self.categorize(space);
    }

    /// Translated from 0x27b6e0 and 0x27aa30 (disassembled, server.so).
    fn update_duck(&mut self, held: bool, dt: f32, space: &Space) {
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
                    self.finish_duck(false, space);
                } else {
                    self.duck_eye(seconds * 2.5);
                }
            }
            if jumping {
                if !in_duck {
                    self.finish_duck(true, space);
                } else if !held {
                    self.unduck_jump(space);
                }
            }
        } else {
            if self.duck.in_duck_jump {
                self.unduck_jump(space);
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
            let tr = self.standing_trace(to, space, false);
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
                        self.categorize(space);
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
    fn walk_move(&mut self, fmove: f32, smove: f32, max: f32, dt: f32, space: &Space) {
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
        self.travel(dt, true, space);
        self.stay_on_ground(space);
    }

    /// Translated from 0x2cf2a0 (decompiled, server.so)
    fn air_move(&mut self, fmove: f32, smove: f32, max: f32, dt: f32, space: &Space) {
        let flat = [self.velocity[0], self.velocity[1]];
        let mut want = wish(self.yaw, fmove, smove);
        if flinging(flat) {
            want = fling_guard(flat, want);
        } else {
            for p in space.pair.iter().flatten() {
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
        self.travel(dt, false, space);
    }

    fn trace(&self, from: Vec3, to: Vec3, space: &Space) -> Trace {
        space.trace(
            self.owner,
            from,
            to,
            [-HALF_WIDTH, -HALF_WIDTH, 0.0],
            [HALF_WIDTH, HALF_WIDTH, self.height()],
        )
    }

    /// WalkMove's direct move, then StepMove, or TryPlayerMove in the air.
    /// Translated from 0x274230 and 0x2cf2a0 (decompiled, server.so)
    fn travel(&mut self, dt: f32, walking: bool, space: &Space) {
        let start = self.feet;
        let cast = |a, b| self.trace(a, b, space);
        let target = collision::add(start, collision::scale(self.velocity, dt));
        (self.feet, self.velocity) = match walking {
            true if cast(start, target).fraction == 1.0 => (target, self.velocity),
            true => collision::step_move(start, self.velocity, dt, STEP, cast),
            false => collision::slide(start, self.velocity, dt, true, cast),
        };
    }

    /// Translated from 0x274050 (decompiled, server.so)
    fn stay_on_ground(&mut self, space: &Space) {
        let p = self.feet;
        let up = self.trace(p, [p[0], p[1], p[2] + 2.0], space);
        let down = self.trace(up.end, [p[0], p[1], p[2] - STEP], space);
        if down.fraction > 0.0
            && down.fraction < 1.0
            && !down.start_solid
            && standable(down.normal[2])
            && (p[2] - down.end[2]).abs() > 0.015625
        {
            self.feet = down.end;
        }
    }

    /// Translated from 0x2d06d0 (decompiled, server.so)
    fn categorize(&mut self, space: &Space) {
        let p = self.feet;
        let tr = self.trace(p, [p[0], p[1], p[2] - GROUND_REACH], space);
        self.grounded =
            !leaves_ground(self.velocity[2]) && tr.block.is_some() && standable(tr.normal[2]);
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

    /// StartTouch, Touch and EndTouch of both portals on the player's box.
    fn touch_portals(&mut self, space: &Space) -> Option<usize> {
        let pair = space.pair?;
        for i in 0..2 {
            let now = touching(&pair[i], self.centre(), self.half());
            let was = std::mem::replace(&mut self.touching[i], now);
            if now && !was {
                self.start_touch(i, &pair);
            }
            let crossed = match (was, now) {
                (_, true) => self.touch(i, &pair, space),
                (true, false) => self.end_touch(i, &pair),
                (false, false) => false,
            };
            if crossed {
                self.relink(&pair);
                return Some(i);
            }
        }
        None
    }

    /// After a teleport the player's touches are taken where it now stands.
    fn relink(&mut self, pair: &[Portal; 2]) {
        for (j, p) in pair.iter().enumerate() {
            let now = touching(p, self.centre(), self.half());
            if now && !self.touching[j] {
                self.start_touch(j, pair);
            }
            self.touching[j] = now;
        }
    }

    fn closer(&self, i: usize, pair: &[Portal; 2]) -> bool {
        match self.owner {
            Some(o) if o != i => nearer(self.centre(), &pair[i], &pair[o]),
            _ => true,
        }
    }

    /// Translated from 0x91da70 (decompiled, server.so)
    fn start_touch(&mut self, i: usize, pair: &[Portal; 2]) {
        if in_front(&pair[i], self.centre()) && self.closer(i, pair) {
            self.owner = Some(i);
        }
    }

    /// Translated from 0x91d3b0 (decompiled, server.so)
    fn touch(&mut self, i: usize, pair: &[Portal; 2], space: &Space) -> bool {
        if self.owner != Some(i) {
            if !self.closer(i, pair) {
                return false;
            }
            let in_solid = collision::trace_brushes(
                self.feet,
                self.feet,
                [-HALF_WIDTH, -HALF_WIDTH, 0.0],
                [HALF_WIDTH, HALF_WIDTH, self.height()],
                &space.world,
            )
            .start_solid;
            if in_solid || in_front(&pair[i], self.centre()) {
                self.owner = Some(i);
            }
        }
        self.teleport_if_entered(i, pair)
    }

    /// A player who fell out of a ceiling portal keeps it while ducked behind its plane.
    /// Translated from 0x91dc20 (decompiled, server.so)
    fn end_touch(&mut self, i: usize, pair: &[Portal; 2]) -> bool {
        if self.teleport_if_entered(i, pair) {
            return true;
        }
        let hold = keeps_leaving(&pair[i], self.centre(), self.duck.in_duck_jump);
        if self.owner == Some(i) && !hold {
            self.owner = None;
        }
        false
    }

    fn teleport_if_entered(&mut self, i: usize, pair: &[Portal; 2]) -> bool {
        let entering = self.owner == Some(i) && entered(&pair[i], self.centre(), self.half());
        if entering {
            self.teleport(&pair[i], &pair[1 - i]);
            self.owner = Some(1 - i);
        }
        entering
    }

    /// TeleportTouchingEntity for the player: no trace, a blocked exit is left to CheckStuck.
    /// Translated from 0x912ff0 (decompiled, server.so)
    fn teleport(&mut self, from: &Portal, to: &Portal) {
        let mut centre = self.centre();
        let tilted = from.forward[2].abs() > 0.0
            && ((from.forward[2].abs() - 1.0).abs() >= 0.01
                || (to.forward[2].abs() - 1.0).abs() >= 0.01);
        if tilted && !self.duck.ducked {
            // ForceDuckThisFrame, 0x908c60 (decompiled, server.so)
            self.duck.ducked = true;
            self.duck.in_duck_jump = true;
            self.forced_duck = true;
            centre[2] += if from.forward[2] > 0.0 { -16.0 } else { 16.0 };
        }
        // nothing can be carried in the sandbox, so a held object never clears it
        self.pitch_reorientation = pitch_reorients(self.forward(), from, to.forward[2]);
        let offset = collision::sub(self.feet, centre);
        self.feet = collision::add(through_point(from, to, centre), offset);
        self.velocity = exit_velocity(from, to, self.velocity, Mover::Player);
        self.grounded = false;
        self.ground = None;
        self.turn(from, to);
    }

    /// CheckInterval for STUCK: 0.2 s of ticks in single player, every tick once stuck.
    /// Translated from 0x273240 and 0x26d7c0 (decompiled, server.so)
    fn stuck_check_due(&self, dt: f32) -> bool {
        let interval = match self.stuck_count {
            0 => ((0.2 / dt) as u64).max(1),
            _ => 1,
        };
        (self.command + 1).is_multiple_of(interval)
    }

    /// The portal CheckStuck, which runs FindClosestPassableSpace inside a portal's space.
    /// Translated from 0x2ce4c0 (decompiled, server.so)
    fn check_stuck(&mut self, space: &Space) -> bool {
        if !self.base_check_stuck(space) {
            self.last_fcps_warning = None;
            return false;
        }
        if let Some(push) = self.owner.zip(space.pair).map(|(i, pair)| pair[i].forward) {
            let old = self.feet;
            if !self.find_closest_passable_space(space, push) {
                if self
                    .last_fcps_warning
                    .is_none_or(|last| self.clock - last >= 5.0)
                {
                    self.last_fcps_warning = Some(self.clock);
                    bevy::log::warn!("[walker] fcps=failed at={} feet={:?} velocity={:?} push={push:?} ducked={} owner={:?} attempts={} pair={:?}", self.clock, self.feet, self.velocity, self.duck.ducked, self.owner, self.stuck_count, space.pair);
                }
            }
            if collision::dot(collision::sub(self.feet, old), push) < 0.0 {
                self.feet = collision::add(old, collision::scale(push, 5.0));
            }
        }
        self.base_check_stuck(space)
    }

    /// Translated from 0x26dbd0 (decompiled, server.so)
    fn base_check_stuck(&mut self, space: &Space) -> bool {
        let stuck_at = |w: &Self, at: Vec3| w.trace(at, at, space).start_solid;
        if !stuck_at(self, self.feet) {
            self.stuck_count = 0;
            return false;
        }
        if (self.stuck_time as f64) < self.clock as f64 - 0.05 {
            self.stuck_time = self.clock;
            let test = collision::add(self.feet, stuck_offset(self.stuck_count % 54));
            self.stuck_count += 1;
            if !stuck_at(self, test) {
                self.stuck_count = 0;
                self.feet = test;
                return false;
            }
        }
        true
    }

    /// Steps the box toward open space by weighing traces between its corners.
    /// Translated from 0x2d7da0 (decompiled, server.so)
    fn find_closest_passable_space(&mut self, space: &Space, push: Vec3) -> bool {
        let world = &space.world;
        let (mins, maxs) = (
            collision::add(self.feet, [-HALF_WIDTH, -HALF_WIDTH, 0.0]),
            collision::add(self.feet, [HALF_WIDTH, HALF_WIDTH, self.height()]),
        );
        let c = [0, 1, 2].map(|i| (mins[i] + maxs[i]) * 0.5);
        let ext = collision::sub(maxs, c);
        let k = ext.map(|e| e * 0.00990099);
        let mut pos = [c[0], c[1], c[2] + 0.001];
        let mut high = collision::sub(ext, k);
        let mut low = collision::scale(high, -1.0);
        let mut grow = k;
        let cast = |a: Vec3, b: Vec3, half: Vec3| {
            collision::trace_brushes(a, b, collision::scale(half, -1.0), half, world)
        };
        for _ in 0..FCPS_TRIES {
            let t = cast(pos, c, ext);
            if !t.start_solid {
                self.feet = collision::add(collision::sub(t.end, c), self.feet);
                return true;
            }
            let corners: [Vec3; 8] = std::array::from_fn(|i| {
                [
                    pos[0] + if i & 1 == 0 { low[0] } else { high[0] },
                    pos[1] + if i & 2 == 0 { low[1] } else { high[1] },
                    pos[2] + if i & 4 == 0 { low[2] } else { high[2] },
                ]
            });
            let outside = corners.map(|p| world.iter().any(|(_, planes)| inside(planes, p)));
            let mut weight = [0.0f32; 8];
            for i in 0..7 {
                for j in i + 1..8 {
                    let delta = collision::sub(corners[j], corners[i]);
                    let d = collision::dot(delta, delta).sqrt();
                    let reach = |from: usize, to: usize, blocked: bool| match blocked {
                        true => -FCPS_PENALTY,
                        false => {
                            let t = cast(corners[from], corners[to], grow);
                            if t.start_solid {
                                -FCPS_PENALTY
                            } else {
                                t.fraction * d
                            }
                        }
                    };
                    weight[i] += reach(i, j, outside[i]);
                    weight[j] += reach(j, i, outside[j]);
                }
            }
            let total: f32 = weight.iter().filter(|&&w| w > 0.0).sum();
            let step = if total == 0.0 {
                high = ext;
                low = collision::scale(ext, -1.0);
                grow = k;
                push
            } else {
                let sum = (0..8)
                    .filter(|&i| weight[i] > 0.0)
                    .fold([0.0; 3], |acc, i| {
                        collision::add(
                            acc,
                            collision::scale(collision::sub(corners[i], pos), weight[i]),
                        )
                    });
                low = collision::sub(k, high);
                high = collision::sub(high, k);
                grow = collision::add(grow, k);
                collision::scale(sum, 1.0 / total)
            };
            pos = collision::add(pos, step);
        }
        false
    }
}

/// CreateStuckTable: small nudges on each axis and their corners, then big ones.
/// Translated from 0x26d990 (decompiled, server.so)
fn stuck_offset(n: usize) -> Vec3 {
    let small = [-0.125, 0.0, 0.125];
    let big = [-2.0, 0.0, 2.0];
    let heights = [0.0, 1.0, 6.0];
    let mut table: Vec<Vec3> = Vec::with_capacity(54);
    table.extend(small.map(|z| [0.0, 0.0, z]));
    table.extend(small.map(|y| [0.0, y, 0.0]));
    table.extend(small.map(|x| [x, 0.0, 0.0]));
    for x in [-0.125, 0.125] {
        for y in [-0.125, 0.125] {
            table.extend([-0.125, 0.125].map(|z| [x, y, z]));
        }
    }
    table.extend(heights.map(|z| [0.0, 0.0, z]));
    table.extend(big.map(|y| [0.0, y, 0.0]));
    table.extend(big.map(|x| [x, 0.0, 0.0]));
    for z in heights {
        for x in big {
            table.extend(big.map(|y| [x, y, z]));
        }
    }
    table.push([0.0; 3]);
    table[n]
}

/// PointOutsideWorld, read as a point inside some brush.
fn inside(planes: &[collision::Plane], p: Vec3) -> bool {
    planes
        .iter()
        .all(|pl| collision::dot(pl.normal, p) < pl.distance)
}

/// A wish split into its direction and speed.
fn direction(w: [f32; 2]) -> (Vec3, f32) {
    let speed = w[0].hypot(w[1]);
    match speed > 0.0 {
        true => ([w[0] / speed, w[1] / speed, 0.0], speed),
        false => ([0.0; 3], 0.0),
    }
}

/// Ground under a foot for the foot placement, none over a portal's opening.
pub fn body_support(at: Vec3, chamber: &Chamber, linked: Option<&[Portal; 2]>) -> Option<f32> {
    let (from, to) = (
        [at[0], at[1], at[2] + GROUND_REACH],
        [at[0], at[1], at[2] - HEIGHT],
    );
    if linked.is_some_and(|pair| pair.iter().any(|p| crossing(p, from, to).is_some())) {
        return None;
    }
    let trace = Space::new(chamber, None).trace(None, from, to, [0.0; 3], [0.0; 3]);
    trace
        .block
        .filter(|_| trace.normal[2] > 0.7 && !trace.start_solid)
        .map(|i| chamber.blocks[i].max[2])
}
