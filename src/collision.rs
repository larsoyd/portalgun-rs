// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::movement::standable;
use portal::pair::Vec3;

/// the engine's trace skin, 0x829f88 in engine.so
pub const DIST_EPSILON: f32 = 0.03125;

#[derive(Debug, Clone, Copy)]
pub struct Trace {
    pub fraction: f32,
    pub end: Vec3,
    pub normal: Vec3,
    pub start_solid: bool,
    pub all_solid: bool,
    pub block: Option<usize>,
}

pub fn trace(
    start: Vec3,
    end: Vec3,
    mins: Vec3,
    maxs: Vec3,
    solids: &[(usize, Vec3, Vec3)],
) -> Trace {
    let brushes: Vec<_> = solids
        .iter()
        .map(|&(block, lo, hi)| (block, box_planes(lo, hi)))
        .collect();
    trace_brushes(start, end, mins, maxs, &brushes)
}

/// Convex solid half-space: dot(normal, point) <= distance.
#[derive(Clone, Copy)]
pub struct Plane {
    pub normal: Vec3,
    pub distance: f32,
}

pub fn box_planes(lo: Vec3, hi: Vec3) -> Vec<Plane> {
    (0..3)
        .flat_map(|axis| {
            let mut normal = [0.0; 3];
            normal[axis] = 1.0;
            [
                Plane {
                    normal,
                    distance: hi[axis],
                },
                Plane {
                    normal: scale(normal, -1.0),
                    distance: -lo[axis],
                },
            ]
        })
        .collect()
}

pub fn trace_brushes(
    start: Vec3,
    end: Vec3,
    mins: Vec3,
    maxs: Vec3,
    solids: &[(usize, Vec<Plane>)],
) -> Trace {
    let mut result = Trace {
        fraction: 1.0,
        end,
        normal: [0.0; 3],
        start_solid: false,
        all_solid: false,
        block: None,
    };
    for (block, planes) in solids {
        let mut near = f32::NEG_INFINITY;
        let mut entry = f32::NEG_INFINITY;
        let mut far = f32::INFINITY;
        let mut normal = [0.0; 3];
        let mut inside = true;
        let mut end_inside = true;
        for plane in planes {
            let support: f32 = (0..3)
                .map(|i| {
                    plane.normal[i]
                        * if plane.normal[i] >= 0.0 {
                            mins[i]
                        } else {
                            maxs[i]
                        }
                })
                .sum();
            let distance = plane.distance - support;
            let a = dot(plane.normal, start) - distance;
            let b = dot(plane.normal, end) - distance;
            inside &= a < 0.0;
            end_inside &= b < 0.0;
            if a == b {
                if a >= 0.0 {
                    far = f32::NEG_INFINITY;
                }
            } else if a > b {
                near = near.max(a / (a - b));
                let fraction = (a - DIST_EPSILON) / (a - b);
                if fraction > entry {
                    entry = fraction;
                    normal = plane.normal;
                }
            } else {
                far = far.min(a / (a - b));
            }
        }
        if inside {
            result.start_solid = true;
            result.all_solid |= end_inside;
            result.block = Some(*block);
        } else if (0.0..=1.0).contains(&near) && near < far && entry.max(0.0) < result.fraction {
            result.fraction = entry.max(0.0);
            result.normal = normal;
            result.block = Some(*block);
        }
    }
    if result.all_solid {
        result.fraction = 0.0;
    }
    result.end = add(start, scale(sub(end, start), result.fraction));
    result
}

pub fn dot(a: Vec3, b: Vec3) -> f32 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| a[i] + b[i])
}
pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| a[i] - b[i])
}
pub fn scale(a: Vec3, k: f32) -> Vec3 {
    a.map(|v| v * k)
}

/// Translated from 0x2762c0 (disassembled, server.so).
pub fn clip(v: Vec3, normal: Vec3) -> Vec3 {
    let mut out = sub(v, scale(normal, dot(v, normal)));
    let adjust = dot(out, normal);
    if adjust < 0.0 {
        out = sub(out, scale(normal, adjust));
    }
    out
}

