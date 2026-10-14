// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::pair::{through_dir, through_point, Portal, Vec3};

use crate::chamber::Chamber;
use crate::collision::{dot, trace_brushes, Plane, Trace};

pub type Solid = (usize, Vec<Plane>);

/// sv_portal_collision_sim_bounds_x, y and z never go below these
const SIM_BOUNDS: Vec3 = [200.0, 200.0, 252.0];
const FRONT_CLIP: f32 = 0.1;
const WALL_CLIP: f32 = 0.01;
const TUBE_BACK: f32 = 1.01;
const INNER: (f32, f32) = (32.1, 54.1);
const OUTER: (f32, f32) = (32.2, 54.2);
const FAR: f32 = 2000.0;
const WALL_DEPTH: f32 = 100.0;
const WALL_REACH: f32 = 200.0;
const HOLE_NEAR: f32 = 0.5;
const HOLE_FAR: f32 = 500.0;
/// the hole's half sizes, stored as 0.98 of 32 and 54
const HOLE: (f32, f32) = (31.36, 52.92);

/// The world the player moves in, and each portal's own collision for movers it owns.
pub struct Space {
    pub world: Vec<Solid>,
    pub pair: Option<[Portal; 2]>,
    sims: Vec<Sim>,
}

/// One portal's carved collision, CreatePolyhedrons.
/// Translated from 0x2c98e0 (decompiled, server.so)
struct Sim {
    front: Vec<Solid>,
    tube: Vec<Solid>,
    wall: Vec<Solid>,
    linked: Vec<Solid>,
    hole: Vec<Solid>,
    /// the block 500 units straight behind the portal, whose material every piece takes
    surface: usize,
}

