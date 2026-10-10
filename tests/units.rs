use bevy::math::Vec3;
use portalgun::units::{from_bevy, to_bevy, to_bevy_dir, METRES_PER_UNIT};

fn close(a: Vec3, b: Vec3) {
    assert!((a - b).length() < 1e-5, "{a} vs {b}");
}

#[test]
fn source_axes_land_on_bevy_axes() {
    close(to_bevy_dir([1.0, 0.0, 0.0]), Vec3::NEG_Z);
    close(to_bevy_dir([0.0, 1.0, 0.0]), Vec3::NEG_X);
    close(to_bevy_dir([0.0, 0.0, 1.0]), Vec3::Y);
}

#[test]
fn a_hundred_units_is_two_and_a_half_metres() {
    assert_eq!(METRES_PER_UNIT, 0.0254);
    close(to_bevy([0.0, 0.0, 100.0]), Vec3::new(0.0, 2.54, 0.0));
    close(
        to_bevy([10.0, -20.0, 30.0]),
        Vec3::new(0.508, 0.762, -0.254),
    );
}

#[test]
fn handedness_survives_the_trip() {
    let (x, y) = ([1.0, 2.0, 3.0], [-4.0, 0.5, 2.0]);
    let cross = [
        x[1] * y[2] - x[2] * y[1],
        x[2] * y[0] - x[0] * y[2],
        x[0] * y[1] - x[1] * y[0],
    ];
    close(to_bevy_dir(x).cross(to_bevy_dir(y)), to_bevy_dir(cross));
}

#[test]
fn round_trip_gives_the_point_back() {
    let p = [123.5, -64.25, 7.0];
    let back = from_bevy(to_bevy(p));
    for i in 0..3 {
        assert!((back[i] - p[i]).abs() < 1e-3, "axis {i}: {back:?} vs {p:?}");
    }
}
