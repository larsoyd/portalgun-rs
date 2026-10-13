// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

//! The gun's own glows and sprites on its view model. C_WeaponPortalgun runs them, so do we.

pub const SPRITE_SCALE: f32 = 128.0;
const EFFECTS_MIN_ALPHA: f32 = 96.0;
const EFFECTS_MAX_ALPHA: f32 = 128.0;
const EFFECTS_MIN_SIZE: f32 = 3.0;
const BEAM_SIZE: f32 = 0.04;
/// m_fEffectsMaxSize1 outside an emancipation grill
const EFFECTS_MAX_SIZE: f32 = 4.0;
const NO_PORTAL_COLOUR: [f32; 3] = [128.0; 3];

const GLOWS: [&str; 6] = [
    "Arm1_attach1",
    "Arm1_attach2",
    "Arm2_attach1",
    "Arm2_attach2",
    "Arm3_attach1",
    "Arm3_attach2",
];
const END_CAPS: [&str; 3] = ["Arm1_attach3", "Arm2_attach3", "Arm3_attach3"];
const TUBE: [&str; 5] = [
    "Beam_point1",
    "Beam_point2",
    "Beam_point3",
    "Beam_point4",
    "Beam_point5",
];

/// CInterpolatedValue with linear interpolation.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Interp {
    start_time: f32,
    end_time: f32,
    start: f32,
    end: f32,
}

impl Interp {
    pub fn set_absolute(&mut self, v: f32, now: f32) {
        *self = Interp {
            start_time: now,
            end_time: now,
            start: v,
            end: v,
        };
    }

    pub fn init_from_current(&mut self, end: f32, dt: f32, now: f32) {
        let start = self.value(now);
        if dt <= 0.0 {
            self.set_absolute(end, now);
        } else {
            *self = Interp {
                start_time: now,
                end_time: now + dt,
                start,
                end,
            };
        }
    }

