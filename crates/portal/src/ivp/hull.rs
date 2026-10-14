// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

//! Convex hulls and the closest feature pair between two of them.

pub type D3 = [f64; 3];

#[derive(Debug, Clone)]
pub struct Face {
    pub normal: D3,
    pub distance: f64,
    pub points: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct Convex {
    pub points: Vec<D3>,
    pub faces: Vec<Face>,
    pub edges: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    Point(usize),
    Edge(usize),
    Face(usize),
}

/// One side's feature against the other's, the cube always first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    pub cube: Feature,
    pub world: Feature,
}

/// A feature pair's geometry: distance along a normal that points from the world into the cube.
#[derive(Debug, Clone, Copy)]
pub struct Gap {
    pub distance: f64,
    pub normal: D3,
    pub on_cube: D3,
    pub inside: bool,
}

impl Convex {
    pub fn from_planes(planes: &[(D3, f64)]) -> Convex {
        let mut points: Vec<D3> = Vec::new();
        for (i, a) in planes.iter().enumerate() {
            for (j, b) in planes.iter().enumerate().skip(i + 1) {
                for c in &planes[j + 1..] {
                    let bc = cross(b.0, c.0);
                    let det = dot(a.0, bc);
                    if det.abs() < 1e-9 {
                        continue;
                    }
                    let p = add(
                        add(scale(bc, a.1), scale(cross(c.0, a.0), b.1)),
                        scale(cross(a.0, b.0), c.1),
                    )
                    .map(|v| v / det);
                    let inside = planes.iter().all(|pl| dot(pl.0, p) <= pl.1 + 1e-6);
                    if inside && !points.iter().any(|q| length(sub(*q, p)) < 1e-6) {
                        points.push(p);
                    }
                }
            }
        }
        let faces = planes
            .iter()
            .filter_map(|&(normal, distance)| {
                let on: Vec<usize> = (0..points.len())
                    .filter(|&i| (dot(normal, points[i]) - distance).abs() < 1e-6)
                    .collect();
                (on.len() >= 3).then(|| Face {
                    normal,
                    distance,
                    points: wind(&points, normal, on),
                })
            })
            .collect::<Vec<_>>();
        let mut edges = Vec::new();
        for f in &faces {
            for k in 0..f.points.len() {
                let (a, b) = (f.points[k], f.points[(k + 1) % f.points.len()]);
                let e = (a.min(b), a.max(b));
                if !edges.contains(&e) {
                    edges.push(e);
                }
            }
        }
        Convex {
            points,
            faces,
            edges,
        }
    }

    pub fn cube(half: f64) -> Convex {
        let planes = [0, 1, 2].into_iter().flat_map(|a| {
            [-1.0, 1.0].map(|s| {
                let mut n = [0.0; 3];
                n[a] = s;
                (n, half)
            })
        });
        Convex::from_planes(&planes.collect::<Vec<_>>())
    }

    pub fn moved(&self, map: impl Fn(D3) -> D3, turn: impl Fn(D3) -> D3) -> Convex {
        let points: Vec<D3> = self.points.iter().map(|p| map(*p)).collect();
        let faces = self
            .faces
            .iter()
            .map(|f| {
                let normal = turn(f.normal);
                Face {
                    normal,
                    distance: dot(normal, points[f.points[0]]),
                    points: f.points.clone(),
                }
            })
            .collect();
        Convex {
            points,
            faces,
            edges: self.edges.clone(),
        }
    }

    fn segment(&self, e: usize) -> (D3, D3) {
        let (a, b) = self.edges[e];
        (self.points[a], self.points[b])
    }

    fn spread(&self, axis: D3) -> (f64, f64) {
        self.points
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), p| {
                let d = dot(*p, axis);
                (lo.min(d), hi.max(d))
            })
    }
}

fn wind(points: &[D3], normal: D3, mut on: Vec<usize>) -> Vec<usize> {
    let centre = on
        .iter()
        .fold([0.0; 3], |c, &i| add(c, points[i]))
        .map(|v| v / on.len() as f64);
    let u = normalise(sub(points[on[0]], centre));
    let w = cross(normal, u);
    on.sort_by(|&a, &b| {
        let angle = |i: usize| {
            let d = sub(points[i], centre);
            dot(d, w).atan2(dot(d, u))
        };
        angle(a).total_cmp(&angle(b))
    });
    on
}

