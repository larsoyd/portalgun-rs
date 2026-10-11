// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::prelude::*;
use portal::pair::{through_dir, Portal};
use portal::teleport::meets_opening;
use portal::viewbob::{angle_vectors, Angles};

use crate::units::{to_bevy, to_bevy_dir};
use crate::views::through;
use crate::walker::Walker;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapshot {
    pub eye: Vec3,
    pub centre: Vec3,
    pub half: Vec3,
    pub velocity: Vec3,
}

impl Snapshot {
    pub fn capture(player: &Walker) -> Self {
        Self {
            eye: player.eye().into(),
            centre: player.centre().into(),
            half: player.half().into(),
            velocity: player.velocity.into(),
        }
    }

    fn interpolate(self, next: Self, fraction: f32) -> Self {
        Self {
            eye: self.eye.lerp(next.eye, fraction),
            centre: self.centre.lerp(next.centre, fraction),
            half: self.half.lerp(next.half, fraction),
            velocity: self.velocity.lerp(next.velocity, fraction),
        }
    }
}

#[derive(Resource)]
pub struct VisualHistory {
    previous: Snapshot,
    current: Snapshot,
    pub revision: u64,
    reset_revision: u64,
    teleport_rotation: Quat,
}

impl VisualHistory {
    pub fn new(player: &Walker) -> Self {
        let current = Snapshot::capture(player);
        Self {
            previous: current,
            current,
            revision: 0,
            reset_revision: 0,
            teleport_rotation: Quat::IDENTITY,
        }
    }

    pub fn reset(&mut self, player: &Walker) {
        self.current = Snapshot::capture(player);
        self.previous = self.current;
        self.revision = self.revision.wrapping_add(1);
        self.reset_revision = self.reset_revision.wrapping_add(1);
        self.teleport_rotation = Quat::IDENTITY;
    }

    pub fn teleport(&mut self, player: &Walker, from: &Portal, to: &Portal) {
        self.current = Snapshot::capture(player);
        self.previous = self.current;
        self.revision = self.revision.wrapping_add(1);
        let rotation = Mat3::from_cols(
            through_dir(from, to, [1.0, 0.0, 0.0]).into(),
            through_dir(from, to, [0.0, 1.0, 0.0]).into(),
            through_dir(from, to, [0.0, 0.0, 1.0]).into(),
        );
        self.teleport_rotation = (Quat::from_mat3(&rotation) * self.teleport_rotation).normalize();
    }

    pub fn synchronize(&mut self, player: &Walker) {
        if self.current != Snapshot::capture(player) {
            self.reset(player);
        }
    }

    pub fn advance(&mut self, player: &Walker, teleported: bool) {
        if teleported {
            // Forced duck and exit collision can invalidate a rigidly transformed eye history.
            self.reset(player);
        } else {
            self.previous = self.current;
            self.current = Snapshot::capture(player);
        }
    }

    pub fn sample(&self, player: &Walker, fraction: f32) -> VisualPlayer {
        VisualPlayer {
            pose: self.previous.interpolate(self.current, fraction),
            // Mouse look and portal turns are immediate, including across +/-180 degrees.
            angles: Angles {
                pitch: player.pitch,
                yaw: player.yaw,
                roll: 0.0,
            },
            revision: self.revision,
            reset_revision: self.reset_revision,
            teleport_rotation: self.teleport_rotation,
        }
    }
}

#[derive(Resource, Clone, Copy)]
pub struct VisualPlayer {
    pub pose: Snapshot,
    pub angles: Angles,
    pub revision: u64,
    pub reset_revision: u64,
    pub teleport_rotation: Quat,
}

impl VisualPlayer {
    pub fn portal_camera(&self, linked: Option<[Portal; 2]>) -> Transform {
        let camera = self.camera();
        let Some(pair) = linked else { return camera };
        match self.camera_portal(Some(pair)) {
            Some(i) => Transform::from_matrix(through(&pair[i], &pair[1 - i]) * camera.to_matrix()),
            None => camera,
        }
    }

    pub fn camera_portal(&self, linked: Option<[Portal; 2]>) -> Option<usize> {
        let pair = linked?;
        for (i, p) in pair.iter().enumerate() {
            let eye = p.local(self.pose.eye.to_array());
            if eye[0] >= 0.0 {
                continue;
            }
            let feet = self.pose.centre - Vec3::Z * self.pose.half.z;
            let foot = p.local(feet.to_array());
            let ceiling_eye = p.forward[2] < -0.01 && foot[0] > 0.0 && {
                let t = foot[0] / (foot[0] - eye[0]);
                let at = feet.lerp(self.pose.eye, t);
                p.inside(p.local(at.to_array()))
            };
            // The eye can cross before or after the hull centre, especially at ceilings.
            if ceiling_eye
                || meets_opening(p, self.pose.centre.to_array(), self.pose.half.to_array())
            {
                return Some(i);
            }
        }
        None
    }

    pub fn camera(&self) -> Transform {
        let (forward, _, up) = angle_vectors(self.angles);
        Transform::from_translation(to_bevy(self.pose.eye.to_array()))
            .looking_to(to_bevy_dir(forward), to_bevy_dir(up))
    }
}
