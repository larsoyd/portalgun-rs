//! The operators Portal's particle files use, each from its Operate in client.so.

use std::cmp::Ordering;

use super::collection::{Collection, Fade, Particle, Vec3};
use super::random::pow_fixed;
use crate::pcf::Function;

const ONE_255: f32 = 1.0 / 255.0;

#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    FadeAndKill {
        start_in: f32,
        end_in: f32,
        start_out: f32,
        end_out: f32,
        start_alpha: f32,
        end_alpha: f32,
    },
    FadeIn {
        min: f32,
        max: f32,
        exponent: f32,
        proportional: bool,
    },
    FadeOut {
        min: f32,
        max: f32,
        exponent: f32,
        bias: f32,
        proportional: bool,
        ease: bool,
    },
    ColorFade {
        target: Vec3,
        start: f32,
        end: f32,
        ease: bool,
    },
    Cull {
        percent: f32,
        start: f32,
        end: f32,
        exponent: f32,
    },
    Decay,
    Movement {
        gravity: Vec3,
        drag: f32,
        passes: i32,
    },
    DampenToCp {
        cp: usize,
        range: f32,
        scale: f32,
    },
    PositionLock {
        cp: usize,
        start_fade: [f32; 3],
        end_fade: [f32; 3],
        range: f32,
        lock_rotation: bool,
        prev: Option<(Vec3, Frame)>,
    },
    Oscillate(Oscillate),
    RadiusScale {
        start: f32,
        end: f32,
        start_scale: f32,
        end_scale: f32,
        ease: bool,
        bias: f32,
    },
}

/// A control point's frame as GetControlPointTransformAtTime builds it: forward, -right, up, origin.
pub type Frame = [[f32; 4]; 3];

#[derive(Debug, Clone, PartialEq)]
pub struct Oscillate {
    pub rate_min: Vec3,
    pub rate_max: Vec3,
    pub freq_min: Vec3,
    pub freq_max: Vec3,
    pub field: u32,
    pub proportional: bool,
    pub start_end_proportional: bool,
    pub start: [f32; 2],
    pub end: [f32; 2],
    pub multiplier: f32,
    pub phase: f32,
}

pub fn fade(f: &Function) -> Fade {
    Fade {
        start_in: f.float_or("operator start fadein", 0.0),
        end_in: f.float_or("operator end fadein", 0.0),
        start_out: f.float_or("operator start fadeout", 0.0),
        end_out: f.float_or("operator end fadeout", 0.0),
        oscillate: f.float_or("operator fade oscillate", 0.0),
    }
}

/// PreCalcBias: the precalculated bias term Bias(x) = x / ((1 - x) * p + 1) uses.
fn bias_term(b: f32) -> f32 {
    let r = 1.0 / b;
    (r + r) - ((b * r) * r + 2.0)
}

fn bias(x: f32, p: f32) -> f32 {
    x / ((1.0 - x) * p + 1.0)
}

fn spline(x: f32) -> f32 {
    (3.0 - 2.0 * x) * (x * x)
}

fn clamp01(x: f32) -> f32 {
    1.0f32.min(0.0f32.max(x))
}

fn age(c: &Collection, p: &Particle) -> f32 {
    c.cur_time - p.creation
}