fn inside_face(hull: &Convex, face: &Face, p: D3) -> bool {
    let n = face.points.len();
    (0..n).all(|k| {
        let a = hull.points[face.points[k]];
        let b = hull.points[face.points[(k + 1) % n]];
        dot(cross(sub(b, a), face.normal), sub(p, a)) <= 1e-9
    })
}

fn closest_on_segments(a: (D3, D3), b: (D3, D3)) -> Option<(f64, f64)> {
    let (d1, d2) = (sub(a.1, a.0), sub(b.1, b.0));
    let r = sub(a.0, b.0);
    let (aa, ee, f) = (dot(d1, d1), dot(d2, d2), dot(d2, r));
    let (c, bb) = (dot(d1, r), dot(d1, d2));
    let denom = aa * ee - bb * bb;
    if denom < 1e-12 * aa * ee {
        return None;
    }
    let s = (bb * f - c * ee) / denom;
    let t = (bb * s + f) / ee;
    Some((s, t))
}

fn along(a: (D3, D3), s: f64) -> D3 {
    add(a.0, scale(sub(a.1, a.0), s))
}

/// Translated from 0xf7de0, 0xf8190 and 0xf8540 (decompiled, vphysics.so)
pub fn gap(cube: &Convex, world: &Convex, key: Key) -> Option<Gap> {
    let centre = |h: &Convex| {
        h.points
            .iter()
            .fold([0.0; 3], |c, p| add(c, *p))
            .map(|v| v / h.points.len() as f64)
    };
    match (key.cube, key.world) {
        (Feature::Point(p), Feature::Face(f)) => {
            let face = world.faces.get(f)?;
            let point = *cube.points.get(p)?;
            Some(Gap {
                distance: dot(face.normal, point) - face.distance,
                normal: face.normal,
                on_cube: point,
                inside: inside_face(world, face, point),
            })
        }
        (Feature::Face(f), Feature::Point(p)) => {
            let face = cube.faces.get(f)?;
            let point = *world.points.get(p)?;
            let normal = face.normal.map(|v| -v);
            let distance = dot(face.normal, point) - face.distance;
            Some(Gap {
                distance,
                normal,
                on_cube: add(point, scale(face.normal, -distance)),
                inside: inside_face(cube, face, point),
            })
        }
        (Feature::Edge(a), Feature::Edge(b)) => {
            let (ea, eb) = (cube.segment(a), world.segment(b));
            let (s, t) = closest_on_segments(ea, eb)?;
            let (pa, pb) = (along(ea, s), along(eb, t));
            let mut normal = normalise(cross(sub(ea.1, ea.0), sub(eb.1, eb.0)));
            if dot(normal, sub(centre(cube), centre(world))) < 0.0 {
                normal = normal.map(|v| -v);
            }
            Some(Gap {
                distance: dot(sub(pa, pb), normal),
                normal,
                on_cube: pa,
                inside: (0.0..=1.0).contains(&s) && (0.0..=1.0).contains(&t),
            })
        }
        (Feature::Point(p), Feature::Edge(e)) => {
            let point = *cube.points.get(p)?;
            let seg = world.segment(e);
            let d = sub(seg.1, seg.0);
            let s = dot(sub(point, seg.0), d) / dot(d, d);
            let off = sub(point, along(seg, s));
            Some(Gap {
                distance: length(off),
                normal: normalise(off),
                on_cube: point,
                inside: (0.0..=1.0).contains(&s),
            })
        }
        (Feature::Edge(e), Feature::Point(p)) => {
            let point = *world.points.get(p)?;
            let seg = cube.segment(e);
            let d = sub(seg.1, seg.0);
            let s = dot(sub(point, seg.0), d) / dot(d, d);
            let on_cube = along(seg, s);
            let off = sub(on_cube, point);
            Some(Gap {
                distance: length(off),
                normal: normalise(off),
                on_cube,
                inside: (0.0..=1.0).contains(&s),
            })
        }
        (Feature::Point(a), Feature::Point(b)) => {
            let (pa, pb) = (*cube.points.get(a)?, *world.points.get(b)?);
            let off = sub(pa, pb);
            Some(Gap {
                distance: length(off),
                normal: normalise(off),
                on_cube: pa,
                inside: true,
            })
        }
        _ => None,
    }
}

