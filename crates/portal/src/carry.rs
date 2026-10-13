// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::pair::{through_dir, through_point, Portal, Vec3};
use crate::teleport::{exit_velocity, Mover};
use crate::viewbob::{angle_vectors, Angles};

pub type Basis = [Vec3; 3];
pub const IDENTITY: Basis = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

#[derive(Debug, Clone)]
pub struct RigidCube {
    pub position: Vec3,
    pub rotation: Basis,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    pub owner: Option<usize>,
    pub half_size: f32,
    pub mass: f32,
}

impl RigidCube {
    pub fn at(position: Vec3) -> Self {
        Self {
            position,
            rotation: IDENTITY,
            velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            owner: None,
            half_size: 20.0,
            mass: 40.000004,
        }
    }

    // Translated from 0x1f9270 (decompiled, server.so)
    pub fn limit_release_velocity(&mut self, normal_speed: f32) {
        for (velocity, limit) in [
            (&mut self.velocity, 1.5 * normal_speed),
            (&mut self.angular_velocity, 720.0),
        ] {
            let speed = dot(*velocity, *velocity).sqrt();
            if speed > limit {
                *velocity = velocity.map(|v| v * limit / speed);
            }
        }
    }

    pub fn half(&self) -> Vec3 {
        [0, 1, 2].map(|i| self.half_size * self.rotation.iter().map(|v| v[i].abs()).sum::<f32>())
    }

    pub fn support(&self, direction: Vec3) -> f32 {
        self.half_size
            * self
                .rotation
                .iter()
                .map(|axis| dot(*axis, direction).abs())
                .sum::<f32>()
    }

    pub fn rotate(&mut self, dt: f32) {
        let speed = dot(self.angular_velocity, self.angular_velocity).sqrt();
        if speed > 0.0 {
            let axis = self.angular_velocity.map(|v| v / speed);
            self.rotation = self
                .rotation
                .map(|v| crate::viewbob::rotate_about(axis, speed * dt, v));
        }
    }

    pub fn contact(&mut self, normal: Vec3, restitution: f32, friction: f32) {
        let arm: Vec3 = [0, 1, 2].map(|i| {
            -self.half_size
                * self
                    .rotation
                    .iter()
                    .map(|axis| {
                        let projection = dot(*axis, normal);
                        axis[i]
                            * if projection.abs() < 0.00001 {
                                0.0
                            } else {
                                projection.signum()
                            }
                    })
                    .sum::<f32>()
        });
        let omega = self.angular_velocity.map(f32::to_radians);
        let at = add(self.velocity, cross(omega, arm));
        let speed = dot(at, normal);
        if speed >= 0.0 {
            return;
        }
        let inv_inertia = 1.5 / (self.mass * self.half_size * self.half_size);
        let angular = cross(arm, normal);
        let impulse =
            -(1.0 + restitution) * speed / (1.0 / self.mass + dot(angular, angular) * inv_inertia);
        self.velocity = add(self.velocity, normal.map(|v| v * impulse / self.mass));
        let tangent = sub(at, normal.map(|v| v * speed));
        let tangent_speed = dot(tangent, tangent).sqrt();
        let drag = if tangent_speed > 0.0 {
            (friction * impulse).min(tangent_speed * self.mass) / tangent_speed
        } else {
            0.0
        };
        let impulse_vector = sub(normal.map(|v| v * impulse), tangent.map(|v| v * drag));
        self.velocity = sub(self.velocity, tangent.map(|v| v * drag / self.mass));
        self.angular_velocity =
            add(omega, cross(arm, impulse_vector).map(|v| v * inv_inertia)).map(f32::to_degrees);
    }

    // Translated from 0x922ff0 (decompiled, server.so)
    pub fn teleport(&mut self, from: &Portal, to: &Portal, exit: usize) {
        self.position = through_point(from, to, self.position);
        self.rotation = self.rotation.map(|axis| through_dir(from, to, axis));
        self.velocity = exit_velocity(from, to, self.velocity, Mover::Object);
        self.angular_velocity = through_dir(from, to, self.angular_velocity);
        self.owner = Some(exit);
    }
}

// Translated from 0x9123a0 and 0x8a9930 (decompiled, server.so)
pub fn pickup_eligible(mass: f32, size: Vec3, movable: bool) -> bool {
    movable && mass <= 85.0 && size.iter().all(|v| *v <= 128.0)
}

// Translated from 0x207940 (decompiled, server.so)
pub fn carry_pitch(pitch: f32) -> f32 {
    pitch.clamp(-75.0, 75.0)
}

// Translated from 0x207940 (decompiled, server.so)
pub fn hold_distance(radius: f32, fraction: f32) -> f32 {
    if fraction < 0.5 {
        radius * 0.5
    } else {
        24.0 + radius
    }
}

// Translated from 0x209460 (decompiled, server.so)
pub fn throw_impulse(mass: f32) -> f32 {
    ((mass.clamp(0.5, 15.0) - 0.5) * 0.2413793 + 0.5) * 1000.0
}

#[derive(Debug, Clone)]
pub struct Carry {
    pub relative_rotation: Basis,
    pub portal: Option<usize>,
    pub error: f32,
    pub elapsed: f32,
}

impl Carry {
    pub fn new(rotation: Basis, yaw: f32, portal: Option<usize>) -> Self {
        Self::with_angles(
            rotation,
            Angles {
                yaw,
                ..Angles::default()
            },
            portal,
        )
    }

    pub fn with_angles(rotation: Basis, angles: Angles, portal: Option<usize>) -> Self {
        Self {
            relative_rotation: align_rotation(relative_rotation(rotation, angles)),
            portal,
            error: 0.0,
            elapsed: -1.0,
        }
    }

