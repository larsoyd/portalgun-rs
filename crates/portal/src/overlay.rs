// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::carry::{add, cross, dot, sub};
use crate::error::malformed;
use crate::pair::Vec3;
use crate::vmf::{vec3, Entity};
use crate::{Error, Result};

const OVERLAYS: usize = 45;
const WATER: usize = 50;
const FADES: usize = 60;
const RECORD: usize = 0x160;
// constants of engine.so at 0x866c74, 0x85fff0, 0x83a10c, 0x839e24 and 0x839dd8
const TRIANGLE_AREA: f32 = 4.000_000_5;
const ON_PLANE: f32 = 1e-4;
const LIFT: f32 = 0.1;
const PARALLEL: f32 = 0.99;
const SAME_LENGTH: f32 = 0.1;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Overlay {
    pub faces: Vec<u32>,
    pub order: u8,
    pub u: [f32; 2],
    pub v: [f32; 2],
    pub points: [[f32; 2]; 4],
    pub origin: Vec3,
    /// U, V and the normal
    pub basis: [Vec3; 3],
    /// squared fade start and end distances
    pub fade: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub pos: Vec3,
    pub normal: Vec3,
    pub uv: [f32; 2],
    pub st: [f32; 2],
}

impl Overlay {
    /// Translated from 0x45ed10, 0x45d220, 0x45da60 and 0x45e8b0 (decompiled, engine.so)
    pub fn fragments(&self, face: &[Vec3], plane: (Vec3, f32)) -> Result<Vec<Vec<Vertex>>> {
        let quad = self.quad()?;
        let mut polys = Vec::new();
        for i in 1..face.len().saturating_sub(1) {
            let tri = [face[0], face[i], face[i + 1]];
            let area = cross(sub(tri[1], tri[0]), sub(tri[2], tri[0]));
            if dot(area, area) <= TRIANGLE_AREA {
                continue;
            }
            let normal = self.basis[2];
            let flat = tri.map(|p| sub(p, scale(normal, dot(sub(p, self.origin), normal))));
            let mut poly: Vec<Vertex> = quad.corners.to_vec();
            for j in 0..3 {
                let (a, b, c) = (flat[j], flat[(j + 1) % 3], flat[(j + 2) % 3]);
                let mut n = cross(normal, unit(sub(b, a)));
                let mut d = dot(n, a);
                if dot(n, c) - d > 0.0 {
                    (n, d) = (n.map(|v| -v), -d);
                }
                poly = clip(&poly, n, d);
            }
            if poly.is_empty() {
                continue;
            }
            polys.push(
                poly.iter()
                    .map(|v| quad.finish(v.pos, normal, plane))
                    .collect(),
            );
        }
        Ok(polys)
    }

    fn quad(&self) -> Result<Quad> {
        let [u, v, _] = self.basis;
        let st = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];
        let corners = [0, 1, 2, 3].map(|i| {
            let [x, y] = self.points[i];
            Vertex {
                pos: add(self.origin, add(scale(u, x), scale(v, y))),
                normal: [0.0; 3],
                uv: [self.u[st[i][0] as usize], self.v[st[i][1] as usize]],
                st: st[i],
            }
        });
        let (along, across) = (
            sub(corners[3].pos, corners[0].pos),
            sub(corners[1].pos, corners[0].pos),
        );
        let opposite = sub(corners[2].pos, corners[1].pos);
        let (length, other) = (dot(along, along).sqrt(), dot(opposite, opposite).sqrt());
        if dot(unit(along), unit(opposite)) <= PARALLEL || (length - other).abs() >= SAME_LENGTH {
            return Err(Error::Unsupported(format!(
                "overlay at {:?} is not a parallelogram",
                self.origin
            )));
        }
        Ok(Quad {
            corners,
            along,
            across,
        })
    }

    /// What vbsp writes for an info_overlay, read off the first chamber's accessors and their records
    pub fn from_keys(e: &Entity) -> Result<Overlay> {
        let key = |k: &str| {
            e.get(k)
                .ok_or_else(|| Error::Refused(format!("overlay without {k}")))
        };
        let number = |k: &str| -> Result<f32> {
            key(k)?
                .trim()
                .parse()
                .map_err(|_| malformed("overlay", format!("{k} unreadable")))
        };
        let mut points = [[0.0; 2]; 4];
        for (i, p) in points.iter_mut().enumerate() {
            let [x, y, _] = vec3(key(&format!("uv{i}"))?)?;
            *p = [x, y];
        }
        let (u, normal) = (vec3(key("BasisU")?)?, vec3(key("BasisNormal")?)?);
        let v = cross(normal, u);
        if dot(v, vec3(key("BasisV")?)?) < 0.99 {
            return Err(Error::Refused(
                "overlay BasisV is not BasisNormal cross BasisU, the flipped case is not ported"
                    .into(),
            ));
        }
        if e.get("fademaxdist").is_some_and(|d| d.trim() != "0") {
            return Err(Error::Refused("overlay fademaxdist is not ported".into()));
        }
        let faces = key("sides")?
            .split_whitespace()
            .map(|s| {
                s.parse()
                    .map_err(|_| malformed("overlay", format!("side {s}")))
            })
            .collect::<Result<_>>()?;
        Ok(Overlay {
            faces,
            order: e.get("RenderOrder").map_or(Ok(0), |o| {
                o.trim()
                    .parse()
                    .map_err(|_| malformed("overlay", "RenderOrder"))
            })?,
            u: [number("StartU")?, number("EndU")?],
            v: [number("StartV")?, number("EndV")?],
            points,
            origin: vec3(key("BasisOrigin")?)?,
            basis: [u, v, normal],
            fade: [number("fademindist")?, 0.0],
        })
    }
}

