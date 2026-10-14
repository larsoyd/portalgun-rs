// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use super::hull::{
    add, closest, cross, dot, gap, length, normalise, scale, sub, Convex, Gap, Key, D3,
};
use super::{
    combine, damping, normal_impulse, rotation_steps, Motion, Quat, Rest, SleepCheck, TOLERANCE,
};

const T: f64 = TOLERANCE as f64;
/// a static side counts as this many times the moving side's mass
const STATIC_MASS: f64 = 100000.0;
const MAX_SPEED: f64 = 2000.0 * 0.0254;
const MAX_SPIN: f64 = 3600.0 * std::f64::consts::PI / 180.0;
const ENERGY_DECAY: f64 = -0.10536054521799088;

/// World geometry the body touches, with its surface's friction and elasticity.
#[derive(Debug, Clone)]
pub struct Solid {
    pub id: usize,
    pub hull: Convex,
    pub friction: f32,
    pub elasticity: f32,
}

#[derive(Debug, Clone)]
pub struct Contact {
    pub solid: usize,
    pub key: Key,
    pub gap: Gap,
    pub pressure: f64,
    mu: f64,
    span: D3,
    time: f64,
    slipping: bool,
    impacts: u32,
    energy: f64,
}

#[derive(Debug, Clone)]
pub struct Body {
    pub hull: Convex,
    pub mass: f64,
    pub inertia: D3,
    pub damp: f32,
    pub rot_damp: [f32; 3],
    pub radius: f32,
    pub friction: f32,
    pub elasticity: f32,
    pub position: D3,
    pub rotation: Quat,
    pub previous_rotation: Quat,
    pub speed: D3,
    /// body frame, radians a second
    pub spin: D3,
    pub asleep: bool,
    pub contacts: Vec<Contact>,
    pair_energy: f64,
    sleep: Option<SleepCheck>,
    sleep_count: i32,
    seed: u32,
}

impl Body {
    pub fn new(hull: Convex, mass: f64, unit_inertia: D3) -> Body {
        Body {
            hull,
            mass,
            inertia: unit_inertia.map(|v| v * mass),
            damp: 0.0,
            rot_damp: [0.0; 3],
            radius: 0.0,
            friction: 0.8,
            elasticity: 0.25,
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            previous_rotation: [0.0, 0.0, 0.0, 1.0],
            speed: [0.0; 3],
            spin: [0.0; 3],
            asleep: false,
            contacts: Vec::new(),
            pair_energy: 0.0,
            sleep: None,
            sleep_count: 10,
            seed: 1,
        }
    }

    pub fn wake(&mut self) {
        self.asleep = false;
        self.sleep = None;
    }

    /// Drops every contact, for a body moved by hand.
    pub fn detach(&mut self) {
        self.contacts.clear();
        self.pair_energy = 0.0;
        self.wake();
    }

    pub fn world_hull(&self) -> Convex {
        let (p, q) = (self.position, self.rotation);
        self.hull.moved(|v| add(rotate(q, v), p), |v| rotate(q, v))
    }

    pub fn world_spin(&self) -> D3 {
        rotate(self.rotation, self.spin)
    }

    pub fn set_world_spin(&mut self, w: D3) {
        self.spin = rotate(conj(self.rotation), w);
    }

    fn inverse_inertia(&self) -> D3 {
        self.inertia.map(|v| 1.0 / v)
    }

    /// The velocity of a world point carried by the body.
    pub fn point_velocity(&self, at: D3) -> D3 {
        add(self.speed, cross(self.world_spin(), sub(at, self.position)))
    }

    fn velocity_change(&self, at: D3, impulse: D3) -> (D3, D3) {
        let r = rotate(conj(self.rotation), sub(at, self.position));
        let j = rotate(conj(self.rotation), impulse);
        let ii = self.inverse_inertia();
        let dw = [0, 1, 2].map(|i| cross(r, j)[i] * ii[i]);
        (scale(impulse, 1.0 / self.mass), dw)
    }

