// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::pair::Vec3;
use crate::vmf::{Side, TexAxis};

/// Hammer's world limit, every side starts as a quad this big
const HALF_EXTENT: f64 = 32768.0;
const ON_EPSILON: f64 = 0.01;

type DVec = [f64; 3];

pub struct Polygon {
    pub side: usize,
    pub points: Vec<Vec3>,
    /// texel coordinates, divide by the texture size for UVs
    pub uv: Vec<[f32; 2]>,
}

/// The visible face of every side, wound counter-clockwise seen from outside.
pub fn faces(sides: &[Side]) -> Vec<Polygon> {
    sides
        .iter()
        .enumerate()
        .filter_map(|(i, side)| {
            let mut poly = base_quad(side);
            for (j, other) in sides.iter().enumerate() {
                if j != i && poly.len() >= 3 {
                    poly = clip_behind(&poly, other);
                }
            }
            (poly.len() >= 3).then(|| polygon(i, side, &poly))
        })
        .collect()
}

pub fn bounds(polys: &[Polygon]) -> (Vec3, Vec3) {
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for p in polys.iter().flat_map(|p| &p.points) {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    (lo, hi)
}

fn polygon(side: usize, s: &Side, poly: &[DVec]) -> Polygon {
    let points: Vec<Vec3> = poly.iter().map(|p| p.map(|c| c as f32)).collect();
    let uv = points
        .iter()
        .map(|&p| [texel(&s.u, p), texel(&s.v, p)])
        .collect();
    Polygon { side, points, uv }
}

fn texel(t: &TexAxis, p: Vec3) -> f32 {
    (p[0] * t.axis[0] + p[1] * t.axis[1] + p[2] * t.axis[2]) / t.scale + t.shift
}

fn base_quad(side: &Side) -> Vec<DVec> {
    let n = side.plane.normal.map(f64::from);
    let up = if n[2].abs() > n[0].abs().max(n[1].abs()) {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    let right = normalize(cross(up, n));
    let up = cross(n, right);
    let centre = n.map(|c| c * f64::from(side.plane.dist));
    [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .iter()
        .map(|&(a, b)| [0, 1, 2].map(|k| centre[k] + (right[k] * a + up[k] * b) * HALF_EXTENT))
        .collect()
}

/// keeps the part of the polygon inside the other side's plane
fn clip_behind(poly: &[DVec], other: &Side) -> Vec<DVec> {
    let n = other.plane.normal.map(f64::from);
    let d = f64::from(other.plane.dist);
    let dists: Vec<f64> = poly.iter().map(|p| dot(*p, n) - d).collect();
    let mut out = Vec::new();
    for i in 0..poly.len() {
        let j = (i + 1) % poly.len();
        let (a, b) = (dists[i], dists[j]);
        if a <= ON_EPSILON {
            out.push(poly[i]);
        }
        if (a > ON_EPSILON && b < -ON_EPSILON) || (a < -ON_EPSILON && b > ON_EPSILON) {
            let t = a / (a - b);
            out.push([0, 1, 2].map(|k| poly[i][k] + (poly[j][k] - poly[i][k]) * t));
        }
    }
    out
}

fn dot(a: DVec, b: DVec) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: DVec, b: DVec) -> DVec {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: DVec) -> DVec {
    let len = dot(v, v).sqrt();
    v.map(|c| c / len)
}