impl Operator {
    /// The operator a .pcf function names with its InitParams fix-ups, None when it is not ported.
    pub fn from_function(f: &Function) -> Option<Operator> {
        Some(match f.name.to_ascii_lowercase().as_str() {
            "alpha fade and decay" => {
                let (si, so) = (
                    f.float_or("start_fade_in_time", 0.0),
                    f.float_or("start_fade_out_time", 0.5),
                );
                let ei = f.float_or("end_fade_in_time", 0.5).max(si);
                let eo = f.float_or("end_fade_out_time", 1.0).max(so);
                let (start_in, start_out) = if so < si { (so, si) } else { (si, so) };
                let (end_in, end_out) = if eo < ei { (eo, ei) } else { (ei, eo) };
                Operator::FadeAndKill {
                    start_in,
                    end_in,
                    start_out,
                    end_out,
                    start_alpha: f.float_or("start_alpha", 1.0),
                    end_alpha: f.float_or("end_alpha", 0.0),
                }
            }
            "alpha fade in random" => Operator::FadeIn {
                min: f.float_or("fade in time min", 0.25),
                max: f.float_or("fade in time max", 0.25),
                exponent: f.float_or("fade in time exponent", 1.0),
                proportional: f.bool_or("proportional 0/1", true),
            },
            "alpha fade out random" => {
                let (mut min, mut max) = (
                    f.float_or("fade out time min", 0.25),
                    f.float_or("fade out time max", 0.25),
                );
                if min == 0.0 && max == 0.0 {
                    (min, max) = (f32::EPSILON, f32::EPSILON);
                }
                let b = f.float_or("fade bias", 0.5);
                Operator::FadeOut {
                    min,
                    max,
                    exponent: f.float_or("fade out time exponent", 1.0),
                    bias: bias_term(if b == 0.0 { 0.5 } else { b }),
                    proportional: f.bool_or("proportional 0/1", true),
                    ease: f.bool_or("ease in and out", true),
                }
            }
            "color fade" => {
                let c = f.color_or("color_fade", [255; 4]);
                Operator::ColorFade {
                    target: [
                        c[0] as f32 * ONE_255,
                        c[1] as f32 * ONE_255,
                        c[2] as f32 * ONE_255,
                    ],
                    start: f.float_or("fade_start_time", 0.0),
                    end: f.float_or("fade_end_time", 1.0),
                    ease: f.bool_or("ease_in_and_out", true),
                }
            }
            "cull random" => Operator::Cull {
                percent: f.float_or("Cull Percentage", 0.5),
                start: f.float_or("Cull Start Time", 0.0),
                end: f.float_or("Cull End Time", 1.0),
                exponent: f.float_or("Cull Time Exponent", 1.0),
            },
            "lifespan decay" => Operator::Decay,
            "movement basic" => Operator::Movement {
                gravity: f.vec3_or("gravity", [0.0; 3]),
                drag: f.float_or("drag", 0.0),
                passes: f.int_or("max constraint passes", 3),
            },
            "movement dampen relative to control point" => Operator::DampenToCp {
                cp: f.int_or("control_point_number", 0).clamp(0, 63) as usize,
                range: f.float_or("falloff range", 100.0),
                scale: f.float_or("dampen scale", 1.0),
            },
            "movement lock to control point" => Operator::PositionLock {
                cp: f.int_or("control_point_number", 0).clamp(0, 63) as usize,
                start_fade: [
                    f.float_or("start_fadeout_min", 1.0),
                    f.float_or("start_fadeout_max", 1.0),
                    f.float_or("start_fadeout_exponent", 1.0),
                ],
                end_fade: [
                    f.float_or("end_fadeout_min", 1.0),
                    f.float_or("end_fadeout_max", 1.0),
                    f.float_or("end_fadeout_exponent", 1.0),
                ],
                range: f.float_or("distance fade range", 0.0),
                lock_rotation: f.bool_or("lock rotation", false),
                prev: None,
            },
            "oscillate vector" => Operator::Oscillate(Oscillate {
                rate_min: f.vec3_or("oscillation rate min", [0.0; 3]),
                rate_max: f.vec3_or("oscillation rate max", [0.0; 3]),
                freq_min: f.vec3_or("oscillation frequency min", [1.0; 3]),
                freq_max: f.vec3_or("oscillation frequency max", [1.0; 3]),
                field: f.int_or("oscillation field", 0).max(0) as u32,
                proportional: f.bool_or("proportional 0/1", true),
                start_end_proportional: f.bool_or("start/end proportional", true),
                start: [
                    f.float_or("start time min", 0.0),
                    f.float_or("start time max", 0.0),
                ],
                end: [
                    f.float_or("end time min", 1.0),
                    f.float_or("end time max", 1.0),
                ],
                multiplier: f.float_or("oscillation multiplier", 2.0),
                phase: f.float_or("oscillation start phase", 0.5),
            }),
            "radius scale" => {
                let b = f.float_or("scale_bias", 0.5);
                let b = if b == 0.0 { 0.5 } else { b };
                Operator::RadiusScale {
                    start: f.float_or("start_time", 0.0),
                    end: f.float_or("end_time", 1.0),
                    start_scale: f.float_or("radius_start_scale", 1.0),
                    end_scale: f.float_or("radius_end_scale", 1.0),
                    ease: f.bool_or("ease_in_and_out", false),
                    bias: b,
                }
            }
            _ => return None,
        })
    }