    pub fn push(&mut self, at: D3, impulse: D3) {
        let (dv, dw) = self.velocity_change(at, impulse);
        self.speed = add(self.speed, dv);
        self.spin = add(self.spin, dw);
    }

    /// How the velocity of a point changes for a unit impulse at another.
    fn response(&self, at: D3, from: D3, direction: D3) -> D3 {
        let (dv, dw) = self.velocity_change(from, direction);
        add(dv, cross(rotate(self.rotation, dw), sub(at, self.position)))
    }

    fn normal_mass(&self, at: D3, n: D3) -> f64 {
        1.0 / dot(self.response(at, at, n), n)
    }

    /// Translated from 0xde5b0 (decompiled, vphysics.so)
    fn friction_mass(&self, at: D3) -> f64 {
        let r = rotate(conj(self.rotation), sub(at, self.position));
        let ii = self.inverse_inertia();
        let worst = [
            (r[1] * r[1] + r[2] * r[2]) * ii[0],
            (r[2] * r[2] + r[0] * r[0]) * ii[1],
            (r[1] * r[1] + r[0] * r[0]) * ii[2],
        ]
        .into_iter()
        .fold(f64::MIN, f64::max);
        1.0 / (worst + 1.0 / self.mass)
    }

    fn kinetic(&self) -> f64 {
        0.5 * self.mass * dot(self.speed, self.speed)
            + 0.5
                * (0..3)
                    .map(|i| self.inertia[i] * self.spin[i] * self.spin[i])
                    .sum::<f64>()
    }

    fn motion(&self) -> Motion {
        Motion {
            position: self.position,
            rotation: self.rotation,
            previous_rotation: self.previous_rotation,
            spin: self.spin.map(|v| v as f32),
            radius: self.radius,
        }
    }
}

/// One simulation step, the controllers then the motion with any impacts inside it.
/// Translated from 0xe4920 (decompiled, vphysics.so)
pub fn step(body: &mut Body, world: &[Solid], gravity: D3, dt: f64, now: f64) {
    if body.asleep {
        return;
    }
    refresh(body, world, now);
    let (angular, linear) = damping(body.rot_damp, body.damp, dt);
    body.spin = [0, 1, 2].map(|i| body.spin[i] * angular[i] as f64);
    body.speed = add(scale(body.speed, linear), scale(gravity, dt));
    friction(body, dt);
    normal(body, dt);
    body.sleep_count -= 1;
    if body.sleep_count <= 0 {
        body.seed = body.seed.wrapping_mul(0x4b);
        let random = (body.seed & 0xffff) as f32 * (1.0 / 65536.0);
        body.sleep_count = (random * 5.0) as i32 + 15;
        let motion = body.motion();
        let check = body
            .sleep
            .get_or_insert_with(|| SleepCheck::new(&motion, now));
        if check.test(&motion, now) == Rest::Calm {
            body.asleep = true;
            body.speed = [0.0; 3];
            body.spin = [0.0; 3];
            return;
        }
    }
    limit(body);
    body.previous_rotation = body.rotation;
    advance(body, world, dt, now);
}

/// Translated from 0x103de0 and 0x103e80 (decompiled, vphysics.so)
fn limit(body: &mut Body) {
    let speed = length(body.speed);
    if speed > MAX_SPEED {
        body.speed = scale(body.speed, 0.99 * MAX_SPEED / speed);
    }
    let spin = length(body.spin);
    if spin > MAX_SPIN {
        body.spin = scale(body.spin, 0.9 * MAX_SPIN / spin);
    }
}

