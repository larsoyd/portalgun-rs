// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::pair::{through_dir, Portal, Vec3};
use crate::viewbob::{angle_vectors, rotate_about, vector_angles, Angles};

pub const MIN_FLOOR_EXIT_PLAYER: f32 = 300.0;
const MIN_FLOOR_TO_FLOOR_OBJECT: f32 = 225.0;
const MIN_FLOOR_EXIT_OBJECT: f32 = 50.0;
pub const MAX_EXIT_SPEED: f32 = 1000.0;
/// where a portal starts counting as a floor, the float 0.7071 rather than one over root two
#[allow(clippy::approx_constant)]
const FLOORISH: f32 = 0.7071;
/// EntityIsInPortalHole's box, set up at 0x2ccab8 in server.so
const HOLE_NEAR: f32 = 0.5;
const HOLE_FAR: f32 = 500.0;
const HOLE_SHRINK: f32 = 0.98;
/// prop_portal's collision box reaches 64 out of the wall
const TRIGGER_DEPTH: f32 = 64.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mover {
    Player,
    Object,
}

/// Velocity carried through, with the floor exit pushes and the speed cap.
/// Translated from 0x912ff0 (decompiled, server.so)
pub fn exit_velocity(from: &Portal, to: &Portal, v: Vec3, who: Mover) -> Vec3 {
    let mut out = through_dir(from, to, v);
    if to.forward[2] > FLOORISH {
        let min = match who {
            Mover::Player => MIN_FLOOR_EXIT_PLAYER,
            Mover::Object if from.forward[2] > FLOORISH => MIN_FLOOR_TO_FLOOR_OBJECT,
            Mover::Object => MIN_FLOOR_EXIT_OBJECT,
        };
        out[2] = out[2].max(min);
    }
    let speed = (out[0] * out[0] + out[1] * out[1] + out[2] * out[2]).sqrt();
    if speed > MAX_EXIT_SPEED {
        out = out.map(|c| c * MAX_EXIT_SPEED / speed);
    }
    out
}

/// Whether a mover's centre is behind the plane with its box reaching into the hole
/// behind the opening. There is no swept test.
/// Translated from 0x9172b0 (decompiled, server.so)
pub fn entered(p: &Portal, centre: Vec3, half: Vec3) -> bool {
    p.local(centre)[0] < 0.0 && in_hole(p, centre, half)
}

/// A portal takes over a mover only while its centre is strictly in front.
/// Translated from 0x91da70 and 0x91d3b0 (decompiled, server.so)
pub fn in_front(p: &Portal, centre: Vec3) -> bool {
    p.local(centre)[0] > 0.0
}

/// Whether a box reaches into the hole shape behind the opening, EntityIsInPortalHole.
pub fn in_hole(p: &Portal, centre: Vec3, half: Vec3) -> bool {
    let l = p.local(centre);
    let reach =
        |axis: Vec3| half[0] * axis[0].abs() + half[1] * axis[1].abs() + half[2] * axis[2].abs();
    let meets = |c: f32, r: f32, lo: f32, hi: f32| c + r >= lo && c - r <= hi;
    meets(l[0], reach(p.forward), -HOLE_FAR, -HOLE_NEAR)
        && meets(
            l[1],
            reach(p.right),
            -p.half.0 * HOLE_SHRINK,
            p.half.0 * HOLE_SHRINK,
        )
        && meets(
            l[2],
            reach(p.up),
            -p.half.1 * HOLE_SHRINK,
            p.half.1 * HOLE_SHRINK,
        )
}

/// m_bPitchReorientation, from the view forward before a teleport. Holding an object clears it.
/// Translated from 0x913d68 (decompiled, server.so)
pub fn pitch_reorients(f: Vec3, from: &Portal, exit_z: f32) -> bool {
    let len_sq = f[0] * f[0] + f[1] * f[1];
    let r = match len_sq > 0.0 {
        true => 1.0 / (len_sq + 1e-10).sqrt(),
        false => 1.0,
    };
    let flat = [f[0] * r, f[1] * r, 0.0];
    let along = |v: Vec3| v[0] * flat[0] + v[1] * flat[1] + v[2] * flat[2];
    let (fwd, up) = (from.forward, from.up);
    if up[2] > 0.99 {
        let d = along(fwd);
        if d > 0.5 || -0.5 > d || len_sq == 0.0 {
            return true;
        }
    }
    let steep = |z: f32| z > 0.99 || -0.99 > z;
    if steep(fwd[2]) && steep(exit_z) && (-0.5 > f[2] || f[2] > 0.5) {
        return true;
    }
    along(up) > 0.0 && 0.99 >= exit_z && exit_z > 0.75
}

/// Whether this portal may take a mover held by another, by the nearer origin.
/// Translated from 0x917680 (decompiled, server.so)
pub fn nearer(centre: Vec3, this: &Portal, holder: &Portal) -> bool {
    let d = |p: &Portal| {
        let v = [0, 1, 2].map(|i| centre[i] - p.origin[i]);
        v[0] * v[0] + v[1] * v[1] + v[2] * v[2]
    };
    d(this) < d(holder)
}

/// Whether a portal keeps a mover leaving its box: ducked, behind a ceiling portal's plane.
/// Translated from 0x91dc20 (decompiled, server.so)
pub fn keeps_leaving(p: &Portal, centre: Vec3, in_duck_jump: bool) -> bool {
    p.forward[2] < -FLOORISH && p.local(centre)[0] < 0.0 && in_duck_jump
}