    /// One Operate over every particle. Movement is the system's integrator and runs elsewhere.
    pub fn operate(&mut self, c: &mut Collection, strength: f32) {
        match self {
            Operator::FadeAndKill {
                start_in,
                end_in,
                start_out,
                end_out,
                start_alpha,
                end_alpha,
            } => {
                let (inv_in, inv_out) =
                    (1.0 / (*end_in - *start_in), 1.0 / (*end_out - *start_out));
                for i in 0..c.particles.len() {
                    let p = c.particles[i];
                    let a = age(c, &p);
                    if p.life <= a {
                        c.kill(i);
                        continue;
                    }
                    let t = a / p.life;
                    let init = p.initial.alpha;
                    let mut alpha = p.alpha;
                    if *start_in <= t && t < *end_in {
                        let goal = *start_alpha * init;
                        let x = clamp01((t - *start_in) * inv_in);
                        alpha = ((3.0 - 2.0 * x) * (init - goal)) * (x * x) + goal;
                    }
                    if *start_out <= t && t < *end_out {
                        let x = clamp01((t - *start_out) * inv_out);
                        alpha = (3.0 - x * 2.0) * ((x * x) * ((*end_alpha - 1.0) * init)) + init;
                    }
                    c.particles[i].alpha = alpha;
                }
            }
            Operator::FadeIn {
                min,
                max,
                exponent,
                proportional,
            } => {
                for i in 0..c.particles.len() {
                    let p = c.particles[i];
                    let fade_time = (*max - *min) * pow_fixed(c.op_random(&p, 0), *exponent) + *min;
                    let a = age(c, &p);
                    let t = if *proportional {
                        0.0f32.max(1.0f32.min(a / p.life))
                    } else {
                        a
                    };
                    if t < fade_time {
                        let x = clamp01(t / fade_time);
                        c.particles[i].alpha = ((3.0 - 2.0 * x) * p.initial.alpha) * (x * x);
                    }
                }
            }
            Operator::FadeOut {
                min,
                max,
                exponent,
                bias: p_bias,
                proportional,
                ease,
            } => {
                for i in 0..c.particles.len() {
                    let p = c.particles[i];
                    let fade_time = pow_fixed(c.op_random(&p, 0), *exponent) * (*max - *min) + *min;
                    let a = age(c, &p);
                    let (t, start) = if *proportional {
                        (a / p.life, 1.0 - fade_time)
                    } else {
                        (a, p.life - fade_time)
                    };
                    if start < t {
                        let x = clamp01((t - start) / fade_time);
                        let init = p.initial.alpha;
                        c.particles[i].alpha = if *ease {
                            0.0f32.max(init + (x * x) * ((3.0 - 2.0 * x) * (0.0 - init)))
                        } else {
                            (1.0 - bias(x, *p_bias)) * init
                        };
                    }
                }
            }
            Operator::ColorFade {
                target,
                start,
                end,
                ease,
            } => {
                if *end == *start {
                    return;
                }
                let inv = 1.0 / (*end - *start);
                for p in c.particles.iter_mut().filter(|p| 0.0 < p.life) {
                    let x = clamp01(((c.cur_time - p.creation) / p.life - *start) * inv);
                    let s = if *ease { spline(x) } else { x };
                    for ((tint, initial), target) in
                        p.tint.iter_mut().zip(p.initial.tint).zip(*target)
                    {
                        *tint = (target - initial) * s + initial;
                    }
                }
            }
            Operator::Cull {
                percent,
                start,
                end,
                exponent,
            } => {
                for i in 0..c.particles.len() {
                    let p = c.particles[i];
                    if c.op_random(&p, 15) > strength * *percent {
                        continue;
                    }
                    let t = (c.cur_time - p.creation) / p.life;
                    let t = 1.0f32.min(t.max(0.0));
                    let cut = *start + (*end - *start) * c.op_random(&p, 12).powf(*exponent);
                    if *end >= t && t >= *start && t >= cut {
                        c.kill(i);
                    }
                }
            }
            Operator::Decay => {
                for i in 0..c.particles.len() {
                    let p = c.particles[i];
                    if p.life <= age(c, &p) || p.life <= 0.0 {
                        c.kill(i);
                    }
                }
            }
            Operator::Movement { .. } => {}
            Operator::DampenToCp { cp, range, scale } => {
                if 0.0 >= *range {
                    return;
                }
                let at = c.control_points[*cp].pos;
                for p in c.particles.iter_mut() {
                    let d = [p.xyz[0] - at[0], p.xyz[1] - at[1], p.xyz[2] - at[2]];
                    let dist = ((d[1] * d[1] + d[0] * d[0]) + d[2] * d[2]).sqrt();
                    if dist > *range {
                        continue;
                    }
                    let f = (dist / *range).powf(*scale);
                    for k in 0..3 {
                        p.xyz[k] +=
                            ((p.prev[k] - p.xyz[k]) + (p.xyz[k] - p.prev[k]) * f) * strength;
                    }
                }
            }
            Operator::PositionLock { .. } => position_lock(self, c, strength),
            Operator::Oscillate(o) => oscillate(o, c, strength),
            Operator::RadiusScale {
                start,
                end,
                start_scale,
                end_scale,
                ease,
                bias: b,
            } => {
                if *start >= *end {
                    return;
                }
                let inv = 1.0 / (*end - *start);
                let sw = *end_scale - *start_scale;
                let p_bias = bias_term(*b);
                for p in c.particles.iter_mut() {
                    let t = (c.cur_time - p.creation) / p.life;
                    if !(0.0 < p.life && *start <= t && t < *end) {
                        continue;
                    }
                    let x = (t - *start) * inv;
                    let scale = if *ease {
                        (sw * (3.0 - 2.0 * x)) * (x * x) + *start_scale
                    } else if *b == 0.5 {
                        (t - *start) * (inv * sw) + *start_scale
                    } else {
                        bias(x, p_bias) * sw + *start_scale
                    };
                    p.radius = scale * p.initial.radius;
                }
            }
        }
    }
}