/// Recomputes each contact's gap, carries its slip along and drops the lost ones.
/// Translated from 0xf8be0 and 0xe9940 (decompiled, vphysics.so)
fn refresh(body: &mut Body, world: &[Solid], now: f64) {
    let hull = body.world_hull();
    let mut released = 0.0;
    let old = std::mem::take(&mut body.contacts);
    for mut c in old {
        let Some(solid) = world.iter().find(|s| s.id == c.solid) else {
            continue;
        };
        let Some(g) = gap(&hull, &solid.hull, c.key) else {
            continue;
        };
        if g.distance >= 4.5 * T || (c.slipping && !g.inside) {
            continue;
        }
        released += (c.gap.distance - g.distance) * c.pressure;
        let v = body.point_velocity(g.on_cube);
        let tangent = sub(v, scale(g.normal, dot(v, g.normal)));
        c.span = add(c.span, scale(tangent, now - c.time));
        c.span = sub(c.span, scale(g.normal, dot(c.span, g.normal)));
        c.time = now;
        c.gap = g;
        c.impacts = 0;
        body.contacts.push(c);
    }
    if body.contacts.len() >= 2 && released > 0.0 {
        body.pair_energy += released;
    }
}

/// Holds each contact's point where it touched, up to the friction limit.
/// Translated from 0xea430 and 0xe65d0 (decompiled, vphysics.so)
fn friction(body: &mut Body, dt: f64) {
    let mut gained = 0.0;
    for i in 0..body.contacts.len() {
        let c = body.contacts[i].clone();
        let mu = c.mu;
        let inv_mred = 1.0 / body.friction_mass(c.gap.on_cube);
        let max_span = mu * c.pressure * inv_mred * dt * dt;
        let mut span = c.span;
        let mut slipping = c.slipping;
        let len = length(span);
        if len * len > max_span * max_span + 1e-6 {
            slipping = true;
            span = scale(span, max_span / len);
        }
        let lim = c.pressure * mu * dt;
        let mut energy = c.energy;
        if lim >= 1e-6 {
            let n = c.gap.normal;
            let t1 = normalise(if length(span) > 1e-12 {
                span
            } else {
                perpendicular(n)
            });
            let t2 = cross(n, t1);
            let at = c.gap.on_cube;
            let k = [t1, t2].map(|a| [t1, t2].map(|b| dot(body.response(at, at, b), a)));
            let det = k[0][0] * k[1][1] - k[0][1] * k[1][0];
            if det * det >= 1e-38 {
                let v = body.point_velocity(at);
                let want = [t1, t2].map(|t| -dot(span, t) / dt - dot(v, t));
                let mut imp = [
                    (k[1][1] * want[0] - k[0][1] * want[1]) / det,
                    (k[0][0] * want[1] - k[1][0] * want[0]) / det,
                ];
                let size = (imp[0] * imp[0] + imp[1] * imp[1]).sqrt();
                if size > lim {
                    imp = imp.map(|v| v * lim / size);
                }
                body.push(at, add(scale(t1, imp[0]), scale(t2, imp[1])));
                let now_energy =
                    0.5 * (dot(span, span) * dt * dt * (imp[0] * imp[0] + imp[1] * imp[1])).sqrt();
                gained += now_energy - energy;
                energy = now_energy;
            }
        }
        let c = &mut body.contacts[i];
        c.span = span;
        c.slipping = slipping;
        c.energy = energy;
    }
    if body.contacts.len() >= 2 {
        if gained > 0.0 {
            body.pair_energy += gained;
        }
        remove_energy(body, dt);
    }
}

/// Translated from 0xe9860 and 0xe97b0 (decompiled, vphysics.so)
fn remove_energy(body: &mut Body, dt: f64) {
    body.pair_energy *= (dt * ENERGY_DECAY).exp();
    let total = body.kinetic();
    if body.pair_energy < 0.0 || total < 1e-19 {
        return;
    }
    let removed = body.pair_energy.min(0.1 * total);
    body.pair_energy -= removed;
    let keep = (1.0 - removed / total).max(0.0).sqrt();
    body.speed = scale(body.speed, keep);
    body.spin = scale(body.spin, keep);
}