/// Whether a mover's box meets the portal's own box, 64 deep in front of the opening.
pub fn touching(p: &Portal, centre: Vec3, half: Vec3) -> bool {
    let l = p.local(centre);
    overlaps(l[0], reach(half, p.forward), 0.0, TRIGGER_DEPTH)
        && overlaps(l[1], reach(half, p.right), -p.half.0, p.half.0)
        && overlaps(l[2], reach(half, p.up), -p.half.1, p.half.1)
}

/// Whether a box crosses the portal's plane within its opening (UTIL_IsBoxIntersectingPortal).
pub fn meets_opening(p: &Portal, centre: Vec3, half: Vec3) -> bool {
    let l = p.local(centre);
    l[0].abs() <= reach(half, p.forward)
        && overlaps(l[1], reach(half, p.right), -p.half.0, p.half.0)
        && overlaps(l[2], reach(half, p.up), -p.half.1, p.half.1)
}

fn reach(half: Vec3, axis: Vec3) -> f32 {
    half[0] * axis[0].abs() + half[1] * axis[1].abs() + half[2] * axis[2].abs()
}

fn overlaps(c: f32, r: f32, lo: f32, hi: f32) -> bool {
    c + r >= lo && c - r <= hi
}

/// A view's forward and up carried through.
pub fn view_through(from: &Portal, to: &Portal, forward: Vec3, up: Vec3) -> (Vec3, Vec3) {
    (through_dir(from, to, forward), through_dir(from, to, up))
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Reorient {
    pub rate: f32,
    pub upside_down: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct RollFix {
    pub grounded: bool,
    pub in_air: bool,
    pub pitch_reorientation: bool,
    pub coin: bool,
    pub frametime: f32,
}

/// The view's roll easing back to level after a portal, run every client frame.
/// Translated from 0x550ac0 (decompiled, client.so)
pub fn fix_roll(state: &mut Reorient, a: Angles, how: RollFix) -> Angles {
    let (forward, right, up) = angle_vectors(a);
    if a.roll == 0.0 {
        *state = Reorient {
            rate: 0.0,
            upside_down: up[2] < 0.0,
        };
        return a;
    }
    let cap = if how.grounded { 240.0 } else { 120.0 };
    let stuck = a.roll > 175.0 && forward[2] > 0.99;
    if stuck {
        state.rate = cap;
    } else if !how.in_air && !how.grounded {
        state.upside_down = up[2] < 0.0;
        return a;
    }
    let ramp = 400.0 * how.frametime;
    state.rate = if up[2] < 0.75 {
        (state.rate + ramp).min(cap)
    } else if state.rate > 60.0 {
        (state.rate - ramp).max(60.0)
    } else {
        (state.rate + ramp).min(60.0)
    };
    let step = state.rate * how.frametime;
    let fixed = if (stuck || how.pitch_reorientation) && up[2] < 0.2 {
        let turn = if forward[2] > 0.0 { -step } else { step };
        vector_angles(
            rotate_about(right, turn, forward),
            rotate_about(right, turn, up),
        )
    } else {
        let roll = if a.roll == 180.0 && how.coin && !stuck && !how.pitch_reorientation {
            -180.0
        } else {
            a.roll
        };
        Angles {
            roll: if roll > 0.0 {
                (roll - step).max(0.0)
            } else {
                (roll + step).min(0.0)
            },
            ..a
        }
    };
    state.upside_down = stuck && angle_vectors(fixed).2[2] < 0.0;
    fixed
}

/// Whether an axis-aligned box overlaps either triangle of the portal's quad and straddles its plane.
/// Translated from 0x28a870 and 0x2debb0 (decompiled, client.so)
pub fn box_meets_portal(p: &Portal, centre: Vec3, half: Vec3) -> bool {
    let corner = |s: f32, t: f32| {
        [0, 1, 2].map(|i| p.origin[i] + p.right[i] * s * p.half.0 + p.up[i] * t * p.half.1)
    };
    let [a, b, c, d] = [
        corner(-1.0, -1.0),
        corner(1.0, -1.0),
        corner(1.0, 1.0),
        corner(-1.0, 1.0),
    ];
    let straddles = {
        let distance = dot(p.forward, p.origin);
        let reach = reach(half, p.forward);
        let middle = dot(p.forward, centre);
        middle + reach >= distance && middle - reach < distance
    };
    straddles
        && (triangle_meets_box([a, b, c], centre, half)
            || triangle_meets_box([a, c, d], centre, half))
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn triangle_meets_box(triangle: [Vec3; 3], centre: Vec3, half: Vec3) -> bool {
    let v = triangle.map(|p| [0, 1, 2].map(|i| p[i] - centre[i]));
    let separated = |axis: Vec3| {
        let projected = v.map(|p| dot(p, axis));
        let radius = reach(half, axis);
        projected.iter().cloned().fold(f32::INFINITY, f32::min) > radius
            || projected.iter().cloned().fold(f32::NEG_INFINITY, f32::max) < -radius
    };
    let unit = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    if unit.iter().any(|&axis| separated(axis)) {
        return false;
    }
    let edges = [0, 1, 2].map(|i| [0, 1, 2].map(|k| v[(i + 1) % 3][k] - v[i][k]));
    !edges.iter().any(|e| {
        unit.iter().any(|u| {
            separated([
                u[1] * e[2] - u[2] * e[1],
                u[2] * e[0] - u[0] * e[2],
                u[0] * e[1] - u[1] * e[0],
            ])
        })
    })
}