/// GetControlPointTransformAtTime's frame (orientation is not interpolated).
pub fn cp_frame(c: &Collection, cp: usize) -> Frame {
    let p = &c.control_points[cp];
    let (f, r, u, o) = (p.forward, p.right, p.up, p.pos);
    [
        [f[0], -r[0], u[0], o[0]],
        [f[1], -r[1], u[1], o[1]],
        [f[2], -r[2], u[2], o[2]],
    ]
}

fn concat(a: &Frame, b: &Frame) -> Frame {
    let mut out = [[0.0; 4]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = a[i][0] * b[0][j]
                + a[i][1] * b[1][j]
                + a[i][2] * b[2][j]
                + if j == 3 { a[i][3] } else { 0.0 };
        }
    }
    out
}

/// MatrixInvert of a rigid frame: the transposed rotation with -R^T t.
fn invert(m: &Frame) -> Frame {
    let mut out = [[0.0; 4]; 3];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = m[j][i];
        }
        out[i][3] = -(m[0][i] * m[0][3] + m[1][i] * m[1][3] + m[2][i] * m[2][3]);
    }
    out
}

/// C_OP_PositionLock: particles move with their control point, fading out over a random part of life.
fn position_lock(op: &mut Operator, c: &mut Collection, strength: f32) {
    let Operator::PositionLock {
        cp,
        start_fade,
        end_fade,
        range,
        lock_rotation,
        prev,
    } = op
    else {
        return;
    };
    let at = c.control_points[*cp].pos;
    let now = cp_frame(c, *cp);
    let (prev_pos, prev_frame) = *prev.get_or_insert((at, now));
    let m = concat(&now, &invert(&prev_frame));
    let d = [0, 1, 2].map(|k| (at[k] - prev_pos[k]) * strength);
    let inv_dt = 1.0 / c.dt;
    let inv_range = 1.0 / *range;
    let bp = bias_term(0.2);
    // a NaN start takes the fade path too
    let fade_path = !matches!(
        start_fade[0].partial_cmp(&1.0),
        Some(Ordering::Greater | Ordering::Equal)
    );
    for i in 0..c.particles.len() {
        let p = c.particles[i];
        let frac = inv_dt * age(c, &p).min(c.dt);
        let (s, k) = if !fade_path {
            let mut s = d.map(|v| v * frac);
            let mut k = strength;
            if *range != 0.0 {
                let q = [0, 1, 2].map(|j| (s[j] + p.xyz[j]) - at[j]);
                let b = falloff(q, inv_range, bp);
                s = s.map(|v| v * (1.0 - b));
                k = 1.0 - b * strength;
            }
            (s, k)
        } else {
            if i % 4 == 0 {
                let r = c.simd.draw();
                let r2 = c.simd.draw();
                c.lock_fade = Some((r, r2));
            }
            let (r, r2) = c.lock_fade.unwrap_or_default();
            let pct = 0.0f32.max(1.0f32.min(age(c, &p) / p.life));
            let st = (start_fade[1] - start_fade[0]) * pow_fixed(r[i % 4], start_fade[2])
                + start_fade[0];
            let en = (end_fade[1] - end_fade[0]) * pow_fixed(r2[i % 4], end_fade[2]) + end_fade[0];
            let lock = 1.0 - 0.0f32.max(1.0f32.min((pct - st) / (en - st)));
            if lock.partial_cmp(&0.0) != Some(Ordering::Greater) {
                continue;
            }
            let mut s = d.map(|v| v * (frac * lock));
            let mut k = lock;
            if *range != 0.0 {
                let q = [0, 1, 2].map(|j| (p.xyz[j] - at[j]) + s[j]);
                let b = falloff(q, inv_range, bp);
                k = 1.0 - lock * b;
                s = s.map(|v| v * (1.0 - b));
            }
            (s, k)
        };
        let q = &mut c.particles[i];
        if !*lock_rotation {
            for ((x, prev), s) in q.xyz.iter_mut().zip(q.prev.iter_mut()).zip(s) {
                *x += s;
                *prev += s;
            }
        } else {
            let kk = frac * k;
            for v in [&mut q.xyz, &mut q.prev] {
                let o = *v;
                for r in 0..3 {
                    v[r] = (((m[r][3] - o[r]) + (m[r][1] * o[1] + m[r][0] * o[0]))
                        + m[r][2] * o[2])
                        * kk
                        + o[r];
                }
            }
        }
    }
    *prev = Some((at, now));
}