/// Translated from 0x45fe30 and 0x45b6e0 (decompiled, engine.so), the overlay records of a map
pub fn lump(bytes: &[u8]) -> Result<Vec<Overlay>> {
    let int = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .ok_or_else(|| malformed("bsp", format!("truncated at {at}")))
    };
    let float = |at: usize| int(at).map(f32::from_bits);
    let vector = |at: usize| -> Result<Vec3> { Ok([float(at)?, float(at + 4)?, float(at + 8)?]) };
    let span = |lump: usize| -> Result<(usize, usize)> {
        Ok((int(8 + lump * 16)? as usize, int(12 + lump * 16)? as usize))
    };
    let (start, length) = span(OVERLAYS)?;
    if length % RECORD != 0 {
        return Err(malformed("bsp", format!("overlay lump size {length}")));
    }
    if span(WATER)?.1 != 0 {
        return Err(Error::Refused("water overlays are not ported".into()));
    }
    let (fades, fade_length) = span(FADES)?;
    (0..length / RECORD)
        .map(|i| {
            let at = start + i * RECORD;
            let packed = int(at + 4)? >> 16;
            let mut points = [[0.0; 2]; 4];
            let mut z = [0.0; 4];
            for k in 0..4 {
                let p = vector(at + 0x118 + 12 * k)?;
                points[k] = [p[0], p[1]];
                z[k] = p[2];
            }
            let normal = vector(at + 0x154)?;
            let u = [z[0], z[1], z[2]];
            let v = cross(normal, u).map(|c| if z[3] == 1.0 { -c } else { c });
            Ok(Overlay {
                faces: (0..(packed & 0x3fff) as usize)
                    .map(|f| int(at + 8 + 4 * f))
                    .collect::<Result<_>>()?,
                order: (packed >> 14) as u8,
                u: [float(at + 0x108)?, float(at + 0x10c)?],
                v: [float(at + 0x110)?, float(at + 0x114)?],
                points,
                origin: vector(at + 0x148)?,
                basis: [u, v, normal],
                fade: if fade_length == 0 {
                    [-1.0, 0.0]
                } else {
                    [float(fades + 8 * i)?, float(fades + 8 * i + 4)?]
                },
            })
        })
        .collect()
}

struct Quad {
    corners: [Vertex; 4],
    along: Vec3,
    across: Vec3,
}

impl Quad {
    /// the parallelogram path of 0x21de50 and 0x21da00, then the face projection of 0x45cd10
    fn finish(&self, pos: Vec3, normal: Vec3, (face, distance): (Vec3, f32)) -> Vertex {
        let rel = sub(pos, self.corners[0].pos);
        let span = cross(self.along, self.across);
        let area = dot(span, span);
        let s = dot(cross(rel, self.across), span) / area;
        let t = dot(cross(self.along, rel), span) / area;
        let lerp = |f: fn(&Vertex) -> [f32; 2]| -> [f32; 2] {
            let [c0, c1, c2, c3] = self.corners.each_ref().map(f);
            let a = [0, 1].map(|k| c0[k] + (c1[k] - c0[k]) * t);
            let b = [0, 1].map(|k| c3[k] + (c2[k] - c3[k]) * t);
            [0, 1].map(|k| a[k] + (b[k] - a[k]) * s)
        };
        let unit_face = unit(face);
        let denominator = dot(unit_face, normal);
        let mut drop = dot(unit_face, pos) - distance;
        if denominator != 0.0 {
            drop /= denominator;
        }
        let on_face = sub(pos, scale(normal, drop));
        Vertex {
            pos: add(on_face, scale(face, LIFT)),
            normal: face,
            uv: lerp(|v| v.uv),
            st: lerp(|v| v.st),
        }
    }
}

/// 0x45d510, the part of a polygon behind a plane with points within the epsilon counted on it
fn clip(poly: &[Vertex], n: Vec3, d: f32) -> Vec<Vertex> {
    let dist: Vec<f32> = poly.iter().map(|v| dot(v.pos, n) - d).collect();
    let side = |x: f32| {
        if x > ON_PLANE {
            0
        } else if x >= -ON_PLANE {
            2
        } else {
            1
        }
    };
    if !dist.iter().any(|&x| side(x) == 0) {
        return poly.to_vec();
    }
    if !dist.iter().any(|&x| side(x) == 1) {
        return Vec::new();
    }
    let mut back = Vec::new();
    for i in 0..poly.len() {
        let j = (i + 1) % poly.len();
        let (here, next) = (side(dist[i]), side(dist[j]));
        if here != 0 {
            back.push(poly[i]);
        }
        if here != 2 && next != 2 && here != next {
            let f = dist[i] / (dist[i] - dist[j]);
            let mix = |a: f32, b: f32| a + (b - a) * f;
            let (a, b) = (poly[i], poly[j]);
            back.push(Vertex {
                pos: [0, 1, 2].map(|k| mix(a.pos[k], b.pos[k])),
                normal: [0.0; 3],
                uv: [0, 1].map(|k| mix(a.uv[k], b.uv[k])),
                st: [0, 1].map(|k| mix(a.st[k], b.st[k])),
            });
        }
    }
    back
}

fn scale(v: Vec3, f: f32) -> Vec3 {
    v.map(|c| c * f)
}

fn unit(v: Vec3) -> Vec3 {
    scale(v, 1.0 / dot(v, v).sqrt())
}
