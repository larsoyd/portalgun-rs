// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::pair::{Portal, Vec3};
use crate::teleport::box_meets_portal;

/// below this squared ground speed nothing steps
const STILL_SQ: f32 = 1e-8;
/// GetStepSoundVelocities' run speed standing
const RUN: f32 = 220.0;
const WALK_MS: f32 = 400.0;
const RUN_MS: f32 = 300.0;
const LAND_MS: f32 = 400.0;
const PUNCH_FALL: f32 = 303.0;
const SAFE_FALL: f32 = 526.5;
const MIN_BOUNCE: f32 = 173.0;
/// PortalPlayer.FallRecover's pitch and volume from the fall speed
const RECOVER_PITCH: (f64, f64) = (125.0, 0.03);
const RECOVER_VOLUME: (f64, f64, f64) = (0.00075, 1.38, 0.38);

/// One footstep: which foot and the volume that replaces the script's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    pub left: bool,
    pub volume: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Landing {
    pub step: Step,
    /// PortalPlayer.FallRecover's pitch and volume after a fall the boots take
    pub recover: Option<(i32, f32)>,
}

/// m_flStepSoundTime in milliseconds and m_nStepside.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Steps {
    pub time: f32,
    pub left: bool,
}

impl Steps {
    /// Translated from 0x2499e0 (disassembled, server.so)
    fn play(&mut self, volume: f32) -> Step {
        let left = self.left;
        self.left = !left;
        Step { left, volume }
    }

    /// Translated from 0x243dc0, 0x2d5190 and 0x244200 (disassembled, server.so), on dry ground
    /// and standing. material is the surface's gamematerial character.
    pub fn update(
        &mut self,
        frametime: f32,
        velocity: Vec3,
        on_ground: bool,
        material: u8,
    ) -> Option<Step> {
        self.update_duck(frametime, velocity, on_ground, material, false)
    }

    pub fn update_duck(
        &mut self,
        frametime: f32,
        velocity: Vec3,
        on_ground: bool,
        material: u8,
        ducked: bool,
    ) -> Option<Step> {
        if self.time > 0.0 {
            self.time -= 1000.0 * frametime;
            if self.time < 0.0 {
                self.time = 0.0;
            } else if self.time > 0.0 {
                return None;
            }
        }
        let ground_sq = velocity[0] * velocity[0] + velocity[1] * velocity[1];
        if !on_ground || ground_sq <= STILL_SQ {
            return None;
        }
        let speed = (ground_sq + velocity[2] * velocity[2]).sqrt();
        let walking = (if ducked { 60.0 } else { RUN }) > speed;
        self.time = if walking { WALK_MS } else { RUN_MS };
        if ducked {
            self.time += 100.0;
        }
        let (walk, run) = match material {
            b'D' => (0.25, 0.55),
            b'V' => (0.4, 0.7),
            _ => (0.2, 0.5),
        };
        let volume = if walking { walk } else { run };
        Some(self.play(if ducked {
            (volume as f64 * 0.65) as f32
        } else {
            volume
        }))
    }

    /// Translated from 0x2750e0 (disassembled, server.so)
    pub fn jump(&mut self) -> Step {
        self.play(1.0)
    }

