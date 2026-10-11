// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

//! The initializers Portal's particle files use, from their scalar and block paths in client.so.

use super::collection::{Collection, Particle, Vec3};
use super::operators::Frame;
use super::random::pow_fixed;
use crate::pcf::Function;

pub const XYZ: u32 = 1;
pub const LIFE: u32 = 1 << 1;
pub const PREV: u32 = 1 << 2;
pub const RADIUS: u32 = 1 << 3;
pub const ROTATION: u32 = 1 << 4;
pub const TINT: u32 = 1 << 6;
pub const ALPHA: u32 = 1 << 7;
pub const CREATION: u32 = 1 << 8;
pub const SEQUENCE: u32 = 1 << 9;
pub const TRAIL: u32 = 1 << 10;

const ONE_255: f32 = 1.0 / 255.0;
/// the scalar normalize adds this under the root (0x7f2588)
const NORM_EPS: f32 = 1e-10;

/// The world as Velocity Repulse traces it: the fraction of a solid line trace for a collision group.
pub trait Query {
    fn trace(&self, start: Vec3, end: Vec3, group: i32) -> f32;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Range {
    pub min: f32,
    pub max: f32,
    pub exponent: f32,
}

/// Position Within Sphere Random. Control points never carry an entity here, so its create in
/// model test (MovePointInsideControllingObject with no object) accepts the first try.
#[derive(Debug, Clone, PartialEq)]
pub struct Sphere {
    pub min: f32,
    pub max: f32,
    pub bias: Vec3,
    pub bias_abs: Vec3,
    pub bias_local: bool,
    pub cp: usize,
    pub speed: Range,
    pub local_min: Vec3,
    pub local_max: Vec3,
    pub highest: bool,
    pub growth: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Remap {
    pub start: f32,
    pub end: f32,
    pub input: u32,
    pub in_min: f32,
    pub in_max: f32,
    pub output: u32,
    pub scale_initial: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Repulse {
    pub min: Vec3,
    pub max: Vec3,
    pub group: i32,
    pub cp: usize,
    pub per_particle: bool,
    pub radius_length: bool,
    pub offset: bool,
    pub proportional: bool,
    pub length: f32,
    pub inherit: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Initializer {
    Alpha(Range),
    Lifetime(Range),
    Radius(Range),
    Trail(Range),
    Rotation {
        range: Range,
        initial: f32,
    },
    Sequence {
        min: i32,
        max: i32,
    },
    Color {
        c1: Vec3,
        c2: Vec3,
    },
    Offset {
        cp: usize,
        min: Vec3,
        max: Vec3,
        local: bool,
        proportional: bool,
    },
    Warp {
        cp: usize,
        min: Vec3,
        max: Vec3,
        time: f32,
        start: f32,
        reverse: bool,
    },
    Sphere(Sphere),
    RemapScalar {
        remap: Remap,
        out_min: f32,
        out_max: f32,
        in_range_only: bool,
    },
    RemapVector {
        remap: Remap,
        out_min: Vec3,
        out_max: Vec3,
        local: bool,
        cp: usize,
    },
    Repulse(Repulse),
}

fn cp_number(f: &Function, name: &str) -> usize {
    f.int_or(name, 0).clamp(0, 63) as usize
}

fn range(f: &Function, min: &str, max: &str, exponent: &str, default: f32) -> Range {
    Range {
        min: f.float_or(min, default),
        max: f.float_or(max, default),
        exponent: f.float_or(exponent, 1.0),
    }
}

fn rgb(c: [u8; 4]) -> Vec3 {
    [0, 1, 2].map(|k| c[k] as f32 * ONE_255)
}

/// GetCollisionGroupFromName's table, unknown names falling back to none.
fn collision_group(name: &str) -> i32 {
    let groups = [
        ("DEBRIS", 1),
        ("INTERACTIVE", 4),
        ("NPC", 9),
        ("ACTOR", 18),
        ("PASSABLE", 15),
    ];
    groups
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map_or(0, |g| g.1)
}

fn remap(f: &Function, start: f32) -> Remap {
    Remap {
        start: f.float_or("emitter lifetime start time (seconds)", start),
        end: f.float_or("emitter lifetime end time (seconds)", start),
        input: f.int_or("input field", 8) as u32,
        in_min: f.float_or("input minimum", 0.0),
        in_max: f.float_or("input maximum", 1.0),
        output: 0,
        scale_initial: f.bool_or("output is scalar of initial random range", false),
    }
}

impl Initializer {
    /// Names match without case, as client.so compares them.
    pub fn from_function(f: &Function) -> Option<Initializer> {
        Some(match f.name.to_ascii_lowercase().as_str() {
            "alpha random" => Initializer::Alpha(Range {
                min: f.int_or("alpha_min", 255) as f32 * ONE_255,
                max: f.int_or("alpha_max", 255) as f32 * ONE_255,
                exponent: f.float_or("alpha_random_exponent", 1.0),
            }),
            "lifetime random" => Initializer::Lifetime(range(
                f,
                "lifetime_min",
                "lifetime_max",
                "lifetime_random_exponent",
                0.0,
            )),
            "radius random" => Initializer::Radius(range(
                f,
                "radius_min",
                "radius_max",
                "radius_random_exponent",
                1.0,
            )),
            "trail length random" => Initializer::Trail(range(
                f,
                "length_min",
                "length_max",
                "length_random_exponent",
                0.1,
            )),
            "rotation random" => {
                let rad = |name, default: f32| {
                    (f.float_or(name, default) as f64 * (std::f64::consts::PI / 180.0)) as f32
                };
                Initializer::Rotation {
                    range: Range {
                        min: rad("rotation_offset_min", 0.0),
                        max: rad("rotation_offset_max", 360.0),
                        exponent: f.float_or("rotation_random_exponent", 1.0),
                    },
                    initial: rad("rotation_initial", 0.0),
                }
            }
            "sequence random" => Initializer::Sequence {
                min: f.int_or("sequence_min", 0),
                max: f.int_or("sequence_max", 0),
            },
            // tinting by the engine's lighting at a control point is not ported
            "color random" if f.float_or("tint_perc", 0.0) == 0.0 => Initializer::Color {
                c1: rgb(f.color_or("color1", [255; 4])),
                c2: rgb(f.color_or("color2", [255; 4])),
            },
            "position modify offset random" => Initializer::Offset {
                cp: cp_number(f, "control_point_number"),
                min: f.vec3_or("offset min", [0.0; 3]),
                max: f.vec3_or("offset max", [0.0; 3]),
                local: f.bool_or("offset in local space 0/1", false),
                proportional: f.bool_or("offset proportional to radius 0/1", false),
            },
            "position modify warp random" => Initializer::Warp {
                cp: cp_number(f, "control point number"),
                min: f.vec3_or("warp min", [1.0; 3]),
                max: f.vec3_or("warp max", [1.0; 3]),
                time: f.float_or(
                    "warp transition time (treats min/max as start/end sizes)",
                    0.0,
                ),
                start: f.float_or("warp transition start time", 0.0),
                reverse: f.bool_or("reverse warp (0/1)", false),
            },
            "position within sphere random" => Initializer::Sphere(Sphere {
                min: f.float_or("distance_min", 0.0),
                max: f.float_or("distance_max", 0.0),
                bias: f.vec3_or("distance_bias", [1.0; 3]),
                bias_abs: f.vec3_or("distance_bias_absolute_value", [0.0; 3]),
                bias_local: f.bool_or("bias in local system", false),
                cp: cp_number(f, "control_point_number"),
                speed: range(f, "speed_min", "speed_max", "speed_random_exponent", 0.0),
                local_min: f.vec3_or("speed_in_local_coordinate_system_min", [0.0; 3]),
                local_max: f.vec3_or("speed_in_local_coordinate_system_max", [0.0; 3]),
                highest: f.bool_or(
                    "randomly distribute to highest supplied Control Point",
                    false,
                ),
                growth: f.float_or("randomly distribution growth time", 0.0),
            }),
            "remap initial scalar" => Initializer::RemapScalar {
                remap: Remap {
                    output: f.int_or("output field", 3) as u32,
                    ..remap(f, -1.0)
                },
                out_min: f.float_or("output minimum", 0.0),
                out_max: f.float_or("output maximum", 1.0),
                in_range_only: f.bool_or("only active within specified input range", false),
            },
            "remap scalar to vector" => Initializer::RemapVector {
                remap: Remap {
                    output: f.int_or("output field", 0) as u32,
                    ..remap(f, -1.0)
                },
                out_min: f.vec3_or("output minimum", [0.0; 3]),
                out_max: f.vec3_or("output maximum", [1.0; 3]),
                local: f.bool_or("use local system", true),
                cp: usize::try_from(f.int_or("control_point_number", 0))
                    .ok()
                    .filter(|cp| *cp < 64)?,
            },
            "velocity repulse from world" => {
                // broadcasting to children is not ported, and inheriting reads the next point
                let inherit = f.bool_or("Inherit from Parent", false);
                let cp = cp_number(f, "control_point_number");
                if f.int_or("control points to broadcast to children (n + 1)", -1) != -1
                    || (inherit && cp == 63)
                {
                    return None;
                }
                Initializer::Repulse(Repulse {
                    min: f.vec3_or("minimum velocity", [0.0; 3]),
                    max: f.vec3_or("maximum velocity", [1.0; 3]),
                    group: collision_group(f.string_or("collision group", "NONE")),
                    cp,
                    per_particle: f.bool_or("Per Particle World Collision Tests", false),
                    radius_length: f.bool_or("Use radius for Per Particle Trace Length", false),
                    offset: f.bool_or("Offset instead of accelerate", false),
                    proportional: f.bool_or("Offset proportional to radius 0/1", false),
                    length: f.float_or("Trace Length", 64.0),
                    inherit,
                })
            }
            _ => return None,
        })
    }

    /// GetWrittenAttributes.
    pub fn writes(&self) -> u32 {
        match self {
            Initializer::Alpha(_) => ALPHA,
            Initializer::Lifetime(_) => LIFE,
            Initializer::Radius(_) => RADIUS,
            Initializer::Trail(_) => TRAIL,
            Initializer::Rotation { .. } => ROTATION,
            Initializer::Sequence { .. } => SEQUENCE,
            Initializer::Color { .. } => TINT,
            Initializer::RemapScalar { remap, .. } => 1 << remap.output,
            Initializer::RemapVector { remap, .. } => (1 << remap.output) | PREV,
            _ => XYZ | PREV,
        }
    }

    /// InitMultipleOverride: modifiers run in a second pass after every other initializer.
    pub fn modifier(&self) -> bool {
        matches!(
            self,
            Initializer::Offset { .. }
                | Initializer::Warp { .. }
                | Initializer::RemapScalar { .. }
                | Initializer::RemapVector { .. }
                | Initializer::Repulse(_)
        )
    }

    /// The splitter (0x67d620): bursts of 16 or more take the block path for their aligned middle.
    pub fn init(&mut self, c: &mut Collection, first: usize, n: usize, mask: u32, q: &dyn Query) {
        if n == 0 {
            return;
        }
        if n < 16 {
            self.scalar(c, first, n, mask, q);
            return;
        }
        let (mut first, mut n) = (first, n);
        if first & 3 != 0 {
            let head = 4 - (first & 3);
            self.scalar(c, first, head, mask, q);
            first += head;
            n -= head;
        }
        if !self.block(c, first, n & !3, mask) {
            self.scalar(c, first, n & !3, mask, q);
        }
        if n & 3 != 0 {
            self.scalar(c, first + (n & !3), n & 3, mask, q);
        }
    }

    fn scalar(&mut self, c: &mut Collection, first: usize, n: usize, mask: u32, q: &dyn Query) {
        let span = first..first + n;
        match self {
            Initializer::Alpha(r) => random_attribute(c, span, *r, 7),
            Initializer::Lifetime(r) => random_attribute(c, span, *r, 1),
            Initializer::Radius(r) => random_attribute(c, span, *r, 3),
            Initializer::Trail(r) => random_attribute(c, span, *r, 10),
            Initializer::Rotation { range: r, initial } => {
                for i in span {
                    c.particles[i].rotation =
                        c.random_float_exp(r.min, r.max, r.exponent) + *initial;
                }
            }
            Initializer::Sequence { min, max } => {
                for i in span {
                    c.particles[i].sequence = c.random_int(*min, *max) as f32;
                }
            }
            Initializer::Color { c1, c2 } => {
                for i in span {
                    let r = c.random();
                    c.particles[i].tint = lerp(*c1, *c2, r);
                }
            }
            Initializer::Offset {
                cp,
                min,
                max,
                local,
                proportional,
            } => {
                for i in span {
                    let p = c.particles[i];
                    let off = if *proportional {
                        c.random_vector(scale(*min, p.radius), scale(*max, p.radius))
                    } else {
                        c.random_vector(*min, *max)
                    };
                    let off = if *local {
                        rotate(&frame_at(c, *cp, p.creation), off)
                    } else {
                        off
                    };
                    let p = &mut c.particles[i];
                    p.xyz = add(p.xyz, off);
                    p.prev = add(p.prev, off);
                }
            }
            Initializer::Warp {
                cp,
                min,
                max,
                time,
                start,
                reverse,
            } => {
                for i in span {
                    let ct = c.particles[i].creation;
                    let w = if *time == 0.0 {
                        c.random_vector(*min, *max)
                    } else {
                        let (a, b) = if *reverse { (*max, *min) } else { (*min, *max) };
                        let t = if *start + *time == *start {
                            if *start + *time <= ct {
                                1.0
                            } else {
                                0.0
                            }
                        } else {
                            ((ct - *start) / *time).clamp(0.0, 1.0)
                        };
                        [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t)
                    };
                    let m = frame_at(c, *cp, ct);
                    let warp = |v: Vec3| transform(&m, mul(itransform(&m, v), w));
                    let p = &mut c.particles[i];
                    p.xyz = warp(p.xyz);
                    p.prev = warp(p.prev);
                }
            }
            Initializer::Sphere(s) => {
                for i in span {
                    sphere_scalar(s, c, i, mask);
                }
            }
            Initializer::RemapScalar {
                remap,
                out_min,
                out_max,
                in_range_only,
            } => {
                let (mut lo, mut hi) = (*out_min, *out_max);
                if remap.output == 7 || remap.output == 16 {
                    lo = lo.clamp(0.0, 1.0);
                    hi = hi.clamp(0.0, 1.0);
                }
                for p in &mut c.particles[span] {
                    if !remap.active(p) {
                        continue;
                    }
                    let x = p.scalar(remap.input);
                    if *in_range_only && (x < remap.in_min || x > remap.in_max) {
                        continue;
                    }
                    let mut y = remap.map(x, lo, hi);
                    if remap.scale_initial {
                        y *= p.scalar(remap.output);
                    }
                    if remap.output == 11 {
                        y = y as i32 as f32;
                    }
                    p.set_scalar(remap.output, y);
                }
            }
            Initializer::RemapVector {
                remap,
                out_min,
                out_max,
                local,
                cp,
            } => {
                for i in span {
                    let p = c.particles[i];
                    if !remap.active(&p) {
                        continue;
                    }
                    let v = remap.map_vector(p.scalar(remap.input), *out_min, *out_max);
                    if remap.output != 0 {
                        if let Some(out) = c.particles[i].vector(remap.output) {
                            *out = if remap.scale_initial { mul(v, *out) } else { v };
                        }
                        continue;
                    }
                    let (xyz, prev) = if *local {
                        let v = transform(&frame_at(c, *cp, p.creation), v);
                        let v = if remap.scale_initial {
                            mul(v, p.xyz)
                        } else {
                            v
                        };
                        (v, v)
                    } else {
                        let v = add(v, c.cp_at(*cp, p.creation));
                        if remap.scale_initial {
                            (mul(v, p.xyz), mul(v, p.prev))
                        } else {
                            (v, v)
                        }
                    };
                    c.particles[i].xyz = xyz;
                    c.particles[i].prev = prev;
                }
            }
            Initializer::Repulse(r) => repulse_scalar(r, c, span, q),
        }
    }

    /// The SIMD block path where the class has one, drawing from the global SIMD stream.
    fn block(&mut self, c: &mut Collection, first: usize, n: usize, mask: u32) -> bool {
        let span = first..first + n;
        match self {
            Initializer::Alpha(r) => random_block(c, span, *r, 7),
            Initializer::Lifetime(r) => random_block(c, span, *r, 1),
            Initializer::Radius(r) => random_block(c, span, *r, 3),
            Initializer::Trail(r) => random_block(c, span, *r, 10),
            // rotation_initial is left out here, as 0x660350 does
            Initializer::Rotation { range: r, .. } => random_block(c, span, *r, 4),
            Initializer::Sequence { min, max } => {
                let r = Range {
                    min: *min as f32,
                    max: *max as f32,
                    exponent: 1.0,
                };
                random_block(c, span, r, 9)
            }
            Initializer::Color { c1, c2 } => {
                for b in span.step_by(4) {
                    let r = c.simd.draw();
                    for (lane, p) in c.particles[b..b + 4].iter_mut().enumerate() {
                        p.tint = lerp(*c1, *c2, r[lane]);
                    }
                }
            }
            Initializer::Sphere(s) if !s.highest && !(s.bias_local && s.bias != [1.0; 3]) => {
                for b in span.step_by(4) {
                    sphere_block(s, c, b, mask);
                }
            }
            _ => return false,
        }
        true
    }
}

impl Remap {
    fn active(&self, p: &Particle) -> bool {
        (self.start <= p.creation && p.creation < self.end)
            || self.start == -1.0
            || self.end == -1.0
    }

    fn map(&self, x: f32, lo: f32, hi: f32) -> f32 {
        if self.in_min == self.in_max {
            return if x >= self.in_max { hi } else { lo };
        }
        lo + (hi - lo) * ((x - self.in_min) / (self.in_max - self.in_min)).clamp(0.0, 1.0)
    }

    /// The vector remap scales by the reciprocal of the input span (0x657640).
    fn map_vector(&self, x: f32, lo: Vec3, hi: Vec3) -> Vec3 {
        if self.in_min == self.in_max {
            return if x >= self.in_max { hi } else { lo };
        }
        lerp(
            lo,
            hi,
            ((x - self.in_min) * (1.0 / (self.in_max - self.in_min))).clamp(0.0, 1.0),
        )
    }
}

fn lerp(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t)
}

fn random_attribute(c: &mut Collection, span: std::ops::Range<usize>, r: Range, attr: u32) {
    for i in span {
        let v = c.random_float_exp(r.min, r.max, r.exponent);
        c.particles[i].set_scalar(attr, v);
    }
}

/// 0x659b00, or 0x659bc0 with the exponent cut to whole quarters when it is not 1.
fn random_block(c: &mut Collection, span: std::ops::Range<usize>, r: Range, attr: u32) {
    for b in span.step_by(4) {
        let draw = c.simd.draw();
        for (lane, p) in c.particles[b..b + 4].iter_mut().enumerate() {
            let x = if r.exponent != 1.0 {
                pow_fixed(draw[lane], r.exponent)
            } else {
                draw[lane]
            };
            p.set_scalar(attr, r.min + (r.max - r.min) * x);
        }
    }
}

/// RandomVectorInUnitSphere (0x67f490): a direction and length from three entries after id.
fn unit_sphere(c: &Collection, id: u32) -> (Vec3, f32) {
    let u =
        [0, 1, 2].map(|k| c.table.get(id.wrapping_add(c.seed).wrapping_add(k)) * 0.9999 + 0.0001);
    let theta = (1.0 - 2.0 * u[0]).acos();
    let phi = (u[1] as f64 * std::f64::consts::TAU) as f32;
    let len = u[2].cbrt();
    let (st, ct) = theta.sin_cos();
    let (sp, cp) = phi.sin_cos();
    ([st * cp * len, st * sp * len, ct * len], len)
}

fn sphere_scalar(s: &Sphere, c: &mut Collection, i: usize, mask: u32) {
    let ct = c.particles[i].creation;
    let mut cp = s.cp;
    if s.highest {
        let f = if s.growth == 0.0 {
            1.0
        } else {
            c.cur_time.min(s.growth) / s.growth
        };
        let hi = (c.highest_cp as f32 * f).floor();
        cp = (s.cp as i32 + ((hi + 1.0 - s.cp as f32) * c.random()) as i32) as usize;
    }
    let id = c.query_count;
    c.query_count = c.query_count.wrapping_add(1);
    let (v, len) = unit_sphere(c, id);
    let v = [0, 1, 2].map(|k| {
        if s.bias_abs[k] != 0.0 {
            v[k].abs()
        } else {
            v[k]
        }
    });
    let b = mul(v, s.bias);
    let dir = scale(b, 1.0 / (dot(b, b) + NORM_EPS).sqrt());
    let mut p = scale(dir, s.min + (s.max - s.min) * len);
    if s.bias != [1.0; 3] && s.bias_local {
        p = transform(&frame_at(c, cp, ct), p);
    } else {
        p = add(p, c.cp_at(cp, ct));
    }
    c.particles[i].xyz = p;
    if mask & PREV == 0 {
        return;
    }
    let mut vel = [0.0; 3];
    if s.speed.max > 0.0 {
        vel = scale(
            dir,
            c.random_float_exp(s.speed.min, s.speed.max, s.speed.exponent),
        );
    }
    let l = [0, 1, 2].map(|k| c.random_float(s.local_min[k], s.local_max[k]));
    let vel = add(vel, along(&c.control_points[cp], l));
    c.particles[i].prev = sub(p, scale(vel, c.prev_dt));
}

/// 0x655320: four particles from the SIMD stream, rejection sampled inside the unit cube's ball.
fn sphere_block(s: &Sphere, c: &mut Collection, b: usize, mask: u32) {
    let draw3 = |c: &mut Collection| [c.simd.draw(), c.simd.draw(), c.simd.draw()];
    let mut r = draw3(c);
    let mut v = [0usize; 4].map(|_| [0.0f32; 3]);
    let fill = |v: &mut [Vec3; 4], r: &[[f32; 4]; 3], lanes: [bool; 4]| {
        for lane in 0..4 {
            if lanes[lane] {
                v[lane] = [0, 1, 2].map(|k| 2.0 * r[k][lane] - 1.0);
            }
        }
    };
    fill(&mut v, &r, [true; 4]);
    let mut out = v.map(|v| dot(v, v) > 1.0);
    while out.iter().any(|o| *o) {
        r = draw3(c);
        fill(&mut v, &r, out);
        out = v.map(|v| dot(v, v) >= 1.0);
    }
    let point = c.control_points[s.cp];
    let t = if c.dt > 0.0 { 1.0 / c.dt } else { 0.0 };
    let dirs = v.map(|v| sphere_dir(s, &v));
    for lane in 0..4 {
        let p = &mut c.particles[b + lane];
        let len = dot(v[lane], v[lane]).sqrt();
        let at = lerp(
            point.prev,
            point.pos,
            (p.creation - (c.cur_time - c.dt)) * t,
        );
        p.xyz = add(at, scale(dirs[lane], s.min + len * (s.max - s.min)));
    }
    if mask & PREV == 0 {
        return;
    }
    if s.speed.max <= 0.0 && s.local_min == [0.0; 3] && s.local_max == [0.0; 3] {
        for p in &mut c.particles[b..b + 4] {
            p.prev = p.xyz;
        }
        return;
    }
    let speed = c.simd.draw();
    let local = [c.simd.draw(), c.simd.draw(), c.simd.draw()];
    for lane in 0..4 {
        let sp =
            s.speed.min + (s.speed.max - s.speed.min) * pow_fixed(speed[lane], s.speed.exponent);
        let l =
            [0, 1, 2].map(|k| s.local_min[k] + (s.local_max[k] - s.local_min[k]) * local[k][lane]);
        let vel = add(scale(dirs[lane], sp), along(&point, l));
        let p = &mut c.particles[b + lane];
        p.prev = sub(p.xyz, scale(vel, c.prev_dt));
    }
}

/// Local speed along forward, right and up (the sphere's own mapping, +right).
fn along(p: &super::collection::ControlPoint, l: Vec3) -> Vec3 {
    add(
        add(scale(p.forward, l[0]), scale(p.right, l[1])),
        scale(p.up, l[2]),
    )
}

fn sphere_dir(s: &Sphere, v: &Vec3) -> Vec3 {
    let a = [0, 1, 2].map(|k| {
        if s.bias_abs[k] > 0.0 {
            v[k].abs()
        } else {
            v[k]
        }
    });
    let b = mul(a, s.bias);
    scale(b, 1.0 / dot(b, b).sqrt())
}

const DIRECTIONS: [Vec3; 6] = [
    [1.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, -1.0, 0.0],
    [0.0, 0.0, 1.0],
    [0.0, 0.0, -1.0],
];

/// The six traces: a push away from what they hit, as strong as the hits are close.
fn repulse(r: &Repulse, d: &mut [Vec3; 6], start: Vec3, length: f32, q: &dyn Query) -> Vec3 {
    let mut e = 0.0;
    let mut s = [0.0; 3];
    for dir in d.iter_mut() {
        let f = q.trace(start, add(start, scale(*dir, length)), r.group);
        *dir = scale(*dir, f - 1.0);
        e += 1.0 - f;
        s = add(s, *dir);
    }
    if s == [0.0; 3] {
        return [0.0; 3];
    }
    let n = scale(s, e / (dot(s, s) + NORM_EPS).sqrt());
    mul(n, [0, 1, 2].map(|k| r.min[k] + (r.max[k] - r.min[k]) * e))
}

fn repulse_scalar(r: &Repulse, c: &mut Collection, span: std::ops::Range<usize>, q: &dyn Query) {
    let mut d = DIRECTIONS;
    if r.per_particle {
        // the direction table is not reset between particles in the original
        for i in span {
            let p = c.particles[i];
            let length = if r.radius_length { p.radius } else { r.length };
            let mut v = repulse(r, &mut d, p.xyz, length, q);
            if r.proportional {
                v = scale(v, p.radius);
            }
            push(&mut c.particles[i], v, r.offset);
        }
        return;
    }
    let first = c.particles[span.start].creation;
    let mut v = if r.inherit {
        let e = c.cp_at(r.cp + 1, first)[0];
        let n = c.cp_at(r.cp, first);
        mul(n, [0, 1, 2].map(|k| r.min[k] + (r.max[k] - r.min[k]) * e))
    } else {
        repulse(r, &mut d, c.cp_at(r.cp, first), r.length, q)
    };
    for i in span {
        // the radius scale builds up across particles, as 0x649514 multiplies in place
        if r.proportional {
            v = scale(v, c.particles[i].radius);
        }
        push(&mut c.particles[i], v, r.offset);
    }
}

fn push(p: &mut Particle, v: Vec3, offset: bool) {
    p.xyz = add(p.xyz, v);
    if offset {
        p.prev = add(p.prev, v);
    }
}

/// GetControlPointTransformAtTime: forward, -right and up, at the position for that time.
fn frame_at(c: &Collection, cp: usize, t: f32) -> Frame {
    let p = &c.control_points[cp];
    let o = c.cp_at(cp, t);
    let (f, r, u) = (p.forward, p.right, p.up);
    [
        [f[0], -r[0], u[0], o[0]],
        [f[1], -r[1], u[1], o[1]],
        [f[2], -r[2], u[2], o[2]],
    ]
}

fn rotate(m: &Frame, v: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

fn transform(m: &Frame, v: Vec3) -> Vec3 {
    add(rotate(m, v), [m[0][3], m[1][3], m[2][3]])
}

fn itransform(m: &Frame, v: Vec3) -> Vec3 {
    let d = sub(v, [m[0][3], m[1][3], m[2][3]]);
    [0, 1, 2].map(|j| m[0][j] * d[0] + m[1][j] * d[1] + m[2][j] * d[2])
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn mul(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}

fn scale(a: Vec3, s: f32) -> Vec3 {
    a.map(|v| v * s)
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// InitializeNewParticles (0x67eba0): two passes, a writer skipped once its attributes are set.
pub fn initialize(
    c: &mut Collection,
    inits: &mut [Initializer],
    first: usize,
    n: usize,
    initted: u32,
    q: &dyn Query,
) {
    let mut left = !initted;
    for pass_modifiers in [false, true] {
        for init in inits.iter_mut().filter(|i| i.modifier() == pass_modifiers) {
            let w = init.writes();
            if !pass_modifiers && w & left == 0 {
                continue;
            }
            init.init(c, first, n, left, q);
            left &= !w;
        }
    }
}