/// The nearest feature pair, or the shallowest one when the hulls overlap.
pub fn closest(cube: &Convex, world: &Convex) -> Option<(Key, Gap)> {
    let mut keys = Vec::new();
    for p in 0..cube.points.len() {
        keys.extend((0..world.faces.len()).map(|f| (Feature::Point(p), Feature::Face(f))));
        keys.extend((0..world.edges.len()).map(|e| (Feature::Point(p), Feature::Edge(e))));
        keys.extend((0..world.points.len()).map(|q| (Feature::Point(p), Feature::Point(q))));
    }
    for q in 0..world.points.len() {
        keys.extend((0..cube.faces.len()).map(|f| (Feature::Face(f), Feature::Point(q))));
        keys.extend((0..cube.edges.len()).map(|e| (Feature::Edge(e), Feature::Point(q))));
    }
    for a in 0..cube.edges.len() {
        keys.extend((0..world.edges.len()).map(|b| (Feature::Edge(a), Feature::Edge(b))));
    }
    let gaps = keys.into_iter().filter_map(|(c, w)| {
        let key = Key { cube: c, world: w };
        gap(cube, world, key).map(|g| (key, g))
    });
    match overlap(cube, world) {
        None => gaps
            .filter(|(_, g)| g.inside && g.distance >= 0.0)
            .min_by(|a, b| a.1.distance.total_cmp(&b.1.distance)),
        Some(key) => gap(cube, world, key).map(|g| (key, g)),
    }
}

/// The feature pair along the axis of least overlap, or None when some axis separates the hulls.
fn overlap(cube: &Convex, world: &Convex) -> Option<Key> {
    let depth = |axis: D3| {
        let (alo, ahi) = cube.spread(axis);
        let (blo, bhi) = world.spread(axis);
        (ahi - blo).min(bhi - alo)
    };
    let deepest = |h: &Convex, n: D3| {
        (0..h.points.len())
            .min_by(|&i, &j| dot(h.points[i], n).total_cmp(&dot(h.points[j], n)))
            .unwrap_or(0)
    };
    let mut best: Option<(f64, Key)> = None;
    let mut consider = |d: f64, key: Key| {
        if best.is_none_or(|(b, _)| d < b) {
            best = Some((d, key));
        }
    };
    for (f, face) in world.faces.iter().enumerate() {
        let p = deepest(cube, face.normal);
        consider(
            depth(face.normal),
            Key {
                cube: Feature::Point(p),
                world: Feature::Face(f),
            },
        );
    }
    for (f, face) in cube.faces.iter().enumerate() {
        let p = deepest(world, face.normal);
        consider(
            depth(face.normal),
            Key {
                cube: Feature::Face(f),
                world: Feature::Point(p),
            },
        );
    }
    for a in 0..cube.edges.len() {
        for b in 0..world.edges.len() {
            let (ea, eb) = (cube.segment(a), world.segment(b));
            let axis = cross(sub(ea.1, ea.0), sub(eb.1, eb.0));
            if length(axis) < 1e-9 {
                continue;
            }
            let key = Key {
                cube: Feature::Edge(a),
                world: Feature::Edge(b),
            };
            if gap(cube, world, key).is_some_and(|g| g.inside) {
                consider(depth(normalise(axis)), key);
            }
        }
    }
    best.filter(|(d, _)| *d >= 0.0).map(|(_, k)| k)
}

pub fn dot(a: D3, b: D3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn add(a: D3, b: D3) -> D3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn sub(a: D3, b: D3) -> D3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn scale(a: D3, k: f64) -> D3 {
    a.map(|v| v * k)
}
pub fn cross(a: D3, b: D3) -> D3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub fn length(a: D3) -> f64 {
    dot(a, a).sqrt()
}
pub fn normalise(a: D3) -> D3 {
    let l = length(a);
    if l > 0.0 {
        a.map(|v| v / l)
    } else {
        [0.0, 0.0, 1.0]
    }
}
