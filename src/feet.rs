// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::bodyanim::{AnimatedBody, PlayerClips};
use bevy::prelude::*;

pub const BOOT_SIZE: Vec3 = Vec3::new(4.5, 3.0, 10.0);

pub fn boot(clips: &PlayerClips, pose: &AnimatedBody, side: usize) -> Mat4 {
    let toe = pose.bones[clips.bone(if side == 0 { "toe_L" } else { "toe_R" })];
    let sign = if side == 0 { 1.0 } else { -1.0 };
    let forward = toe.y_axis.truncate() * sign;
    let up = toe.z_axis.truncate() * -sign;
    let centre = toe.w_axis.truncate() - forward * 2.0 - up * 1.5;
    Mat4::from_cols(
        forward.cross(up).extend(0.0),
        up.extend(0.0),
        (-forward).extend(0.0),
        centre.extend(1.0),
    )
}

pub fn sole(matrix: Mat4) -> f32 {
    let half = BOOT_SIZE * 0.5;
    matrix.w_axis.z
        - matrix.x_axis.z.abs() * half.x
        - matrix.y_axis.z.abs() * half.y
        - matrix.z_axis.z.abs() * half.z
}

pub fn ground(
    clips: &PlayerClips,
    pose: &mut AnimatedBody,
    grounded: bool,
    probe: impl Fn(Vec3) -> Option<f32>,
) {
    let rotation = Quat::from_rotation_z(pose.yaw.to_radians());
    for side in 0..2 {
        let shoe = boot(clips, pose, side);
        let at = pose.origin + rotation * shoe.w_axis.truncate();
        let Some(floor) = probe(at) else { continue };
        let height = pose.origin.z + sole(shoe);
        let planted = pose.planted == Some(side) && height < floor + 18.0;
        if height < floor || grounded && (height < floor + 1.0 || planted) {
            let ankle = clips.bone(if side == 0 { "ankle_L" } else { "ankle_R" });
            let target = pose.bones[ankle].w_axis.truncate() + Vec3::Z * (floor - height);
            solve_leg(clips, pose, side, target);
        }
    }
}

fn solve_leg(clips: &PlayerClips, pose: &mut AnimatedBody, side: usize, target: Vec3) {
    let suffix = if side == 0 { "L" } else { "R" };
    let [hip, knee, ankle] =
        ["thigh", "knee", "ankle"].map(|n| clips.bone(&format!("{n}_{suffix}")));
    let [a, b, c] = [hip, knee, ankle].map(|i| pose.bones[i].w_axis.truncate());
    let upper = a.distance(b);
    let lower = b.distance(c);
    let direction = (target - a).normalize_or(Vec3::NEG_Z);
    let distance = a
        .distance(target)
        .clamp((upper - lower).abs() + 0.001, upper + lower - 0.001);
    let along = (upper * upper - lower * lower + distance * distance) / (2.0 * distance);
    let bend = ((b - a) - direction * (b - a).dot(direction)).normalize_or(Vec3::X);
    let joint = a + direction * along + bend * (upper * upper - along * along).max(0.0).sqrt();
    let foot_rotation = pose.bones[ankle].to_scale_rotation_translation().1;
    turn(
        clips,
        &mut pose.bones,
        hip,
        Quat::from_rotation_arc((b - a).normalize(), (joint - a).normalize()),
    );
    let b = pose.bones[knee].w_axis.truncate();
    let c = pose.bones[ankle].w_axis.truncate();
    let reachable = a + direction * distance;
    turn(
        clips,
        &mut pose.bones,
        knee,
        Quat::from_rotation_arc((c - b).normalize(), (reachable - b).normalize()),
    );
    let current = pose.bones[ankle].to_scale_rotation_translation().1;
    turn(
        clips,
        &mut pose.bones,
        ankle,
        foot_rotation * current.inverse(),
    );
}

fn turn(clips: &PlayerClips, bones: &mut [Mat4], root: usize, rotation: Quat) {
    let pivot = bones[root].w_axis.truncate();
    let matrix = Mat4::from_rotation_translation(rotation, pivot - rotation * pivot);
    for (i, bone) in bones.iter_mut().enumerate().skip(root) {
        let mut parent = Some(i);
        while let Some(n) = parent {
            if n == root {
                *bone = matrix * *bone;
                break;
            }
            parent = clips.mdl.bones[n].parent;
        }
    }
}