    // Translated from 0x2074a0 (decompiled, server.so)
    pub fn update_error(&mut self, distance: f32, dt: f32, looking_through: bool) -> bool {
        self.elapsed += dt;
        if self.elapsed > 0.0 {
            let weight = self.elapsed.min(1.0);
            let sample = if distance / weight > 1000.0 {
                distance * 0.5
            } else {
                distance
            };
            self.error = sample * weight + (1.0 - weight) * self.error;
            if self.portal.is_some() && !looking_through {
                self.error *= 2.5;
            }
            self.elapsed = 0.0;
        }
        self.error > 12.0
    }

    // Translated from 0x207940 (decompiled, server.so)
    pub fn target(
        &self,
        eye: Vec3,
        forward: Vec3,
        distance: f32,
        yaw: f32,
        pair: Option<&[Portal; 2]>,
    ) -> Option<(Vec3, Basis)> {
        self.target_angles(
            eye,
            forward,
            distance,
            Angles {
                yaw,
                ..Angles::default()
            },
            pair,
        )
    }

    pub fn target_angles(
        &self,
        eye: Vec3,
        forward: Vec3,
        distance: f32,
        angles: Angles,
        pair: Option<&[Portal; 2]>,
    ) -> Option<(Vec3, Basis)> {
        let target = [0, 1, 2].map(|i| eye[i] + forward[i] * distance);
        let frame = carry_frame(angles);
        let rotation = self
            .relative_rotation
            .map(|v| [0, 1, 2].map(|i| (0..3).map(|j| frame[j][i] * v[j]).sum()));
        match self.portal {
            None => Some((target, rotation)),
            Some(i) => pair.map(|p| {
                (
                    through_point(&p[i], &p[1 - i], target),
                    rotation.map(|v| through_dir(&p[i], &p[1 - i], v)),
                )
            }),
        }
    }

    pub fn player_crossed(&mut self, entrance: usize) {
        self.portal = match self.portal {
            Some(i) if i == entrance => None,
            None => Some(1 - entrance),
            other => other,
        };
    }

    pub fn object_crossed(&mut self, entrance: usize) {
        self.portal = match self.portal {
            Some(i) if i == 1 - entrance => None,
            None => Some(entrance),
            other => other,
        };
    }
}

// Translated from 0x1f9110 (decompiled, server.so)
pub fn relative_rotation(rotation: Basis, angles: Angles) -> Basis {
    let frame = carry_frame(angles);
    rotation.map(|v| frame.map(|axis| dot(axis, v)))
}

fn carry_frame(mut angles: Angles) -> Basis {
    angles.pitch = 0.0;
    let (forward, right, up) = angle_vectors(angles);
    [forward, right.map(|v| -v), up]
}

pub fn yaw_rotate(v: Vec3, yaw: f32) -> Vec3 {
    let (s, c) = yaw.to_radians().sin_cos();
    [v[0] * c - v[1] * s, v[0] * s + v[1] * c, v[2]]
}

// Translated from 0x1f9900 (decompiled, server.so)
pub fn align_rotation(mut basis: Basis) -> Basis {
    for i in (0..3).rev() {
        if let Some(axis) = (0..3).find(|&a| basis[i][a].abs() > 0.8660254) {
            let sign = basis[i][axis].signum();
            basis[i] = [0.0; 3];
            basis[i][axis] = sign;
            let j = (i + 1) % 3;
            let k = (i + 2) % 3;
            basis[k] = cross(basis[i], basis[j]);
            basis[j] = cross(basis[k], basis[i]);
            for a in [j, k] {
                let len = dot(basis[a], basis[a]).sqrt();
                basis[a] = basis[a].map(|v| v / len);
            }
        }
    }
    basis
}

pub fn dot(a: Vec3, b: Vec3) -> f32 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| a[i] + b[i])
}
pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| a[i] - b[i])
}
pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn rotation_velocity(from: Basis, to: Basis, dt: f32) -> Vec3 {
    let m: Basis =
        [0, 1, 2].map(|i| [0, 1, 2].map(|j| (0..3).map(|k| to[k][i] * from[k][j]).sum()));
    let trace = m[0][0] + m[1][1] + m[2][2];
    let mut q = [0.0; 4];
    if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        q = [
            (m[2][1] - m[1][2]) / s,
            (m[0][2] - m[2][0]) / s,
            (m[1][0] - m[0][1]) / s,
            s * 0.25,
        ];
    } else {
        let i = (0..3)
            .max_by(|a, b| m[*a][*a].total_cmp(&m[*b][*b]))
            .unwrap();
        let j = (i + 1) % 3;
        let k = (i + 2) % 3;
        let s = (1.0 + m[i][i] - m[j][j] - m[k][k]).max(0.0).sqrt() * 2.0;
        q[i] = s * 0.25;
        q[j] = (m[j][i] + m[i][j]) / s;
        q[k] = (m[k][i] + m[i][k]) / s;
        q[3] = (m[k][j] - m[j][k]) / s;
    }
    if q[3] < 0.0 {
        q = q.map(|v| -v);
    }
    let length = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt();
    if length < 1e-6 || dt <= 0.0 {
        return [0.0; 3];
    }
    let speed = (2.0 * length.atan2(q[3]).to_degrees() / dt).min(3600.0);
    [q[0], q[1], q[2]].map(|v| v * speed / length)
}

// Translated from 0x209460 (decompiled, server.so)
pub fn throw_torque(mass: f32, random_axes: Vec3) -> Vec3 {
    random_axes.map(|v| v * (throw_impulse(mass) / 1000.0))
}