    /// Translated from 0x271130, 0x26f5a0 and 0x2d0a60 (disassembled, server.so), on still dry
    /// ground. fall is m_flFallVelocity.
    pub fn land(&mut self, fall: f32) -> Option<Landing> {
        if 0.0 >= fall || fall < PUNCH_FALL {
            return None;
        }
        let volume = landing_volume(fall);
        if volume <= 0.0 {
            return None;
        }
        self.time = LAND_MS;
        let recover = (volume >= 1.0).then(|| {
            let pitch = (RECOVER_PITCH.0 - RECOVER_PITCH.1 * fall as f64).trunc() as i32;
            let v = fall as f64 * RECOVER_VOLUME.0;
            let loud = if RECOVER_VOLUME.1 > v {
                (v - RECOVER_VOLUME.2) as f32
            } else {
                1.0
            };
            (pitch, loud)
        });
        Some(Landing {
            step: self.play(volume),
            recover,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Heard {
    Step(Step),
    Jump(Step),
    Land(Landing),
}

/// What a tick of movement did that makes a sound.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moved {
    /// velocity, ground and duck before the move
    pub before: (Vec3, bool, bool),
    pub jumped: bool,
    pub landed: Option<f32>,
}

impl Steps {
    /// PlayerMove runs UpdateStepSound before FullWalkMove, which jumps and then lands.
    /// Translated from 0x27ab40 (decompiled, server.so)
    pub fn tick(&mut self, frametime: f32, moved: Moved, material: u8) -> Vec<Heard> {
        let (velocity, grounded, ducked) = moved.before;
        let step = self.update_duck(frametime, velocity, grounded, material, ducked);
        let jump = moved.jumped.then(|| self.jump());
        let land = moved.landed.and_then(|fall| self.land(fall));
        let heard = [
            step.map(Heard::Step),
            jump.map(Heard::Jump),
            land.map(Heard::Land),
        ];
        heard.into_iter().flatten().collect()
    }
}

/// Translated from 0x271130 (disassembled, server.so)
pub fn landing_volume(fall: f32) -> f32 {
    if fall > SAFE_FALL {
        1.0
    } else if fall > SAFE_FALL / 2.0 {
        0.85
    } else if MIN_BOUNCE > fall {
        0.0
    } else {
        0.5
    }
}

/// Whether the player's shrunk box meets the portal holding it, for PortalPlayer.EnterPortal and
/// ExitPortal.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PlaneSounds {
    intersecting: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaneSound {
    pub enter: bool,
    pub pitch: i32,
    pub volume: f32,
}

impl PlaneSounds {
    /// size is the player's box. Enter takes a quarter of it, exit three tenths.
    /// Translated from 0x909920 (decompiled, server.so)
    pub fn update(
        &mut self,
        portal: Option<&Portal>,
        centre: Vec3,
        size: Vec3,
        velocity: Vec3,
    ) -> Option<PlaneSound> {
        let speed =
            (velocity[0] * velocity[0] + velocity[1] * velocity[1] + velocity[2] * velocity[2])
                .sqrt();
        let Some(p) = portal else {
            return std::mem::take(&mut self.intersecting).then(|| plane_sound(false, speed));
        };
        if velocity.iter().all(|&c| -0.01 < c && c < 0.01) {
            return None;
        }
        let scale = if self.intersecting { 0.3 } else { 0.25 };
        let meets = box_meets_portal(p, centre, size.map(|s| s * scale));
        (meets != self.intersecting).then(|| {
            self.intersecting = meets;
            plane_sound(meets, speed)
        })
    }
}

/// Pitch and volume from the speed, in place of the script's.
pub fn plane_sound(enter: bool, speed: f32) -> PlaneSound {
    PlaneSound {
        enter,
        pitch: (0.03 * speed + 80.0) as i32,
        volume: match speed < 933.3333 {
            true => speed * 0.00075 + 0.3,
            false => 1.0,
        },
    }
}

/// PortalPlayer.Woosh's volume, faded to over 0.1 s.
/// Translated from 0x902670 (decompiled, server.so)
pub fn woosh_volume(velocity: Vec3) -> f32 {
    let speed_sq =
        velocity[0] * velocity[0] + velocity[1] * velocity[1] + velocity[2] * velocity[2];
    let speed = speed_sq.sqrt();
    match speed < 300.0 {
        true => 0.0,
        false if speed_sq <= 5290000.5 => (speed - 300.0) * 0.0005,
        false => 1.0,
    }
}

/// A controller patch's volume, faded linearly to each new target. The fade restarts on every
/// change, as the woosh's 0.1 s fade does each tick.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Fade {
    pub volume: f32,
    rate: f32,
    target: f32,
}

impl Fade {
    pub fn to(&mut self, target: f32, seconds: f32) {
        self.target = target;
        self.rate = (target - self.volume) / seconds;
    }

    pub fn advance(&mut self, dt: f32) -> f32 {
        let next = self.volume + self.rate * dt;
        self.volume = match self.rate > 0.0 {
            true => next.min(self.target),
            false => next.max(self.target),
        };
        self.volume
    }
}