fn falloff(q: Vec3, inv_range: f32, bp: f32) -> f32 {
    let dist = ((q[0] * q[0] + q[1] * q[1]) + q[2] * q[2]).sqrt();
    bias(1.0f32.min(dist * inv_range), bp)
}

/// FastSin-style parabolic sine with period 2 (0x668e70): close to sin(pi x).
pub fn parabolic_sine(x: f32) -> f32 {
    const TWO_23: f32 = 8_388_608.0;
    let a = x.abs();
    let mut e = f32::from_bits((TWO_23 + a).to_bits() & 0xffff_fffe) - TWO_23;
    if a < e {
        e -= 2.0;
    }
    let u = a - e;
    let h = if 1.0 <= u { 1.0 } else { 0.0 };
    let s = ((e + h) + (1.0 - a)) * (((a - e) - h) * 4.0);
    let negative = x.is_sign_negative() != (h == 1.0);
    if negative {
        -s
    } else {
        s
    }
}

fn oscillate(o: &Oscillate, c: &mut Collection, strength: f32) {
    let sd = strength * c.dt;
    let t_all = c.cur_time * o.multiplier + o.phase;
    let fw = [0, 1, 2].map(|k| o.freq_max[k] - o.freq_min[k]);
    let rw = [0, 1, 2].map(|k| o.rate_max[k] - o.rate_min[k]);
    for i in 0..c.particles.len() {
        let p = c.particles[i];
        let a = age(c, &p);
        let t = if o.start_end_proportional {
            a / p.life
        } else {
            a
        };
        let st = (o.start[1] - o.start[0]) * c.op_random(&p, 11) + o.start[0];
        let en = c.op_random(&p, 12) * (o.end[1] - o.end[0]) + o.end[0];
        if !(st <= t && 0.0 < p.life && t < en) {
            continue;
        }
        let freq = [
            o.freq_min[0] + c.op_random(&p, 8) * fw[0],
            o.freq_min[1] + c.op_random(&p, 12) * fw[1],
            c.op_random(&p, 15) * fw[2] + o.freq_min[2],
        ];
        let rate = [
            c.op_random(&p, 3) * rw[0] + o.rate_min[0],
            c.op_random(&p, 7) * rw[1] + o.rate_min[1],
            c.op_random(&p, 9) * rw[2] + o.rate_min[2],
        ];
        let Some(v) = c.particles[i].vector(o.field) else {
            continue;
        };
        for k in 0..3 {
            let phase = if o.proportional {
                (freq[k] * o.multiplier) * (a / p.life) + o.phase
            } else {
                freq[k] * t_all
            };
            let mut out = v[k] + parabolic_sine(phase) * (rate[k] * sd);
            if o.field == 6 {
                out = out.clamp(0.0, 1.0);
            }
            v[k] = out;
        }
    }
}