    pub fn value(&self, now: f32) -> f32 {
        if now >= self.end_time {
            self.end
        } else if now <= self.start_time {
            self.start
        } else {
            self.start
                + (self.end - self.start) * (now - self.start_time)
                    / (self.end_time - self.start_time)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    None,
    Ready,
    Holding,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite {
    pub attachment: &'static str,
    pub material: &'static str,
    pub color: [f32; 3],
    /// 0 to 255
    pub alpha: f32,
    /// edge length in the model's units
    pub size: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Beam {
    pub start: &'static str,
    pub end: &'static str,
    pub material: &'static str,
    pub width: f32,
    pub end_width: f32,
    pub alpha: f32,
    pub amplitude: f32,
    pub speed: f32,
    pub frame_rate: f32,
    pub segments: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    GravLight,
    Portal1Light,
    Portal2Light,
    Glow,
    EndCap,
    MuzzleGlow,
    Tube,
}

#[derive(Debug, Clone, Copy)]
struct Effect {
    kind: Kind,
    attachment: &'static str,
    material: &'static str,
    color: [f32; 3],
    alpha: Interp,
    scale: Interp,
    visible: bool,
}

/// The view model's effect parameters (the third-person ones are the same with other sizes).
pub struct GunFx {
    effects: Vec<Effect>,
    state: State,
    pulse: f32,
    pulse_up: bool,
    max_size: f32,
    world: bool,
}

fn portal_colour(last_fired: Option<u8>) -> [f32; 3] {
    match last_fired {
        Some(1) => [64.0, 160.0, 255.0],
        Some(2) => [255.0, 160.0, 32.0],
        _ => NO_PORTAL_COLOUR,
    }
}

/// random draws lie in [0, 1)
fn random_int(random: &mut impl FnMut() -> f32, lo: i32, hi: i32) -> f32 {
    (lo + ((hi - lo + 1) as f32 * random()) as i32) as f32
}

fn random_float(random: &mut impl FnMut() -> f32, lo: f32, hi: f32) -> f32 {
    lo + (hi - lo) * random()
}

impl GunFx {
    // Translated from 0x55b8a0 (decompiled, client.so)
    pub fn set_max_size(&mut self, value: f32) {
        self.max_size = value;
    }

    // Translated from 0x559b80 and 0x55b280 (decompiled, client.so)
    pub fn beams(&self) -> Vec<Beam> {
        if self.state != State::Holding {
            return Vec::new();
        }
        END_CAPS
            .iter()
            .map(|&start| Beam {
                start,
                end: "muzzle",
                material: if self.world {
                    "sprites/grav_beam"
                } else {
                    "sprites/grav_beam_noz"
                },
                width: if self.world { 0.5 } else { 0.0 },
                end_width: 2.0,
                alpha: 128.0,
                amplitude: 16.0,
                speed: 150.0,
                frame_rate: 30.0,
                segments: 8,
            })
            .collect()
    }

    // Translated from 0x55a0f0 (decompiled, client.so)
    pub fn world(now: f32) -> Self {
        let mut fx = Self::new(now);
        fx.world = true;
        for e in &mut fx.effects {
            let factor = match e.kind {
                Kind::GravLight | Kind::Portal1Light | Kind::Portal2Light => 0.03 / 0.018,
                Kind::Glow | Kind::EndCap => 2.0,
                _ => 1.0,
            };
            e.scale.set_absolute(e.scale.value(now) * factor, now);
        }
        fx
    }

    /// StartEffects: every sprite made hidden with its first size and alpha.
    pub fn new(now: f32) -> Self {
        let mut effects = Vec::new();
        let mut add = |kind, attachment, material, scale: f32, alpha: f32, color| {
            let mut e = Effect {
                kind,
                attachment,
                material,
                color,
                alpha: Interp::default(),
                scale: Interp::default(),
                visible: false,
            };
            e.scale.set_absolute(scale, now);
            e.alpha.set_absolute(alpha, now);
            effects.push(e);
        };
        let white = [255.0; 3];
        add(
            Kind::GravLight,
            "Body_light",
            "sprites/grav_light",
            0.018 * SPRITE_SCALE,
            128.0,
            white,
        );
        add(
            Kind::Portal1Light,
            "Body_light",
            "sprites/bluelight",
            0.018 * SPRITE_SCALE,
            128.0,
            white,
        );
        add(
            Kind::Portal2Light,
            "Body_light",
            "sprites/orangelight",
            0.018 * SPRITE_SCALE,
            128.0,
            white,
        );
        for a in GLOWS {
            add(
                Kind::Glow,
                a,
                "sprites/glow04_noz",
                0.05 * SPRITE_SCALE,
                24.0,
                [255.0, 128.0, 0.0],
            );
        }
        for a in END_CAPS {
            add(
                Kind::EndCap,
                a,
                "sprites/grav_flare",
                0.02 * SPRITE_SCALE,
                128.0,
                white,
            );
        }
        add(
            Kind::MuzzleGlow,
            "Inside_effects",
            "sprites/portalgun_effects",
            0.025 * SPRITE_SCALE,
            64.0,
            white,
        );
        for a in TUBE {
            add(
                Kind::Tube,
                a,
                "sprites/portalgun_effects",
                BEAM_SIZE * SPRITE_SCALE,
                255.0,
                white,
            );
        }
        GunFx {
            effects,
            state: State::None,
            pulse: 0.0,
            pulse_up: false,
            max_size: EFFECTS_MAX_SIZE,
            world: false,
        }
    }

    /// DoEffectReady, DoEffectHolding or DoEffectNone.
    pub fn set_state(&mut self, state: State, last_fired: Option<u8>, now: f32) {
        self.state = state;
        let colour = portal_colour(last_fired);
        for e in &mut self.effects {
            match (state, e.kind) {
                (State::None, _) => e.visible = false,
                (_, Kind::Glow) => {
                    let (scale, alpha) = if state == State::Holding {
                        (0.5, 64.0)
                    } else {
                        (0.4, 32.0)
                    };
                    let (factor, duration) = if self.world { (2.0, 0.4) } else { (1.0, 0.2) };
                    e.scale
                        .init_from_current(scale * factor * SPRITE_SCALE, duration, now);
                    e.alpha.init_from_current(alpha, 0.2, now);
                    e.visible = true;
                }
                (_, Kind::EndCap) => e.visible = state == State::Holding,
                (_, Kind::MuzzleGlow | Kind::Tube) => {
                    e.color = colour;
                    e.visible = true;
                }
                _ => {}
            }
        }
    }

    /// ClientThink with DoEffectIdle: the fired-last light, the glows' flicker and the inner pulse.
    pub fn think(
        &mut self,
        last_fired: Option<u8>,
        dt: f32,
        now: f32,
        random: &mut impl FnMut() -> f32,
    ) {
        if self.pulse_up {
            self.pulse += dt;
            if self.pulse > 1.0 {
                self.pulse = 1.0;
                self.pulse_up = false;
            }
        } else {
            self.pulse -= dt;
            if self.pulse < 0.0 {
                self.pulse = 0.0;
                self.pulse_up = true;
            }
        }
        let colour = portal_colour(last_fired);
        let pulsed_alpha = EFFECTS_MIN_ALPHA + (EFFECTS_MAX_ALPHA - EFFECTS_MIN_ALPHA) * self.pulse;
        let active = self.state != State::None;
        for e in &mut self.effects {
            match e.kind {
                Kind::GravLight if active => e.visible = false,
                Kind::Portal1Light if active => e.visible = last_fired == Some(1),
                Kind::Portal2Light if active => e.visible = last_fired == Some(2),
                Kind::Glow => {
                    let factor = if self.world { 2.0 } else { 1.0 };
                    let scale = random_float(random, 0.0075 * factor, 0.05 * factor) * SPRITE_SCALE;
                    e.scale.init_from_current(scale, 0.1, now);
                    let alpha = random_int(random, 10, 24);
                    e.alpha.set_absolute(alpha, now);
                }
                Kind::EndCap => {
                    let factor = if self.world { 2.0 } else { 1.0 };
                    let scale = random_float(random, factor, 3.0 * factor);
                    e.scale.set_absolute(scale, now);
                    let alpha = random_int(random, 96, 128);
                    e.alpha.set_absolute(alpha, now);
                }
                Kind::MuzzleGlow => {
                    e.scale.set_absolute(
                        EFFECTS_MIN_SIZE + (self.max_size - EFFECTS_MIN_SIZE) * self.pulse,
                        now,
                    );
                    e.alpha.set_absolute(pulsed_alpha, now);
                    e.color = colour;
                }
                Kind::Tube => {
                    e.alpha.set_absolute(pulsed_alpha, now);
                    e.color = colour;
                }
                _ => {}
            }
        }
    }

    /// How many sprites can be on at once.
    pub fn capacity(&self) -> usize {
        self.effects.len()
    }

    /// The sprites to draw now.
    pub fn sprites(&self, now: f32) -> Vec<Sprite> {
        self.effects
            .iter()
            .filter(|e| e.visible)
            .map(|e| Sprite {
                attachment: e.attachment,
                material: e.material,
                color: e.color,
                alpha: e.alpha.value(now).trunc(),
                size: e.scale.value(now),
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BeamPoint {
    pub fraction: f32,
    pub offset: f32,
    pub radius: f32,
    pub texture: f32,
}

// Translated from 0x5d2e00 (decompiled, client.so)
pub fn beam_noise(random: &mut impl FnMut() -> f32) -> [f32; 129] {
    fn fill(noise: &mut [f32], scale: f32, random: &mut impl FnMut() -> f32) {
        let n = noise.len() - 1;
        if n <= 1 {
            return;
        }
        let mid = n / 2;
        noise[mid] = ((random() * scale) as f64 + (noise[0] + noise[n]) as f64 * 0.5) as f32;
        fill(&mut noise[mid..], scale * 0.5, random);
        fill(&mut noise[..=mid], scale * 0.5, random);
    }
    let mut noise = [0.0; 129];
    fill(&mut noise, 1.0, random);
    noise
}

impl Beam {
    // Translated from 0x5d2f00 and 0x2fe1f0 (decompiled, client.so)
    pub fn points(&self, length: f32, frequency: f32, noise: &[f32; 129]) -> Vec<BeamPoint> {
        if (length as f64) < 0.1 {
            return Vec::new();
        }
        let mut count = (length as f64 * 0.25 + 3.0) as usize;
        let half_width = self.width.max(self.end_width) * 0.5;
        if ((length * (1.0 / (count - 1) as f64) as f32) as f64) < 1.414 * half_width as f64 {
            count = ((0.7072135785007072 * length as f64 / half_width as f64) as usize + 1).max(2);
        }
        count = count.min(128);
        let step = (1.0 / (count - 1) as f64) as f32;
        let noise_step = (127.0 * 65536.0 * step) as usize;
        let length_scale = (length as f64 * 0.01) as f32;
        let mut texture = (frequency * self.speed) % 1.0;
        (0..count)
            .map(|i| {
                let fraction = i as f32 * step;
                let point = BeamPoint {
                    fraction,
                    offset: self.amplitude * length_scale * noise[(i * noise_step) >> 16],
                    radius: self.width + fraction * (self.end_width - self.width),
                    texture,
                };
                texture += step * length_scale;
                point
            })
            .collect()
    }
}
