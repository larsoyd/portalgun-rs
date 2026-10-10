use bevy::prelude::*;
use portal::viewbob::{angle_vectors, Angles};
use portalgun::view::eye_relative;

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-4
}

#[test]
fn an_unmoved_gun_sits_at_the_eye() {
    let a = Angles {
        pitch: 20.0,
        yaw: 130.0,
        roll: 0.0,
    };
    let m = eye_relative([5.0, 6.0, 7.0], a, [5.0, 6.0, 7.0], a);
    assert!(m.abs_diff_eq(Mat4::IDENTITY, 1e-5), "{m:?}");
}

#[test]
fn the_gun_offset_is_in_view_space_ahead_left_up() {
    let a = Angles {
        pitch: 20.0,
        yaw: 130.0,
        roll: 0.0,
    };
    let (f, r, u) = angle_vectors(a);
    let eye = [5.0, 6.0, 7.0];
    let at = [0, 1, 2].map(|i| eye[i] + f[i] * 2.0 - r[i] * 3.0 + u[i] * 0.5);
    let m = eye_relative(eye, a, at, a);
    assert!(
        close(m.w_axis.truncate(), Vec3::new(2.0, 3.0, 0.5)),
        "{:?}",
        m.w_axis
    );
}

#[test]
fn a_gun_turned_left_turns_about_view_up() {
    let a = Angles {
        pitch: 0.0,
        yaw: 40.0,
        roll: 0.0,
    };
    let turned = Angles { yaw: 50.0, ..a };
    let m = eye_relative([0.0; 3], a, [0.0; 3], turned);
    let ahead = m.transform_vector3(Vec3::X);
    let r = 10f32.to_radians();
    assert!(close(ahead, Vec3::new(r.cos(), r.sin(), 0.0)), "{ahead:?}");
}
