//! Forces and constraints, which only Movement Basic runs (C_OP_BasicMovement 0x666970).

use super::collection::{Collection, Vec3};
use super::random::pow_fixed;
use crate::pcf::Function;

/// FLT_EPSILON as the SIMD code uses it for empty lengths (0x828e40).
const EPS: f32 = 1.192_092_9e-7;
/// 29.999998f at 0x82d7bc: drag is a retention per thirtieth of a second.
const DRAG_RATE: f32 = 29.999_998;

/// A world for the particles: a trace gives the fraction, the surface normal and a start-in-solid flag.
pub trait World {
    fn trace(&self, start: Vec3, end: Vec3, brush_only: bool) -> Trace;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trace {
    pub fraction: f32,
    pub normal: Vec3,
    pub start_solid: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Force {
    Pull {
        amount: f32,
        falloff: f32,
        cp: usize,
    },
    Random {
        min: Vec3,
        max: Vec3,
    },
    Twist {
        amount: f32,
        axis: Vec3,
        local: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    Trace {
        bounce: f32,
        slide: f32,
        radius_scale: f32,
        kill: bool,
        brush_only: bool,
    },
    Distance {
        min: f32,
        max: f32,
        cp: usize,
        offset: Vec3,
        global: bool,
    },
    Plane {
        point: Vec3,
        normal: Vec3,
        cp: usize,
        global_origin: bool,
        global_normal: bool,
    },
}

/// x > limit, false for NaN, as the SIMD masks compare
fn above(x: f32, limit: f32) -> bool {
    x.partial_cmp(&limit) == Some(std::cmp::Ordering::Greater)
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn scale(a: Vec3, s: f32) -> Vec3 {
    a.map(|v| v * s)
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// Local (x, y, z) to world along a control point's right, forward and up, as these operators map it.
fn local(c: &Collection, cp: usize, v: Vec3) -> Vec3 {
    let p = &c.control_points[cp];
    add(
        add(scale(p.right, v[0]), scale(p.forward, v[1])),
        scale(p.up, v[2]),
    )
}

fn normalized(v: Vec3) -> Vec3 {
    scale(v, 1.0 / dot(v, v).sqrt())
}

impl Force {
    pub fn from_function(f: &Function) -> Option<Force> {
        Some(match f.name.to_ascii_lowercase().as_str() {
            "pull towards control point" => Force::Pull {
                amount: f.float_or("amount of force", 0.0),
                falloff: f.float_or("falloff power", 2.0),
                cp: f.int_or("control point number", 0).clamp(0, 63) as usize,
            },
            "random force" => Force::Random {
                min: f.vec3_or("min force", [0.0; 3]),
                max: f.vec3_or("max force", [0.0; 3]),
            },
            "twist around axis" => Force::Twist {
                amount: f.float_or("amount of force", 0.0),
                axis: f.vec3_or("twist axis", [0.0, 0.0, 1.0]),
                local: f.bool_or("object local space axis 0/1", false),
            },
            _ => return None,
        })
    }

    /// AddForces: accelerations in units a second squared, one per particle.
    pub fn add(&self, c: &mut Collection, acc: &mut [Vec3], strength: f32) {
        match *self {
            Force::Pull {
                amount,
                falloff,
                cp,
            } => {
                let p = (falloff as f64 * -4.0) as i32;
                let s = -(amount * strength);
                let at = c.control_points[cp].pos;
                for (a, q) in acc.iter_mut().zip(&c.particles) {
                    let d = sub(q.xyz, at);
                    let len = dot(d, d).sqrt();
                    if !above(len, EPS) {
                        continue;
                    }
                    let pw = pow_fixed(len, p as f32 / 4.0);
                    *a = add(*a, scale(d, pw * (s / len)));
                }
            }
            Force::Random { min, max } => {
                let n = c.particles.len();
                for block in 0..n.div_ceil(4) {
                    let r = [c.simd.draw(), c.simd.draw(), c.simd.draw()];
                    for lane in 0..4.min(n - 4 * block) {
                        for k in 0..3 {
                            let (lo, hi) = (min[k] * strength, max[k] * strength);
                            acc[4 * block + lane][k] += lo + r[k][lane] * (hi - lo);
                        }
                    }
                }
            }
            Force::Twist {
                amount,
                axis,
                local: in_local,
            } => {
                let a = if in_local { local(c, 0, axis) } else { axis };
                let n = normalized(a);
                let at = c.control_points[0].pos;
                let k = amount * strength;
                for (acc, q) in acc.iter_mut().zip(&c.particles) {
                    let o = sub(q.xyz, at);
                    let l2 = dot(o, o);
                    if !above(l2, EPS) {
                        continue;
                    }
                    let u = scale(o, 1.0 / l2.sqrt());
                    let v = scale(u, 1.0 - dot(n, u));
                    let l2b = dot(v, v);
                    if !above(l2b, EPS) {
                        continue;
                    }
                    let w = scale(v, 1.0 / l2b.sqrt());
                    let f = [
                        w[1] * n[2] - w[2] * n[1],
                        w[2] * n[0] - w[0] * n[2],
                        w[0] * n[1] - w[1] * n[0],
                    ];
                    *acc = add(*acc, scale(f, k));
                }
            }
        }
    }
}

impl Constraint {
    pub fn from_function(f: &Function) -> Option<Constraint> {
        Some(match f.name.to_ascii_lowercase().as_str() {
            "collision via traces" => Constraint::Trace {
                bounce: f.float_or("amount of bounce", 0.0),
                slide: f.float_or("amount of slide", 0.0),
                radius_scale: f.float_or("radius scale", 1.0),
                kill: f.bool_or("kill particle on collision", false),
                brush_only: f.bool_or("brush only", false),
            },
            "constrain distance to control point" => Constraint::Distance {
                min: f.float_or("minimum distance", 0.0),
                max: f.float_or("maximum distance", 100.0),
                cp: f.int_or("control point number", 0).clamp(0, 63) as usize,
                offset: f.vec3_or("offset of center", [0.0; 3]),
                global: f.bool_or("global center point", false),
            },
            "prevent passing through a plane" => Constraint::Plane {
                point: f.vec3_or("plane point", [0.0; 3]),
                normal: f.vec3_or("plane normal", [0.0, 0.0, 1.0]),
                cp: f.int_or("control point number", 0).clamp(0, 63) as usize,
                global_origin: f.bool_or("global origin", false),
                global_normal: f.bool_or("global normal", false),
            },
            _ => return None,
        })
    }

    /// Final constraints run once after the passes (IsFinalConstraint).
    pub fn is_final(&self) -> bool {
        matches!(*self, Constraint::Trace { bounce, slide, .. } if bounce != 0.0 || slide != 0.0)
    }

    /// EnforceConstraint: true when any particle had to be moved.
    pub fn enforce(&self, c: &mut Collection, world: &dyn World) -> bool {
        match *self {
            Constraint::Trace {
                bounce,
                slide,
                radius_scale,
                kill,
                brush_only,
            } => {
                let mut any = false;
                for p in c.particles.iter_mut() {
                    let delta = sub(p.xyz, p.prev);
                    let l2 = dot(delta, delta);
                    if !above(l2, 0.0) {
                        continue;
                    }
                    let dir = scale(delta, 1.0 / l2.sqrt());
                    let end = add(p.xyz, scale(dir, p.radius * radius_scale));
                    let tr = world.trace(p.prev, end, brush_only);
                    if !above(1.0, tr.fraction) {
                        continue;
                    }
                    any = true;
                    if kill {
                        p.life = 0.0;
                        continue;
                    }
                    let mut h = add(p.prev, scale(delta, tr.fraction));
                    if bounce != 0.0 || slide != 0.0 {
                        let n = tr.normal;
                        let slid = scale(sub(delta, scale(n, dot(delta, n))), slide);
                        let bounced = scale(sub(dir, scale(n, 2.0 * dot(dir, n))), bounce);
                        let v = add(slid, bounced);
                        let left = if tr.start_solid {
                            0.0
                        } else {
                            1.0 - tr.fraction
                        };
                        h = add(h, scale(v, left));
                        p.prev = sub(h, v);
                    }
                    p.xyz = h;
                }
                any
            }
            Constraint::Distance {
                min,
                max,
                cp,
                offset,
                global,
            } => {
                let centre = if global {
                    offset
                } else {
                    add(c.control_points[cp].pos, local(c, cp, offset))
                };
                let mut any = false;
                for p in c.particles.iter_mut() {
                    let d = sub(p.xyz, centre);
                    let l2 = dot(d, d);
                    let close = l2 < min * min;
                    let far = max * max < l2;
                    if !(close || far) {
                        continue;
                    }
                    any = true;
                    let u = scale(d, 1.0 / (if l2 == 0.0 { EPS } else { l2 }).sqrt());
                    p.xyz = add(centre, scale(u, if close { min } else { max }));
                }
                any
            }
            Constraint::Plane {
                point,
                normal,
                cp,
                global_origin,
                global_normal,
            } => {
                let n = normalized(if global_normal {
                    normal
                } else {
                    local(c, cp, normal)
                });
                let at = if global_origin {
                    point
                } else {
                    add(c.control_points[cp].pos, local(c, cp, point))
                };
                let mut any = false;
                for p in c.particles.iter_mut() {
                    let dist = dot(sub(p.xyz, at), n) - p.radius;
                    if dist < 0.0 {
                        any = true;
                        p.xyz = sub(p.xyz, scale(n, dist));
                    }
                }
                any
            }
        }
    }
}

/// Movement Basic: Verlet with Source's frame-rate drag, then the constraint passes.
pub struct Movement {
    pub gravity: Vec3,
    pub drag: f32,
    pub passes: i32,
}

impl Movement {
    pub fn integrate(&self, c: &mut Collection, forces: &[(Force, f32)]) {
        let l = if 0.0 > self.drag {
            0.0
        } else {
            ((1.0 - self.drag as f64) as f32).ln()
        };
        let adj = (c.dt / c.prev_dt) * (l * (c.dt * DRAG_RATE)).exp();
        let mut acc = vec![self.gravity; c.particles.len()];
        for (force, strength) in forces {
            force.add(c, &mut acc, *strength);
        }
        let dt2 = c.dt * c.dt;
        for (p, a) in c.particles.iter_mut().zip(acc) {
            let moved = [0, 1, 2].map(|k| (p.xyz[k] - p.prev[k]) * adj + (p.xyz[k] + a[k] * dt2));
            p.prev = p.xyz;
            p.xyz = moved;
        }
    }

    /// The constraint passes: one that moved something sends the others round again, finals last.
    pub fn constrain(
        &self,
        c: &mut Collection,
        constraints: &[(Constraint, bool)],
        world: &dyn World,
    ) {
        if constraints.is_empty() || c.particles.is_empty() {
            return;
        }
        let mut satisfied = vec![false; constraints.len()];
        for _ in 0..self.passes {
            for i in 0..constraints.len() {
                if satisfied[i] {
                    continue;
                }
                satisfied[i] = true;
                let (con, runs) = &constraints[i];
                if !con.is_final() && *runs && con.enforce(c, world) {
                    for (j, s) in satisfied.iter_mut().enumerate() {
                        if j != i {
                            *s = false;
                        }
                    }
                }
            }
        }
        for (con, runs) in constraints {
            if con.is_final() && *runs {
                con.enforce(c, world);
            }
        }
    }
}