/// Pushes contacts apart until each one sits at twice the tolerance.
/// Translated from 0xea2d0, 0xe5790 and 0xec400 (decompiled, vphysics.so)
fn normal(body: &mut Body, dt: f64) {
    body.contacts.retain(|c| c.gap.distance < 4.5 * T);
    match body.contacts.len() {
        0 => {}
        1 => {
            let c = body.contacts[0].clone();
            let at = c.gap.on_cube;
            let closing = -dot(body.point_velocity(at), c.gap.normal);
            let imp = normal_impulse(
                closing,
                c.gap.distance as f32,
                body.normal_mass(at, c.gap.normal) as f32,
            );
            body.contacts[0].pressure = imp / dt;
            if imp > 0.0 {
                body.push(at, scale(c.gap.normal, imp));
            }
        }
        _ => solve_normals(body, dt),
    }
}

fn solve_normals(body: &mut Body, dt: f64) {
    let active: Vec<usize> = (0..body.contacts.len())
        .filter(|&i| body.contacts[i].gap.distance <= 2.0 * T + 0.01 * T)
        .collect();
    for c in body.contacts.iter_mut() {
        c.pressure = 0.0;
    }
    let geometry: Vec<(D3, D3)> = active
        .iter()
        .map(|&i| (body.contacts[i].gap.on_cube, body.contacts[i].gap.normal))
        .collect();
    let a: Vec<Vec<f64>> = geometry
        .iter()
        .map(|&(pi, ni)| {
            geometry
                .iter()
                .map(|&(pj, nj)| dot(body.response(pi, pj, nj), ni))
                .collect()
        })
        .collect();
    let b: Vec<f64> = active
        .iter()
        .zip(&geometry)
        .map(|(&i, &(p, n))| {
            let error = 2.0 * T - body.contacts[i].gap.distance;
            let gain = if error < 0.0 { 20.0 } else { 1.0 };
            -dot(body.point_velocity(p), n) + gain * error
        })
        .collect();
    let mut x = vec![0.0; active.len()];
    for _ in 0..250 {
        let mut change: f64 = 0.0;
        for i in 0..x.len() {
            let ax: f64 = (0..x.len()).map(|j| a[i][j] * x[j]).sum();
            let next = (x[i] + (b[i] - ax) / a[i][i]).max(0.0);
            change = change.max((next - x[i]).abs());
            x[i] = next;
        }
        if change < 1e-9 {
            break;
        }
    }
    for (k, &i) in active.iter().enumerate() {
        body.contacts[i].pressure = x[k] / dt;
        if x[k] > 0.0 {
            let (p, n) = geometry[k];
            body.push(p, scale(n, x[k]));
        }
    }
}

/// Moves the body through the step, stopping at each impact on the way.
/// Translated from 0x12f9f0, 0x11bae0 and 0x103410 (decompiled, vphysics.so)
fn advance(body: &mut Body, world: &[Solid], dt: f64, now: f64) {
    let mut left = dt;
    for _ in 0..32 {
        let Some((at, solid, key)) = next_impact(body, world, left) else {
            break;
        };
        drift(body, at);
        left -= at;
        let hull = body.world_hull();
        let solid = &world[solid];
        let Some(g) = gap(&hull, &solid.hull, key) else {
            break;
        };
        let index = match body
            .contacts
            .iter()
            .position(|c| c.solid == solid.id && c.key == key)
        {
            Some(i) => i,
            None => {
                body.contacts.push(Contact {
                    solid: solid.id,
                    key,
                    gap: g,
                    pressure: 0.0,
                    mu: combine(body.friction, solid.friction) as f64,
                    span: [0.0; 3],
                    time: now + dt - left,
                    slipping: false,
                    impacts: 0,
                    energy: 0.0,
                });
                body.contacts.len() - 1
            }
        };
        body.contacts[index].gap = g;
        impact_system(body, world, index, dt);
        if left <= 0.0 {
            break;
        }
    }
    drift(body, left.max(0.0));
}