impl Space {
    pub fn new(chamber: &Chamber, linked: Option<&[Portal; 2]>) -> Space {
        let world: Vec<Solid> = chamber
            .blocks
            .iter()
            .enumerate()
            .map(|(n, b)| (n, b.planes()))
            .collect();
        let sims = linked
            .map(|pair| {
                let fronts = pair.map(|p| front(chamber, &p));
                (0..2)
                    .map(|i| Sim {
                        front: bevel_all(fronts[i].clone()),
                        tube: bevel_all(tube(&pair[i])),
                        wall: bevel_all(wall(chamber, &pair[i])),
                        linked: bevel_all(carried(&fronts[1 - i], &pair[1 - i], &pair[i])),
                        surface: behind(&world, &pair[i]),
                        hole: bevel_all(vec![(
                            usize::MAX,
                            local_box(
                                &pair[i],
                                [-HOLE_FAR, -HOLE.0, -HOLE.1],
                                [-HOLE_NEAR, HOLE.0, HOLE.1],
                            ),
                        )]),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Space {
            world,
            pair: linked.copied(),
            sims,
        }
    }

    pub fn trace_oriented(
        &self,
        owner: Option<usize>,
        from: Vec3,
        to: Vec3,
        basis: portal::carry::Basis,
        half: f32,
    ) -> Trace {
        let expand = |solids: &[Solid]| {
            solids
                .iter()
                .map(|(id, planes)| (*id, expanded_planes(planes, basis, half)))
                .collect()
        };
        let expanded = Space {
            world: expand(&self.world),
            pair: self.pair,
            sims: self
                .sims
                .iter()
                .map(|s| Sim {
                    front: expand(&s.front),
                    tube: expand(&s.tube),
                    wall: expand(&s.wall),
                    linked: expand(&s.linked),
                    hole: expand(&s.hole),
                    surface: s.surface,
                })
                .collect(),
        };
        expanded.trace(owner, from, to, [0.0; 3], [0.0; 3])
    }

    pub fn add_object(&mut self, centre: Vec3, basis: portal::carry::Basis, half: f32) {
        let planes: Vec<Plane> = basis
            .into_iter()
            .flat_map(|axis| {
                [-1.0, 1.0].map(|sign| {
                    let normal = axis.map(|v| v * sign);
                    Plane {
                        normal,
                        distance: dot(normal, centre) + half,
                    }
                })
            })
            .collect();
        self.add_solid(planes);
    }

    pub fn add_solid(&mut self, planes: Vec<Plane>) {
        self.world.push((usize::MAX - 1, planes.clone()));
        if let Some(pair) = self.pair {
            for (i, sim) in self.sims.iter_mut().enumerate() {
                if let Some(solid) = solid(
                    usize::MAX - 1,
                    [planes.clone(), vec![bound(&pair[i], 0, -1.0, FRONT_CLIP)]].concat(),
                ) {
                    sim.front.push(solid);
                }
                let linked = carried(&[(usize::MAX - 1, planes.clone())], &pair[1 - i], &pair[i]);
                sim.linked.extend(linked);
            }
        }
    }

    /// An owned object only meets its portal's carved world, and the far side while in the hole.
    /// Translated from 0x90a9e0 and 0x2dbe90 (decompiled, server.so)
    pub fn object_solids(
        &self,
        owner: Option<usize>,
        in_hole: bool,
    ) -> Vec<(usize, usize, &[Plane])> {
        let Some((i, sim)) = owner.and_then(|i| self.sims.get(i).map(|s| (i, s))) else {
            return self
                .world
                .iter()
                .enumerate()
                .map(|(k, (n, planes))| (k, *n, planes.as_slice()))
                .collect();
        };
        let far = self.sims[1 - i].surface;
        let mut groups = vec![
            (&sim.front, sim.surface),
            (&sim.wall, sim.surface),
            (&sim.tube, sim.surface),
        ];
        if in_hole {
            groups.push((&sim.linked, far));
        }
        groups
            .into_iter()
            .enumerate()
            .flat_map(|(g, (solids, surface))| {
                solids.iter().enumerate().map(move |(k, (n, planes))| {
                    let surface = if *n == usize::MAX - 1 { *n } else { surface };
                    ((1 + i) << 24 | g << 20 | k, surface, planes.as_slice())
                })
            })
            .collect()
    }

    /// TracePlayerBBox: the real world, or the owning portal's world when it lets the hull further.
    /// Translated from 0x2ce890 and 0x2dc8e0 (decompiled, server.so)
    pub fn trace(
        &self,
        owner: Option<usize>,
        from: Vec3,
        to: Vec3,
        mins: Vec3,
        maxs: Vec3,
    ) -> Trace {
        let real = trace_brushes(from, to, mins, maxs, &self.world);
        let Some(sim) = owner.and_then(|i| self.sims.get(i)) else {
            return real;
        };
        let sim = sim.trace(from, to, mins, maxs);
        match !real.start_solid && real.fraction >= sim.fraction {
            true => real,
            // a hit keeps the real trace's surface
            false if sim.fraction < 1.0 => Trace {
                block: real.block.or(sim.block),
                ..sim
            },
            false => sim,
        }
    }
}

impl Sim {
    /// Translated from 0x2d8d00 (decompiled, server.so)
    fn trace(&self, from: Vec3, to: Vec3, mins: Vec3, maxs: Vec3) -> Trace {
        // a sub trace that starts solid counts as hitting at once, which is why the tube refuses them
        let cast = |solids: &[Solid]| match trace_brushes(from, to, mins, maxs, solids) {
            t if t.start_solid => Trace {
                fraction: 0.0,
                end: from,
                ..t
            },
            t => t,
        };
        let mut best: Option<Trace> = None;
        let mut take = |t: Trace, start_solid_ok: bool| {
            let closer = best.is_none_or(|b| t.fraction < b.fraction);
            if closer && (start_solid_ok || !t.start_solid) {
                best = Some(t);
            }
        };
        take(cast(&self.front), true);
        take(cast(&self.tube), false);
        take(cast(&self.wall), true);
        let hole = cast(&self.hole);
        if hole.fraction < 1.0 || hole.start_solid {
            take(cast(&self.linked), true);
        }
        best.unwrap_or_else(|| cast(&[]))
    }
}

/// Translated from 0x2d60f0 (decompiled, server.so)
fn behind(world: &[Solid], p: &Portal) -> usize {
    let from = p.world([1.0, 0.0, 0.0]);
    let to = p.world([-500.0, 0.0, 0.0]);
    trace_brushes(from, to, [0.0; 3], [0.0; 3], world)
        .block
        .unwrap_or(usize::MAX)
}

/// The world in front of the portal, cut off 0.1 in front of its plane.
fn front(chamber: &Chamber, p: &Portal) -> Vec<Solid> {
    let size = SIM_BOUNDS;
    let grab = bounds(
        p,
        [0.0, -size[1] / 2.0, -size[2] / 2.0],
        [size[0], size[1] / 2.0, size[2] / 2.0],
    );
    grabbed(chamber, grab)
        .filter_map(|(n, planes)| solid(n, [planes, vec![bound(p, 0, -1.0, FRONT_CLIP)]].concat()))
        .collect()
}

/// The wall behind the portal, cut 0.01 behind its plane, with the opening carved out of it.
fn wall(chamber: &Chamber, p: &Portal) -> Vec<Solid> {
    let grab = bounds(
        p,
        [-WALL_DEPTH, -WALL_REACH, -WALL_REACH],
        [0.0, WALL_REACH, WALL_REACH],
    );
    let prism = [
        bound(p, 1, 1.0, INNER.0),
        bound(p, 1, -1.0, -INNER.0),
        bound(p, 2, 1.0, INNER.1),
        bound(p, 2, -1.0, -INNER.1),
    ];
    let pieces = [
        local_box(p, [-FAR, -FAR, OUTER.1], [FAR, FAR, FAR]),
        local_box(p, [-FAR, -FAR, -FAR], [FAR, FAR, -OUTER.1]),
        local_box(p, [-FAR, -FAR, -OUTER.1], [FAR, -OUTER.0, OUTER.1]),
        local_box(p, [-FAR, OUTER.0, -OUTER.1], [FAR, FAR, OUTER.1]),
    ];
    grabbed(chamber, grab)
        .filter_map(|(n, planes)| solid(n, [planes, vec![bound(p, 0, 1.0, -WALL_CLIP)]].concat()))
        .flat_map(
            |(n, planes)| match solid(n, [planes.clone(), prism.to_vec()].concat()) {
                None => vec![(n, planes)],
                Some(_) => pieces
                    .iter()
                    .filter_map(|piece| solid(n, [planes.clone(), piece.clone()].concat()))
                    .collect(),
            },
        )
        .collect()
}

/// Four thin slabs lining the opening, 32.1 by 54.1 inside and 32.2 by 54.2 outside.
fn tube(p: &Portal) -> Vec<Solid> {
    let (x0, x1) = (-TUBE_BACK, -WALL_CLIP);
    [
        ([x0, -OUTER.0, INNER.1], [x1, OUTER.0, OUTER.1]),
        ([x0, -OUTER.0, -OUTER.1], [x1, OUTER.0, -INNER.1]),
        ([x0, -OUTER.0, -INNER.1], [x1, -INNER.0, INNER.1]),
        ([x0, INNER.0, -INNER.1], [x1, OUTER.0, INNER.1]),
    ]
    .into_iter()
    .map(|(lo, hi)| (usize::MAX, local_box(p, lo, hi)))
    .collect()
}

/// The other portal's front world, moved behind this one.
fn carried(solids: &[Solid], from: &Portal, to: &Portal) -> Vec<Solid> {
    solids
        .iter()
        .map(|(n, planes)| {
            let moved = planes
                .iter()
                .map(|pl| {
                    let normal = through_dir(from, to, pl.normal);
                    let point = through_point(from, to, pl.normal.map(|c| c * pl.distance));
                    Plane {
                        normal,
                        distance: dot(normal, point),
                    }
                })
                .collect();
            (*n, moved)
        })
        .collect()
}

fn grabbed(
    chamber: &Chamber,
    grab: (Vec3, Vec3),
) -> impl Iterator<Item = (usize, Vec<Plane>)> + '_ {
    chamber.blocks.iter().enumerate().filter_map(move |(n, b)| {
        let meets = (0..3).all(|i| b.min[i] <= grab.1[i] && b.max[i] >= grab.0[i]);
        meets.then(|| (n, b.planes()))
    })
}

/// The world box around a box given in the portal's forward, right and up.
fn bounds(p: &Portal, lo: Vec3, hi: Vec3) -> (Vec3, Vec3) {
    let corners = (0..8).map(|i| {
        let l = [0, 1, 2].map(|a| if i >> a & 1 == 0 { lo[a] } else { hi[a] });
        p.world(l)
    });
    corners.fold(([f32::MAX; 3], [f32::MIN; 3]), |(mn, mx), c| {
        (
            [0, 1, 2].map(|i| mn[i].min(c[i])),
            [0, 1, 2].map(|i| mx[i].max(c[i])),
        )
    })
}

fn axis(p: &Portal, a: usize) -> Vec3 {
    [p.forward, p.right, p.up][a]
}

/// sign * local[a] <= sign * at
fn bound(p: &Portal, a: usize, sign: f32, at: f32) -> Plane {
    let n = axis(p, a);
    Plane {
        normal: n.map(|c| c * sign),
        distance: sign * (dot(n, p.origin) + at),
    }
}

fn local_box(p: &Portal, lo: Vec3, hi: Vec3) -> Vec<Plane> {
    (0..3)
        .flat_map(|a| [bound(p, a, 1.0, hi[a]), bound(p, a, -1.0, lo[a])])
        .collect()
}

/// Keeps a clipped brush only when something is left of it.
fn solid(n: usize, planes: Vec<Plane>) -> Option<Solid> {
    let corners = vertices(&planes);
    let inside = |c: [f64; 3]| {
        planes.iter().all(|pl| {
            let d: f64 = (0..3).map(|i| pl.normal[i] as f64 * c[i]).sum();
            d <= pl.distance as f64 - 1e-4
        })
    };
    let count = corners.len() as f64;
    let middle = [0, 1, 2].map(|i| corners.iter().map(|c| c[i]).sum::<f64>() / count);
    (corners.len() >= 4 && inside(middle)).then_some((n, planes))
}

fn vertices(planes: &[Plane]) -> Vec<[f64; 3]> {
    let n = |p: &Plane| p.normal.map(|c| c as f64);
    let mut out = Vec::new();
    for (i, a) in planes.iter().enumerate() {
        for (j, b) in planes.iter().enumerate().skip(i + 1) {
            for c in &planes[j + 1..] {
                let (na, nb, nc) = (n(a), n(b), n(c));
                let bc = cross(nb, nc);
                let det = dot64(na, bc);
                if det.abs() < 1e-9 {
                    continue;
                }
                let v = [0, 1, 2].map(|k| {
                    (a.distance as f64 * bc[k]
                        + b.distance as f64 * cross(nc, na)[k]
                        + c.distance as f64 * cross(na, nb)[k])
                        / det
                });
                if planes
                    .iter()
                    .all(|pl| dot64(n(pl), v) <= pl.distance as f64 + 1e-4)
                {
                    out.push(v);
                }
            }
        }
    }
    out
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot64(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The solid's own planes plus every supporting plane a world box can touch it along, so a
/// box sweep against it is exact.
pub fn bevelled(planes: &[Plane]) -> Vec<Plane> {
    let axial = |p: &Plane| p.normal.iter().any(|c| c.abs() > 0.99999);
    if planes.iter().all(axial) {
        return planes.to_vec();
    }
    let points = vertices(planes);
    let tight: Vec<Plane> = planes
        .iter()
        .map(|p| Plane {
            normal: p.normal,
            distance: points
                .iter()
                .map(|v| dot64(p.normal.map(f64::from), *v) as f32)
                .fold(f32::NEG_INFINITY, f32::max)
                .min(p.distance),
        })
        .collect();
    expanded_planes(
        &tight,
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        0.0,
    )
}

fn bevel_all(solids: Vec<Solid>) -> Vec<Solid> {
    solids
        .into_iter()
        .map(|(n, planes)| (n, bevelled(&planes)))
        .collect()
}

fn expanded_planes(planes: &[Plane], basis: portal::carry::Basis, half: f32) -> Vec<Plane> {
    let support = |normal: Vec3| {
        half * basis
            .iter()
            .map(|axis| dot(*axis, normal).abs())
            .sum::<f32>()
    };
    let mut result: Vec<Plane> = planes
        .iter()
        .map(|p| Plane {
            normal: p.normal,
            distance: p.distance + support(p.normal),
        })
        .collect();
    let points = vertices(planes);
    if points.is_empty() {
        return result;
    }
    let mut add_axis = |axis: Vec3| {
        let length = dot(axis, axis).sqrt();
        if length < 1e-6 {
            return;
        }
        let normal = axis.map(|v| v / length);
        for sign in [-1.0, 1.0] {
            let normal = normal.map(|v| v * sign);
            if result.iter().any(|p| dot(p.normal, normal) > 0.99999) {
                continue;
            }
            let distance = points
                .iter()
                .map(|p| dot64(normal.map(f64::from), *p))
                .fold(f64::NEG_INFINITY, f64::max) as f32
                + support(normal);
            result.push(Plane { normal, distance });
        }
    };
    for axis in basis {
        add_axis(axis);
    }
    for (i, a) in planes.iter().enumerate() {
        for b in &planes[i + 1..] {
            let edge = portal::carry::cross(a.normal, b.normal);
            for axis in basis {
                add_axis(portal::carry::cross(edge, axis));
            }
        }
    }
    result
}
