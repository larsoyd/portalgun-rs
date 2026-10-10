use crate::pair::Vec3;

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
const PUNCH_ROLL: f64 = 0.013;
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
    pub punch_roll: f32,
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
            punch_roll: (fall as f64 * PUNCH_ROLL) as f32,
        })
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
