pub type Vec3 = [f32; 3];

/// How far past the exit plane a carried point or ray starts again.
const EXIT_EPSILON: f32 = 0.01;

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: Vec3, s: f32) -> Vec3 {
    a.map(|v| v * s)
}

/// One portal: forward leaves the surface, right and up lie on it, half is the
/// opening's half width and half height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Portal {
    pub origin: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub half: (f32, f32),
}

impl Portal {
    pub fn new(origin: Vec3, forward: Vec3, up: Vec3, half: (f32, f32)) -> Self {
        Portal {
            origin,
            forward,
            right: cross(forward, up),
            up,
            half,
        }
    }

    /// (forward, right, up) coordinates of a world point
    pub fn local(&self, p: Vec3) -> Vec3 {
        let d = sub(p, self.origin);
        [dot(d, self.forward), dot(d, self.right), dot(d, self.up)]
    }

    pub fn world(&self, l: Vec3) -> Vec3 {
        add(self.origin, self.vector(l))
    }

    fn vector(&self, l: Vec3) -> Vec3 {
        add(
            add(scale(self.forward, l[0]), scale(self.right, l[1])),
            scale(self.up, l[2]),
        )
    }

    fn local_vector(&self, v: Vec3) -> Vec3 {
        [dot(v, self.forward), dot(v, self.right), dot(v, self.up)]
    }

    /// whether a point on the plane lies in the opening, the rectangle rays and
    /// movers pass through (the round look is only the drawn mask)
    pub fn inside(&self, l: Vec3) -> bool {
        l[1].abs() <= self.half.0 && l[2].abs() <= self.half.1
    }
}

/// Turned half way round the up axis, so in through one front is out through the other.
fn flip(l: Vec3) -> Vec3 {
    [-l[0], -l[1], l[2]]
}

pub fn through_point(from: &Portal, to: &Portal, p: Vec3) -> Vec3 {
    to.world(flip(from.local(p)))
}

pub fn through_dir(from: &Portal, to: &Portal, d: Vec3) -> Vec3 {
    to.vector(flip(from.local_vector(d)))
}

/// Where along a to b a mover passes from the front of the portal to its back
/// through the opening, as a fraction of the move.
pub fn crossing(portal: &Portal, a: Vec3, b: Vec3) -> Option<f32> {
    let (la, lb) = (portal.local(a), portal.local(b));
    if la[0] < 0.0 || lb[0] >= 0.0 {
        return None;
    }
    let t = la[0] / (la[0] - lb[0]);
    let at = add(la, scale(sub(lb, la), t));
    portal.inside(at).then_some(t)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Leg {
    pub origin: Vec3,
    pub dir: Vec3,
    pub length: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hit {
    World(Vec3),
    Nothing,
    TooManyPortals,
}

pub struct Path {
    pub legs: Vec<Leg>,
    pub end: Hit,
}

/// A ray that carries on out of the linked portal each time it enters one, at most passes times.
/// world casts a straight ray and gives the distance to what it meets.
pub fn trace(
    pair: &[Portal; 2],
    mut origin: Vec3,
    mut dir: Vec3,
    max: f32,
    passes: usize,
    mut world: impl FnMut(Vec3, Vec3, f32) -> Option<f32>,
) -> Path {
    let mut legs = Vec::new();
    let mut left = max;
    loop {
        let hit = world(origin, dir, left);
        let reach = hit.unwrap_or(left);
        let entered = (0..2)
            .filter_map(|i| {
                crossing(&pair[i], origin, add(origin, scale(dir, reach))).map(|t| (i, t * reach))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((i, along)) = entered else {
            legs.push(Leg {
                origin,
                dir,
                length: reach,
            });
            let end = hit.map_or(Hit::Nothing, |d| Hit::World(add(origin, scale(dir, d))));
            return Path { legs, end };
        };
        legs.push(Leg {
            origin,
            dir,
            length: along,
        });
        if legs.len() > passes {
            return Path {
                legs,
                end: Hit::TooManyPortals,
            };
        }
        let (from, to) = (&pair[i], &pair[1 - i]);
        let at = add(origin, scale(dir, along));
        origin = add(through_point(from, to, at), scale(to.forward, EXIT_EPSILON));
        dir = through_dir(from, to, dir);
        left -= along;
    }
}
