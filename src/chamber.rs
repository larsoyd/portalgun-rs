// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::collections::HashMap;

use portal::brush;
use portal::keyvalues;
use portal::pair::{Portal, Vec3};
use portal::placement::{surface_of, RayHit, Surface, World};
use portal::surfaces::Surfaces;
use portal::view::{clip_poly, Plane as ClipPlane};
use portal::vmf::{self, TexAxis};
use portal::vpk::SearchPath;
use portal::Error;

use crate::collision::{dot, Plane};
use crate::simulator::bevelled;

const SANDBOX: &str = include_str!("../assets/chambers/sandbox.vmf");

/// nodraw still collides, vbsp just never emits its faces
const NODRAW: &str = "tools/toolsnodraw";

pub struct Side {
    pub plane: Plane,
    pub material: String,
    pub surface: Surface,
    /// the visible polygon wound outward, empty when no part of the side shows
    pub points: Vec<Vec3>,
    /// texel coordinates of the points
    pub uv: Vec<[f32; 2]>,
}

pub struct Block {
    pub min: Vec3,
    pub max: Vec3,
    pub sides: Vec<Side>,
    /// one per brush, footsteps and impacts look it up by block
    pub surfaceprop: String,
    planes: Vec<Plane>,
}

pub struct Chamber {
    pub blocks: Vec<Block>,
}

/// vbsp turns %noportal into the surface flag the gun checks
pub fn noportal(vmt: &str) -> portal::Result<bool> {
    let root = keyvalues::parse(vmt)?;
    Ok(root
        .first()
        .and_then(|shader| shader.get("%noportal"))
        .is_some_and(|v| v.trim() == "1"))
}

/// what the material walks and sounds like, the default surface when it names none
pub fn surfaceprop(vmt: &str) -> portal::Result<String> {
    let root = keyvalues::parse(vmt)?;
    Ok(root
        .first()
        .and_then(|shader| shader.get("$surfaceprop"))
        .map_or("default", str::trim)
        .to_ascii_lowercase())
}

struct Look {
    surface: Surface,
    surfaceprop: String,
}

fn look(search: &SearchPath, surfaces: &Surfaces, material: &str) -> portal::Result<Look> {
    let text =
        String::from_utf8_lossy(&search.read(&format!("materials/{material}.vmt"))?).into_owned();
    let prop = surfaceprop(&text)?;
    Ok(Look {
        surface: surface_of(material, noportal(&text)?, surfaces.get(&prop).material),
        surfaceprop: prop,
    })
}

impl Chamber {
    pub fn test_chamber(search: &SearchPath) -> portal::Result<Chamber> {
        Chamber::from_vmf(search, SANDBOX)
    }

    pub fn from_vmf(search: &SearchPath, text: &str) -> portal::Result<Chamber> {
        let map = vmf::parse(text)?;
        let surfaces = Surfaces::read(search)?;
        let mut looks: HashMap<String, Look> = HashMap::new();
        let mut blocks = Vec::new();
        for solid in &map.world {
            for side in &solid.sides {
                if !looks.contains_key(&side.material) {
                    looks.insert(
                        side.material.clone(),
                        look(search, &surfaces, &side.material)?,
                    );
                }
            }
            blocks.push(Block::from_solid(solid, &looks)?);
        }
        Ok(Chamber { blocks })
    }
}

impl Block {
    fn new(min: Vec3, max: Vec3, sides: Vec<Side>, surfaceprop: String) -> Block {
        let own: Vec<Plane> = sides.iter().map(|s| s.plane).collect();
        Block {
            min,
            max,
            planes: bevelled(&own),
            sides,
            surfaceprop,
        }
    }