/// Translated from 0xf6270 (decompiled, vphysics.so)
fn impact_system(body: &mut Body, world: &[Solid], first: usize, dt: f64) {
    let mut next = Some(first);
    for _ in 0..5001 {
        let Some(i) = next else {
            return;
        };
        body.contacts[i].impacts += 1;
        let solid = world.iter().find(|s| s.id == body.contacts[i].solid);
        let (e, mu) = solid.map_or((0.0, 0.0), |s| {
            (
                combine(body.elasticity, s.elasticity),
                combine(body.friction, s.friction),
            )
        });
        impact(body, i, e as f64, mu as f64, dt);
        let hull = body.world_hull();
        next = None;
        let mut best = T;
        for (k, c) in body.contacts.iter_mut().enumerate() {
            let Some(s) = world.iter().find(|s| s.id == c.solid) else {
                continue;
            };
            let Some(g) = gap(&hull, &s.hull, c.key) else {
                continue;
            };
            c.gap = g;
            let closing = -dot(
                add(
                    body.speed,
                    cross(
                        rotate(body.rotation, body.spin),
                        sub(g.on_cube, body.position),
                    ),
                ),
                g.normal,
            );
            let predicted =
                g.distance - (bias(g.distance, body.spin, body.radius, dt) * 0.5 + closing) * dt;
            if predicted < best {
                best = predicted;
                next = Some(k);
            }
        }
    }
}

/// The extra separating speed an impact adds for a body inside the tolerance or spinning.
/// Translated from 0xf5f20 and 0xf5de0 (decompiled, vphysics.so)
fn bias(distance: f64, spin: D3, radius: f32, dt: f64) -> f64 {
    let rescue = if distance >= T {
        0.0
    } else {
        (T - distance) / dt
    };
    let w2 = dot(spin, spin);
    let f = if w2 <= 10000.0 {
        1.0 - (w2 * 2.5e-5).sqrt().cos()
    } else {
        0.12241743810962724
    };
    2.0 * (radius as f64 * f / dt + rescue)
}

/// Translated from 0xf46f0 and 0xf35b0 (decompiled, vphysics.so)
fn impact(body: &mut Body, i: usize, e: f64, mu: f64, dt: f64) {
    let c = body.contacts[i].clone();
    let (at, n) = (c.gap.on_cube, c.gap.normal);
    let repeats = c.impacts.saturating_sub(1) as f64;
    let min_sep = (bias(c.gap.distance, body.spin, body.radius, dt) + 2.0 * T) * 1.2;
    let separating = |b: &Body| dot(b.point_velocity(at), n);
    let push_apart = |b: &mut Body, resting: bool| {
        let mut v = separating(b);
        if v > 0.0 && resting {
            v = 0.0;
        }
        let unit = dot(b.response(at, at, n), n);
        let need = (min_sep - v) / (unit + 1e-15);
        if need >= 0.0 {
            b.push(at, scale(n, need));
        }
    };
    let v0 = separating(body);
    if v0 > -1e-4 {
        push_apart(body, true);
        return;
    }
    let mred = -2.0 * v0 * body.mass * STATIC_MASS / (1.0 + STATIC_MASS);
    let x = mu * (e.sqrt() + 1.0);
    let a = x.atan();
    let cos = 1.0 - a * a * 0.5 + a.powi(4) * (1.0 / 24.0);
    let sin = cos * x;
    let mut converged = false;
    for _ in 0..100 {
        let v = body.point_velocity(at);
        if dot(v, n) >= 0.0 {
            converged = true;
            break;
        }
        let mut d = normalise(scale(v, -1.0));
        if dot(d, n) < cos {
            let t = normalise(sub(d, scale(n, dot(d, n))));
            d = add(scale(t, sin), scale(n, cos));
        }
        body.push(at, scale(d, 0.1 * mred));
    }
    let e_eff = 1.0 + (1.0 / (repeats * 0.5 + 1.0)) * (e - 1.0);
    let target = e_eff.sqrt() * -v0 + repeats.sqrt() * 0.01;
    let need = target - separating(body);
    if need > 0.0 && converged {
        let k = dot(body.response(at, at, n), n);
        if k.abs() > 1e-4 {
            body.push(at, scale(n, need / k));
        }
    }
    push_apart(body, false);
    limit(body);
}

