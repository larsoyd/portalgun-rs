use portal::pair::Vec3;

const DIST_EPSILON: f32 = 0.03125;

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
    let delta = sub(end, start);
    let mut result = Trace {
        fraction: 1.0,
        end,
        normal: [0.0; 3],
        start_solid: false,
        all_solid: false,
        block: None,
    };
    for &(block, lo, hi) in solids {
        let lo = sub(lo, maxs);
        let hi = sub(hi, mins);
        let inside = (0..3).all(|i| start[i] > lo[i] && start[i] < hi[i]);
        if inside {
            result.start_solid = true;
            result.all_solid |= (0..3).all(|i| end[i] > lo[i] && end[i] < hi[i]);
            result.block = Some(block);
            continue;
        }
        let mut near = f32::NEG_INFINITY;
        let mut far = f32::INFINITY;
        let mut normal = [0.0; 3];
        for i in 0..3 {
            if delta[i] == 0.0 {
                // Contact without motion into a face must allow sliding along it.
                if start[i] <= lo[i] || start[i] >= hi[i] {
                    far = f32::NEG_INFINITY;
                    break;
                }
            } else {
                let a = (lo[i] - start[i]) / delta[i];
                let b = (hi[i] - start[i]) / delta[i];
                if a.min(b) > near {
                    near = a.min(b);
                    normal = [0.0; 3];
                    normal[i] = -delta[i].signum();
                }
                far = far.min(a.max(b));
            }
        }
        if (0.0..=1.0).contains(&near) && near < far {
            let mut entry = f32::NEG_INFINITY;
            for i in 0..3 {
                if delta[i] == 0.0 {
                    continue;
                }
                let distance = if delta[i] > 0.0 {
                    lo[i] - DIST_EPSILON - start[i]
                } else {
                    hi[i] + DIST_EPSILON - start[i]
                };
                let fraction = distance / delta[i];
                if fraction > entry {
                    entry = fraction;
                    normal = [0.0; 3];
                    normal[i] = -delta[i].signum();
                }
            }
            let fraction = entry.max(0.0);
            if fraction >= result.fraction {
                continue;
            }
            result.fraction = fraction;
            result.normal = normal;
            result.block = Some(block);
        }
    }
    if result.all_solid {
        result.fraction = 0.0;
    }
    result.end = add(start, scale(delta, result.fraction));
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

/// Four sweeps and up to five contact planes, from server.so 0x2763b0.
pub fn slide(
    start: Vec3,
    velocity: Vec3,
    dt: f32,
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
