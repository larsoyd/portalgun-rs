use crate::pair::{Portal, Vec3};

/// r_portal_stencil_depth's default
pub const STENCIL_DEPTH: i32 = 2;
/// how far behind the exit the linked view's clip plane sits
const CLIP_BEHIND: f32 = 0.5;
const MIN_NEAR: f32 = 1.0;
/// 54 squared, close enough that the top level view shows from either side
const CLOSE_SQ: f32 = 2916.0;
const CULL_EPSILON: f32 = 0.01;
#[allow(clippy::approx_constant)]
const DOUBLER_FACING: f32 = -0.7071;

/// Points with dot(normal, p) > dist are kept.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    pub normal: Vec3,
    pub dist: f32,
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Translated from 0x55b880 (decompiled, client.so)
pub fn clip_plane(exit: &Portal) -> Plane {
    Plane {
        normal: exit.forward,
        dist: dot(exit.forward, exit.origin) - CLIP_BEHIND,
    }
}

/// Translated from 0x55bb8d (disassembled, client.so)
pub fn near_plane(parent: f32) -> f32 {
    if MIN_NEAR > parent {
        MIN_NEAR
    } else {
        parent
    }
}

/// Translated from 0x554e00 (decompiled, client.so)
pub fn max_depth(depth: i32, stencil_bits: u32) -> i32 {
    let values = 1 << stencil_bits;
    let cap = if values > 11 { 10 } else { values - 1 };
    depth.min(cap)
}

/// The depth left under faces drawn at a level, as 0x554e00 (client.so) counts it.
pub fn remaining(max: i32, level: i32) -> i32 {
    (max - level).max(0)
}

/// Translated from 0x552780 (disassembled, client.so). The secondary static only ever falls to
/// zero outside demo playback so the max with it is left out.
pub fn static_amount(statics: f32, linked: bool, remaining: i32, doubler: bool) -> f32 {
    if doubler {
        0.0
    } else if remaining == 0 || !linked {
        1.0
    } else {
        statics
    }
}

/// Translated from 0x559c30 (decompiled, client.so)
pub fn doubler_usable(this: &Portal, linked: &Portal) -> bool {
    let to = [0, 1, 2].map(|i| linked.origin[i] - this.origin[i]);
    dot(to, this.forward) > 0.0 && dot(this.forward, linked.forward) < DOUBLER_FACING
}

/// Translated from 0x55a5f0 (decompiled, client.so)
pub fn draws_doubler(usable: bool, remaining: i32, level: i32) -> bool {
    usable && remaining == 0 && level > 1
}

/// Translated from 0x63a8f0 (decompiled, client.so)
pub fn clip_poly(poly: &[Vec3], plane: &Plane, epsilon: f32) -> Vec<Vec3> {
    #[derive(PartialEq, Clone, Copy)]
    enum Side {
        Front,
        Back,
        On,
    }
    let dists: Vec<f32> = poly
        .iter()
        .map(|p| dot(*p, plane.normal) - plane.dist)
        .collect();
    let sides: Vec<Side> = dists
        .iter()
        .map(|&d| match d {
            d if epsilon < d => Side::Front,
            d if -epsilon <= d => Side::On,
            _ => Side::Back,
        })
        .collect();
    if !sides.contains(&Side::Front) {
        return Vec::new();
    }
    if !sides.contains(&Side::Back) {
        return poly.to_vec();
    }
    let mut out = Vec::new();
    for i in 0..poly.len() {
        let j = (i + 1) % poly.len();
        let p = poly[i];
        if sides[i] != Side::Back {
            out.push(p);
        }
        if sides[i] == Side::On || sides[j] == Side::On || sides[i] == sides[j] {
            continue;
        }
        let t = dists[i] / (dists[i] - dists[j]);
        out.push([0, 1, 2].map(|k| {
            if plane.normal[k] == 1.0 {
                plane.dist
            } else if plane.normal[k] == -1.0 {
                -plane.dist
            } else {
                p[k] + (poly[j][k] - p[k]) * t
            }
        }));
    }
    out
}

/// The quad the view cull clips, one unit out from the surface.
fn cull_quad(p: &Portal) -> [Vec3; 4] {
    let (w, h) = p.half;
    [(w, h), (-w, h), (-w, -h), (w, -h)].map(|(r, u)| p.world([1.0, r, u]))
}

/// Translated from 0x557200 (decompiled, client.so)
pub fn shows_view(
    p: &Portal,
    statics: f32,
    linked: bool,
    eye: Vec3,
    level: i32,
    frustum: &[Plane],
) -> bool {
    if !linked || statics == 1.0 {
        return false;
    }
    let to = [0, 1, 2].map(|i| p.origin[i] - eye[i]);
    if level == 0 && CLOSE_SQ > dot(to, to) {
        return true;
    }
    if dot(p.forward, p.origin) >= dot(p.forward, eye) {
        return false;
    }
    let mut poly = cull_quad(p).to_vec();
    for plane in frustum {
        if poly.len() <= 2 {
            break;
        }
        poly = clip_poly(&poly, plane, CULL_EPSILON);
    }
    poly.len() > 2
}
