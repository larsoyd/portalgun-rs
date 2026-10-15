// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::prelude::*;

use crate::object_render::CubePose;

#[derive(Clone, Copy)]
struct Pose {
    position: Vec3,
    rotation: Quat,
}

impl Pose {
    fn capture(pose: &CubePose) -> Self {
        Self {
            position: pose.position.into(),
            rotation: Quat::from_mat3(&Mat3::from_cols_array_2d(&pose.rotation)).normalize(),
        }
    }
}

#[derive(Resource)]
pub struct CubeHistory {
    previous: Pose,
    current: Pose,
}

impl CubeHistory {
    pub fn new(pose: &CubePose) -> Self {
        let current = Pose::capture(pose);
        Self {
            previous: current,
            current,
        }
    }

    pub fn advance(&mut self, pose: &CubePose, reset: bool) {
        self.previous = self.current;
        self.current = Pose::capture(pose);
        if reset {
            self.previous = self.current;
        }
    }

    pub fn sample(&self, fraction: f32) -> CubePose {
        let fraction = fraction.clamp(0.0, 1.0);
        CubePose {
            position: self
                .previous
                .position
                .lerp(self.current.position, fraction)
                .to_array(),
            rotation: Mat3::from_quat(
                self.previous
                    .rotation
                    .slerp(self.current.rotation, fraction)
                    .normalize(),
            )
            .to_cols_array_2d(),
        }
    }
}

/// the cube's render colour, none while there is no cube to draw
#[derive(Resource)]
pub struct CubeTint(pub Option<[u8; 4]>);

impl Default for CubeTint {
    fn default() -> Self {
        Self(Some([255; 4]))
    }
}

pub struct ObjectPresentationPlugin;

impl Plugin for ObjectPresentationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CubePose>()
            .init_resource::<CubeTint>()
            .insert_resource(CubeHistory::new(&CubePose::default()))
            .add_systems(
                FixedUpdate,
                record
                    .after(crate::objects::ObjectsUpdated)
                    .run_if(crate::menu::playing),
            )
            .add_systems(
                Update,
                present
                    .after(crate::objects::ObjectInteraction)
                    .after(crate::firing::fire)
                    .before(crate::gun_visual::GunVisualUpdate),
            );
    }
}

fn record(
    objects: Res<crate::objects::Objects>,
    mut history: ResMut<CubeHistory>,
    mut revision: Local<u64>,
) {
    if let Some((cube, _)) = objects.shown() {
        history.advance(
            &CubePose {
                position: cube.position,
                rotation: cube.rotation,
            },
            *revision != objects.visual_revision,
        );
    }
    *revision = objects.visual_revision;
}

fn present(
    objects: Res<crate::objects::Objects>,
    history: Res<CubeHistory>,
    fixed: Res<Time<Fixed>>,
    mut pose: ResMut<CubePose>,
    mut tint: ResMut<CubeTint>,
    mut gun: ResMut<crate::gun_visual::GunVisualState>,
) {
    *pose = history.sample(fixed.overstep_fraction());
    tint.0 = objects.shown().map(|(_, colour)| colour);
    gun.holding = objects.holding();
}

#[cfg(test)]
mod tests {
    use super::*;
    use portal::pair::{through_dir, through_point, Portal};

    fn pose(position: [f32; 3], yaw: f32) -> CubePose {
        CubePose {
            position,
            rotation: Mat3::from_quat(Quat::from_rotation_z(yaw.to_radians())).to_cols_array_2d(),
        }
    }

    fn same(actual: &CubePose, expected: &CubePose) {
        assert!(Vec3::from(actual.position).distance(Vec3::from(expected.position)) < 0.0001);
        for (a, b) in actual.rotation.iter().zip(expected.rotation) {
            assert!(
                Vec3::from(*a).distance(Vec3::from(b)) < 0.0001,
                "rotation={:?} expected={:?}",
                actual.rotation,
                expected.rotation
            );
        }
    }

    #[test]
    fn cube_samples_position_between_fixed_steps_without_changing_history() {
        let first = pose([10., 20., 30.], 0.);
        let last = pose([14., 28., 42.], 0.);
        let mut history = CubeHistory::new(&first);
        history.advance(&last, false);
        for (fraction, position) in [
            (0., first.position),
            (0.25, [11., 22., 33.]),
            (0.5, [12., 24., 36.]),
            (1., last.position),
            (-1., first.position),
            (2., last.position),
        ] {
            same(&history.sample(fraction), &pose(position, 0.));
        }
        same(&history.sample(0.5), &pose([12., 24., 36.], 0.));
    }

    #[test]
    fn cube_rotation_takes_short_arc_across_yaw_wrap() {
        let mut history = CubeHistory::new(&pose([0.; 3], 170.));
        history.advance(&pose([0.; 3], -170.), false);
        same(&history.sample(0.5), &pose([0.; 3], 180.));
        for fraction in [0.1, 0.25, 0.5, 0.75, 0.9] {
            let basis = Mat3::from_cols_array_2d(&history.sample(fraction).rotation);
            assert!((basis.determinant() - 1.).abs() < 0.00001);
            assert!(basis.x_axis.dot(basis.y_axis).abs() < 0.00001);
        }
    }

    #[test]
    fn cube_teleport_resets_position_and_rotation_for_every_exit() {
        let from = Portal::new([0.; 3], [0., 0., 1.], [1., 0., 0.], (32., 54.));
        for (forward, up) in [
            ([1., 0., 0.], [0., 0., 1.]),
            ([0., 0., 1.], [1., 0., 0.]),
            ([0., 0., -1.], [1., 0., 0.]),
        ] {
            let to = Portal::new([500., 200., 300.], forward, up, (32., 54.));
            let before = pose([4., 5., -1.], 37.);
            let after = CubePose {
                position: through_point(&from, &to, before.position),
                rotation: before.rotation.map(|axis| through_dir(&from, &to, axis)),
            };
            let mut history = CubeHistory::new(&before);
            history.advance(&after, true);
            for fraction in [0., 0.25, 0.5, 0.75, 1.] {
                same(&history.sample(fraction), &after);
            }
            let next = CubePose {
                position: [
                    after.position[0] + 10.,
                    after.position[1],
                    after.position[2],
                ],
                rotation: after.rotation,
            };
            history.advance(&next, false);
            let expected = CubePose {
                position: [after.position[0] + 5., after.position[1], after.position[2]],
                rotation: after.rotation,
            };
            same(&history.sample(0.5), &expected);
        }
    }

    #[test]
    fn unlink_recovery_never_interpolates_through_the_wall() {
        let mut history = CubeHistory::new(&pose([-400., 0., 64.], 90.));
        let safe = pose([-300., 0., 20.], 0.);
        history.advance(&safe, true);
        same(&history.sample(0.), &safe);
        same(&history.sample(0.5), &safe);
    }
}
