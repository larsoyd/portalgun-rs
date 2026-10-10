use crate::pair::{crossing, through_dir, Portal, Vec3};

pub const MIN_FLOOR_EXIT_PLAYER: f32 = 300.0;
const MIN_FLOOR_TO_FLOOR_OBJECT: f32 = 225.0;
const MIN_FLOOR_EXIT_OBJECT: f32 = 50.0;
pub const MAX_EXIT_SPEED: f32 = 1000.0;
/// where a portal starts counting as a floor
const FLOORISH: f32 = std::f32::consts::FRAC_1_SQRT_2;
const HOLE_NEAR: f32 = 0.5;
const HOLE_FAR: f32 = 500.0;
const HOLE_SHRINK: f32 = 0.98;
const TRIGGER_DEPTH: f32 = 64.0;
const ROLL_RATE: f32 = 120.0;
const ROLL_SETTLE: f32 = 60.0;
const ROLL_ACCEL: f32 = 400.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mover {
    Player,
    Object,
}

/// Velocity carried through. The original's floor exit pushes and speed cap come along.
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
/// behind the opening, or its move crossed the opening in one step.
pub fn entered(p: &Portal, before: Vec3, centre: Vec3, half: Vec3) -> bool {
    let l = p.local(centre);
    let reach =
        |axis: Vec3| half[0] * axis[0].abs() + half[1] * axis[1].abs() + half[2] * axis[2].abs();
    let meets = |c: f32, r: f32, lo: f32, hi: f32| c + r >= lo && c - r <= hi;
    let in_hole = l[0] < 0.0
        && meets(l[0], reach(p.forward), -HOLE_FAR, -HOLE_NEAR)
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
        );
    in_hole || crossing(p, before, centre).is_some()
}

/// Whether a mover is in the portal's trigger box, 64 Source units deep in front of it, with
/// its centre in front: what makes the portal take it over.
pub fn touching(p: &Portal, centre: Vec3, half: Vec3) -> bool {
    let l = p.local(centre);
    l[0] >= 0.0
        && overlaps(l[0], reach(half, p.forward), 0.0, TRIGGER_DEPTH)
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

/// One step of the view roll easing back to level after a portal, in degrees. The rate
/// climbs while the view is tipped over and settles otherwise.
pub fn recover_roll(roll: f32, rate: f32, on_ground: bool, up_z: f32, dt: f32) -> (f32, f32) {
    if roll == 0.0 {
        return (0.0, 0.0);
    }
    let cap = if on_ground {
        2.0 * ROLL_RATE
    } else {
        ROLL_RATE
    };
    let rate = if up_z < 0.75 {
        (rate + ROLL_ACCEL * dt).min(cap)
    } else if rate > ROLL_SETTLE {
        (rate - ROLL_ACCEL * dt).max(ROLL_SETTLE)
    } else {
        (rate + ROLL_ACCEL * dt).min(ROLL_SETTLE)
    };
    let step = rate * dt;
    let roll = if roll.abs() <= step {
        0.0
    } else {
        roll - step * roll.signum()
    };
    (roll, if roll == 0.0 { 0.0 } else { rate })
}