    fn from_solid(solid: &vmf::Solid, looks: &HashMap<String, Look>) -> portal::Result<Block> {
        let polys = brush::faces(&solid.sides);
        let (min, max) = brush::bounds(&polys);
        let mut props = solid
            .sides
            .iter()
            .filter(|s| !s.material.starts_with("tools/"))
            .map(|s| looks[&s.material].surfaceprop.as_str());
        let surfaceprop = props.next().unwrap_or("default").to_string();
        if let Some(other) = props.find(|p| *p != surfaceprop) {
            return Err(Error::Refused(format!(
                "solid {} mixes surfaceprops {surfaceprop} and {other}",
                solid.id
            )));
        }
        let sides = solid
            .sides
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let poly = polys.iter().find(|p| p.side == i);
                Side {
                    plane: Plane {
                        normal: s.plane.normal,
                        distance: s.plane.dist,
                    },
                    material: s.material.clone(),
                    surface: looks[&s.material].surface,
                    points: poly.map(|p| p.points.clone()).unwrap_or_default(),
                    uv: poly.map(|p| p.uv.clone()).unwrap_or_default(),
                }
            })
            .collect();
        Ok(Block::new(min, max, sides, surfaceprop))
    }

    /// An axis-aligned box with Hammer's world-aligned texture axes on every side.
    pub fn cuboid(min: Vec3, max: Vec3, material: &str, surface: Surface, prop: &str) -> Block {
        let mut sides = Vec::new();
        for i in 0..3 {
            let (j, k) = ((i + 1) % 3, (i + 2) % 3);
            let (u, v) = world_axes(i);
            for (sign, at) in [(1.0, max[i]), (-1.0, min[i])] {
                let (a, b) = if sign > 0.0 { (j, k) } else { (k, j) };
                let points: Vec<Vec3> = [(0, 0), (1, 0), (1, 1), (0, 1)]
                    .iter()
                    .map(|&(x, y)| {
                        let mut p = [0.0; 3];
                        p[i] = at;
                        p[a] = if x == 0 { min[a] } else { max[a] };
                        p[b] = if y == 0 { min[b] } else { max[b] };
                        p
                    })
                    .collect();
                let mut normal = [0.0; 3];
                normal[i] = sign;
                sides.push(Side {
                    plane: Plane {
                        normal,
                        distance: sign * at,
                    },
                    material: material.to_string(),
                    surface,
                    uv: points
                        .iter()
                        .map(|&p| [texel(&u, p), texel(&v, p)])
                        .collect(),
                    points,
                });
            }
        }
        Block::new(min, max, sides, prop.to_string())
    }

    pub fn planes(&self) -> Vec<Plane> {
        self.planes.clone()
    }

    pub fn faces(&self) -> impl Iterator<Item = &Side> {
        self.sides
            .iter()
            .filter(|s| s.points.len() >= 3 && s.material != NODRAW)
    }

    /// convex ray entry, a ray starting inside the block does not hit it
    fn entry(&self, from: Vec3, dir: Vec3) -> Option<(f32, usize)> {
        let (mut near, mut far, mut hit) = (f32::NEG_INFINITY, f32::INFINITY, None);
        for (i, side) in self.sides.iter().enumerate() {
            let n = side.plane.normal;
            let gap = dot(n, from) - side.plane.distance;
            let rate = dot(n, dir);
            if rate == 0.0 {
                if gap > 0.0 {
                    return None;
                }
                continue;
            }
            let t = -gap / rate;
            if rate < 0.0 {
                if t > near {
                    (near, hit) = (t, Some(i));
                }
            } else {
                far = far.min(t);
            }
        }
        if near > far || near < 0.0 {
            return None;
        }
        hit.map(|i| (near, i))
    }
}

/// Hammer's world alignment for a side facing along the axis
fn world_axes(axis: usize) -> (TexAxis, TexAxis) {
    let unit = |k: usize, sign: f32| {
        let mut v = [0.0; 3];
        v[k] = sign;
        TexAxis {
            axis: v,
            shift: 0.0,
            scale: 0.25,
        }
    };
    match axis {
        0 => (unit(1, 1.0), unit(2, -1.0)),
        1 => (unit(0, 1.0), unit(2, -1.0)),
        _ => (unit(0, 1.0), unit(1, -1.0)),
    }
}

fn texel(t: &TexAxis, p: Vec3) -> f32 {
    dot(p, t.axis) / t.scale + t.shift
}

impl World for Chamber {
    fn clear_portal(&self, p: &Portal) -> bool {
        let (w, h) = p.half;
        let corners = [(-w, -h), (w, -h), (w, h), (-w, h)].map(|(r, u)| p.world([0.05, r, u]));
        // Clip the whole opening so buried corners and thin obstructions cannot escape ray samples.
        self.blocks.iter().all(|b| {
            let mut poly = corners.to_vec();
            for side in &b.sides {
                let inside = ClipPlane {
                    normal: side.plane.normal.map(|c| -c),
                    dist: -side.plane.distance,
                };
                poly = clip_poly(&poly, &inside, 0.0);
                if poly.len() < 3 {
                    return true;
                }
            }
            false
        })
    }

    fn cast(&self, from: Vec3, dir: Vec3, max: f32) -> Option<RayHit> {
        self.blocks
            .iter()
            .filter_map(|b| b.entry(from, dir).map(|(d, i)| (d, &b.sides[i])))
            .filter(|&(d, _)| d <= max)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(distance, side)| RayHit {
                distance,
                normal: side.plane.normal,
                surface: side.surface,
            })
    }
}
