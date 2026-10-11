// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::pair::{Portal, Vec3};

pub const HALF_WIDTH: f32 = 32.0;
pub const HALF_HEIGHT: f32 = 54.0;
/// sqrt((64^2 + 108^2) / 2) Source units, the farthest a shot may be bumped
pub const BUMP_LIMIT: f32 = 88.769_37;
const PROBE: f32 = 1.0;
const SNAP_REACH: f32 = 1.5;
const EDGE_SLACK: f32 = 0.5;
/// the search for a fitting spot goes out in coarse rings, then settles on a fine grid
const COARSE: f32 = 8.0;
const FINE: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Surface {
    Portalable,
    /// glass, moving things and anything else a portal may not open on
    Rejects,
    /// the sky, where a shot just vanishes
    PassThrough,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    pub distance: f32,
    pub normal: Vec3,
    pub surface: Surface,
}

pub trait World {
    fn cast(&self, from: Vec3, dir: Vec3, max: f32) -> Option<RayHit>;

    fn clear_portal(&self, portal: &Portal) -> bool {
        clear_in_front(self, portal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Failure {
    Missed,
    PassThrough,
    BadSurface,
    CantFit,
    Overlap,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    Placed { portal: Portal, quality: f32 },
    Failed(Failure),
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn along(p: Vec3, d: Vec3, s: f32) -> Vec3 {
    [p[0] + d[0] * s, p[1] + d[1] * s, p[2] + d[2] * s]
}

fn normalize(v: Vec3) -> Vec3 {
    let l = dot(v, v).sqrt();
    v.map(|x| x / l)
}

/// Forward is the surface normal. Up is world up, except on a level floor or ceiling where it
/// follows the shot, so the portal's top faces away from the shooter.
pub fn orientation(normal: Vec3, shot: Vec3) -> (Vec3, Vec3) {
    let level = normal[0].abs() < 0.001 && normal[1].abs() < 0.001;
    let pseudo = if level { shot } else { [0.0, 0.0, 1.0] };
    (
        normal,
        normalize(along(pseudo, normal, -dot(pseudo, normal))),
    )
}

/// Where a portal shot opens, and where the shot landed.
pub fn place(
    world: &impl World,
    from: Vec3,
    dir: Vec3,
    range: f32,
    other: Option<&Portal>,
) -> (Placement, Vec3) {
    let Some(hit) = world.cast(from, dir, range) else {
        return (Placement::Failed(Failure::Missed), along(from, dir, range));
    };
    let at = along(from, dir, hit.distance);
    match hit.surface {
        Surface::PassThrough => return (Placement::Failed(Failure::PassThrough), at),
        Surface::Rejects => return (Placement::Failed(Failure::BadSurface), at),
        Surface::Portalable => {}
    }
    let (forward, up) = orientation(hit.normal, dir);
    let shape = Portal::new(at, forward, up, (HALF_WIDTH, HALF_HEIGHT));
    let fits = |p: &Portal| on_surface(world, p) && world.clear_portal(p);
    let candidate = |r, u| {
        let mut p = moved(&shape, r, u);
        snap_to_floor(world, &mut p);
        p
    };
    let ok = |(r, u): (f32, f32)| {
        let p = candidate(r, u);
        let d = [0, 1, 2].map(|i| p.origin[i] - at[i]);
        dot(d, d) <= BUMP_LIMIT * BUMP_LIMIT && fits(&p) && !other.is_some_and(|o| overlaps(&p, o))
    };
    let found = std::iter::once((0.0, 0.0))
        .chain(rings())
        .find(|&c| ok(c))
        .map(|coarse| {
            if coarse == (0.0, 0.0) {
                return coarse;
            }
            let steps = (COARSE / FINE) as i32;
            (-steps..=steps)
                .flat_map(|i| {
                    (-steps..=steps)
                        .map(move |k| (coarse.0 + i as f32 * FINE, coarse.1 + k as f32 * FINE))
                })
                .filter(|&(r, u)| r.hypot(u) <= BUMP_LIMIT && r.hypot(u) < coarse.0.hypot(coarse.1))
                .filter(|&c| ok(c))
                .min_by(|a, b| a.0.hypot(a.1).total_cmp(&b.0.hypot(b.1)))
                .unwrap_or(coarse)
        });
    let found = found.map(|(r, u)| candidate(r, u));
    let Some(portal) = found else {
        let crowded = fits(&shape) && other.is_some_and(|o| overlaps(&shape, o));
        return (
            Placement::Failed(if crowded {
                Failure::Overlap
            } else {
                Failure::CantFit
            }),
            at,
        );
    };
    let d = [0, 1, 2].map(|i| portal.origin[i] - at[i]);
    let m = 1.0 - dot(d, d) / (BUMP_LIMIT * BUMP_LIMIT);
    (
        Placement::Placed {
            portal,
            quality: m.powi(4) * 0.7 + 0.3,
        },
        at,
    )
}

/// offsets along (right, up) on rings of growing radius out to the bump limit
fn rings() -> impl Iterator<Item = (f32, f32)> {
    let rings = (BUMP_LIMIT / COARSE).floor() as usize;
    (1..=rings).flat_map(|k| {
        let r = k as f32 * COARSE;
        let n = ((std::f32::consts::TAU * r / COARSE).ceil() as usize).max(8);
        (0..n).map(move |i| {
            let a = std::f32::consts::TAU * i as f32 / n as f32;
            (r * a.cos(), r * a.sin())
        })
    })
}

fn moved(p: &Portal, r: f32, u: f32) -> Portal {
    Portal {
        origin: along(along(p.origin, p.right, r), p.up, u),
        ..*p
    }
}

/// points spread over the opening, from the centre out to just inside its edges
fn samples(p: &Portal) -> impl Iterator<Item = Vec3> + '_ {
    let (w, h) = (p.half.0 - EDGE_SLACK, p.half.1 - EDGE_SLACK);
    [
        (0.0, 0.0),
        (-w, -h),
        (w, -h),
        (-w, h),
        (w, h),
        (0.0, -h),
        (0.0, h),
        (-w, 0.0),
        (w, 0.0),
    ]
    .into_iter()
    .map(move |(r, u)| along(along(p.origin, p.right, r), p.up, u))
}

fn on_surface(world: &impl World, p: &Portal) -> bool {
    let back = p.forward.map(|v| -v);
    samples(p).all(|s| {
        world
            .cast(along(s, p.forward, PROBE), back, 2.0 * PROBE)
            .is_some_and(|h| {
                h.surface == Surface::Portalable
                    && dot(h.normal, p.forward) > 0.999
                    && (h.distance - PROBE).abs() < 0.01
            })
    })
}

/// nothing standing on the surface inside the opening, such as a wall meeting it in a corner
fn clear_in_front(world: &(impl World + ?Sized), p: &Portal) -> bool {
    let centre = along(p.origin, p.forward, PROBE);
    samples(p).skip(1).all(|s| {
        let to = along(s, p.forward, PROBE);
        let d = [to[0] - centre[0], to[1] - centre[1], to[2] - centre[2]];
        let len = dot(d, d).sqrt();
        world.cast(centre, normalize(d), len).is_none()
    })
}

/// two openings on the same face whose rectangles overlap
fn overlaps(a: &Portal, b: &Portal) -> bool {
    let d = [0, 1, 2].map(|i| b.origin[i] - a.origin[i]);
    if dot(a.forward, b.forward) < 0.95 || dot(d, a.forward).abs() > 2.0 * PROBE {
        return false;
    }
    let reach = |p: &Portal, axis: Vec3| {
        p.half.0 * dot(p.right, axis).abs() + p.half.1 * dot(p.up, axis).abs()
    };
    [a.right, a.up, b.right, b.up]
        .iter()
        .all(|&axis| dot(d, axis).abs() < reach(a, axis) + reach(b, axis) - 0.01)
}

/// sits the bottom edge flush on a floor just below it, as Source does for upright portals
pub fn snap_to_floor(world: &impl World, p: &mut Portal) {
    if p.up[2] <= 0.7 {
        return;
    }
    let down = p.up.map(|v| -v);
    let from = along(p.origin, p.forward, 0.05);
    if let Some(h) = world.cast(from, down, p.half.1 + SNAP_REACH) {
        p.origin = along(p.origin, p.up, p.half.1 - h.distance);
    }
}