/// The earliest time inside the step at which a new feature pair comes within the tolerance.
/// Translated from 0x11bae0 and 0x11c060 (decompiled, vphysics.so)
fn next_impact(body: &Body, world: &[Solid], left: f64) -> Option<(f64, usize, Key)> {
    let reach = length(body.speed) + length(body.spin) * body.radius as f64;
    let range = body.radius as f64 + reach * left + 4.5 * T;
    let near: Vec<usize> = (0..world.len())
        .filter(|&s| {
            let (lo, hi) = bounds(&world[s].hull);
            (0..3).all(|i| body.position[i] + range >= lo[i] && body.position[i] - range <= hi[i])
        })
        .collect();
    let mut time = 0.0;
    for _ in 0..64 {
        let mut ghost = body.clone();
        drift(&mut ghost, time);
        let hull = ghost.world_hull();
        let mut nearest: Option<(f64, usize, Key)> = None;
        for &s in &near {
            if let Some((key, g)) = closest(&hull, &world[s].hull) {
                let known = body
                    .contacts
                    .iter()
                    .any(|c| c.solid == world[s].id && c.key == key);
                let due = if known {
                    g.distance < T
                } else {
                    g.distance < 1.1 * T
                };
                if due {
                    return Some((time, s, key));
                }
                if nearest.is_none_or(|n| g.distance < n.0) {
                    nearest = Some((g.distance, s, key));
                }
            }
        }
        let (distance, _, _) = nearest?;
        if reach <= 0.0 {
            return None;
        }
        time += (distance - T).max(0.0005) / reach;
        if time >= left {
            return None;
        }
    }
    None
}

fn bounds(hull: &Convex) -> (D3, D3) {
    hull.points
        .iter()
        .fold(([f64::MAX; 3], [f64::MIN; 3]), |(lo, hi), p| {
            (
                [0, 1, 2].map(|i| lo[i].min(p[i])),
                [0, 1, 2].map(|i| hi[i].max(p[i])),
            )
        })
}

/// Carries the body along its current velocities.
fn drift(body: &mut Body, dt: f64) {
    if dt <= 0.0 {
        return;
    }
    body.position = add(body.position, scale(body.speed, dt));
    let steps = rotation_steps(length(body.spin), dt);
    let h = dt / steps as f64;
    for _ in 0..steps {
        gyroscope(body, h);
        let w = rotate(body.rotation, body.spin);
        body.rotation = normalise_quat(mul(turn(w, h), body.rotation));
    }
}

/// Translated from 0x12f9f0 (decompiled, vphysics.so)
fn gyroscope(body: &mut Body, h: f64) {
    let ii = body.inverse_inertia();
    let i = body.inertia;
    let w = body.spin;
    body.spin = [
        w[0] + w[1] * w[2] * (i[1] - i[2]) * ii[0] * h,
        w[1] + w[2] * w[0] * (i[2] - i[0]) * ii[1] * h,
        w[2] + w[0] * w[1] * (i[0] - i[1]) * ii[2] * h,
    ];
}

fn perpendicular(n: D3) -> D3 {
    if n[0].abs() < 0.9 {
        cross(n, [1.0, 0.0, 0.0])
    } else {
        cross(n, [0.0, 1.0, 0.0])
    }
}

pub fn rotate(q: Quat, v: D3) -> D3 {
    let u = [q[0], q[1], q[2]];
    let t = scale(cross(u, v), 2.0);
    add(add(v, scale(t, q[3])), cross(u, t))
}

pub fn conj(q: Quat) -> Quat {
    [-q[0], -q[1], -q[2], q[3]]
}

pub fn mul(a: Quat, b: Quat) -> Quat {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

fn turn(w: D3, h: f64) -> Quat {
    let angle = length(w) * h;
    if angle < 1e-12 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let axis = normalise(w);
    let (s, c) = (angle * 0.5).sin_cos();
    [axis[0] * s, axis[1] * s, axis[2] * s, c]
}

fn normalise_quat(q: Quat) -> Quat {
    let l = (q.iter().map(|v| v * v).sum::<f64>()).sqrt();
    q.map(|v| v / l)
}