/// Four sweeps and up to five contact planes.
/// Translated from 0x2763b0 (decompiled, server.so)
pub fn slide(
    start: Vec3,
    velocity: Vec3,
    dt: f32,
    airborne: bool,
    mut cast: impl FnMut(Vec3, Vec3) -> Trace,
) -> (Vec3, Vec3) {
    let mut at = start;
    let mut v = velocity;
    let mut original = velocity;
    let mut left = dt;
    let mut planes: Vec<Vec3> = Vec::new();
    let mut total_fraction = 0.0;
    for _ in 0..4 {
        if dot(v, v) == 0.0 {
            break;
        }
        let tr = cast(at, add(at, scale(v, left)));
        total_fraction += tr.fraction;
        if tr.all_solid {
            return (at, [0.0; 3]);
        }
        if tr.fraction == 1.0 {
            let endpoint = cast(tr.end, tr.end);
            if endpoint.start_solid || endpoint.fraction != 1.0 {
                return (at, [0.0; 3]);
            }
        }
        if tr.fraction > 0.0 {
            at = tr.end;
            original = v;
            planes.clear();
        }
        if tr.fraction == 1.0 {
            break;
        }
        left *= 1.0 - tr.fraction;
        if planes.len() == 5 {
            v = [0.0; 3];
            break;
        }
        planes.push(tr.normal);
        // one plane in the air clips once and skips the primal test
        if airborne && planes.len() == 1 {
            v = clip(original, tr.normal);
            original = v;
            continue;
        }
        let candidate = planes.iter().enumerate().find_map(|(i, &plane)| {
            let clipped = clip(original, plane);
            planes
                .iter()
                .enumerate()
                .all(|(j, &p)| j == i || dot(clipped, p) >= 0.0)
                .then_some(clipped)
        });
        v = match candidate {
            Some(v) => v,
            None if planes.len() == 2 => {
                let [a, b] = [planes[0], planes[1]];
                let crease = [
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                ];
                let len = dot(crease, crease).sqrt();
                if len == 0.0 {
                    [0.0; 3]
                } else {
                    let dir = scale(crease, 1.0 / len);
                    scale(dir, dot(dir, v))
                }
            }
            None => [0.0; 3],
        };
        if dot(v, velocity) <= 0.0 {
            v = [0.0; 3];
            break;
        }
    }
    if total_fraction == 0.0 {
        v = [0.0; 3];
    }
    (at, v)
}

/// The better of a plain move and one stepped up and back down.
/// Translated from 0x26f150 (decompiled, server.so)
pub fn step_move(
    start: Vec3,
    velocity: Vec3,
    dt: f32,
    step: f32,
    mut cast: impl FnMut(Vec3, Vec3) -> Trace,
) -> (Vec3, Vec3) {
    let (down, down_v) = slide(start, velocity, dt, false, &mut cast);
    let top = (step as f64 + start[2] as f64 + 0.03125) as f32;
    let up = cast(start, [start[0], start[1], top]);
    let from = match up.start_solid || up.all_solid {
        true => start,
        false => up.end,
    };
    let (mut raised, mut raised_v) = slide(from, velocity, dt, false, &mut cast);
    let bottom = (raised[2] as f64 - 0.03125 - step as f64) as f32;
    let descend = cast(raised, [raised[0], raised[1], bottom]);
    if !standable(descend.normal[2]) {
        return (down, down_v);
    }
    if !descend.start_solid && !descend.all_solid {
        raised = descend.end;
    }
    let distance = |p: Vec3| (p[0] - start[0]).powi(2) + (p[1] - start[1]).powi(2);
    if distance(down) > distance(raised) {
        return (down, down_v);
    }
    raised_v[2] = down_v[2];
    (raised, raised_v)
}
