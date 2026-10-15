// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::pair::Vec3;

const RADIANS: f32 = std::f32::consts::PI / 180.0;
const DEGREES: f32 = 180.0 / std::f32::consts::PI;
const UPRIGHT: f32 = 1.0e-6;

/// rows of a rotation with the translation in the fourth column
pub(super) type Matrix = [[f32; 4]; 3];

/// Translated from 0xa47370 and 0xa48ba0 (decompiled, server.so)
pub(super) fn angle_matrix(angles: Vec3, origin: Vec3) -> Matrix {
    let (sp, cp) = (angles[0] * RADIANS).sin_cos();
    let (sy, cy) = (angles[1] * RADIANS).sin_cos();
    let (sr, cr) = (angles[2] * RADIANS).sin_cos();
    [
        [
            cp * cy,
            sp * sr * cy - cr * sy,
            sp * cr * cy + sr * sy,
            origin[0],
        ],
        [
            cp * sy,
            sp * sr * sy + cr * cy,
            sy * sp * cr - sr * cy,
            origin[1],
        ],
        [-sp, sr * cp, cr * cp, origin[2]],
    ]
}

/// Translated from 0xa48d30 (decompiled, server.so)
pub(super) fn matrix_angles(m: &Matrix) -> (Vec3, Vec3) {
    let flat = m[0][0] * m[0][0] + m[1][0] * m[1][0];
    let pitch = (-m[2][0]).atan2(flat.sqrt()) * DEGREES;
    let angles = if flat > UPRIGHT {
        [
            pitch,
            m[1][0].atan2(m[0][0]) * DEGREES,
            m[2][1].atan2(m[2][2]) * DEGREES,
        ]
    } else {
        [pitch, (-m[0][1]).atan2(m[1][1]) * DEGREES, 0.0]
    };
    (angles, [m[0][3], m[1][3], m[2][3]])
}

/// the inverse of a rigid transform
pub(super) fn invert(m: &Matrix) -> Matrix {
    let mut out = [[0.0; 4]; 3];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = m[j][i];
        }
        out[i][3] = -(0..3).map(|j| m[j][i] * m[j][3]).sum::<f32>();
    }
    out
}

pub(super) fn concat(a: &Matrix, b: &Matrix) -> Matrix {
    let mut out = [[0.0; 4]; 3];
    for i in 0..3 {
        for j in 0..4 {
            out[i][j] =
                (0..3).map(|k| a[i][k] * b[k][j]).sum::<f32>() + if j == 3 { a[i][3] } else { 0.0 };
        }
    }
    out
}
