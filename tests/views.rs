use bevy::prelude::*;
use portal::pair::{through_dir, through_point, Portal};
use portal::view::{clip_plane, Plane};
use portalgun::units::{to_bevy, to_bevy_dir, METRES_PER_UNIT};
use portalgun::views::{frustum_planes, through, view_clip_plane};

fn wall(x: f32, toward: f32) -> Portal {
    Portal::new(
        [x, 0.0, 64.0],
        [toward, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    )
}

fn floor() -> Portal {
    Portal::new(
        [0.0, 300.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        (32.0, 54.0),
    )
}

fn close(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-4
}

#[test]
fn the_portal_matrix_moves_points_and_directions_like_the_crate() {
    let (a, b) = (wall(-384.0, 1.0), floor());
    let m = through(&a, &b);
    let p = [-300.0, 20.0, 90.0];
    assert!(close(
        m.transform_point3(to_bevy(p)),
        to_bevy(through_point(&a, &b, p))
    ));
    let d = [0.3, -0.5, 0.8];
    assert!(close(
        m.transform_vector3(to_bevy_dir(d)),
        to_bevy_dir(through_dir(&a, &b, d))
    ));
}

#[test]
fn the_clip_plane_keeps_what_is_in_front_of_the_exit() {
    let exit = wall(384.0, -1.0);
    let camera = Transform::from_translation(to_bevy([500.0, 0.0, 64.0]))
        .looking_to(to_bevy_dir([-1.0, 0.0, 0.0]), Vec3::Y);
    let plane = view_clip_plane(clip_plane(&exit), &camera);
    let view_from_world = camera.to_matrix().inverse();
    let side = |x: f32| {
        let p = view_from_world.transform_point3(to_bevy([x, 10.0, 64.0]));
        plane.truncate().dot(p) + plane.w
    };
    assert!(side(383.0) > 0.0, "in the room past the exit is kept");
    assert!(side(384.4) > 0.0, "a little behind the exit is kept");
    assert!(side(384.6) < 0.0, "past half a unit behind is cut");
    assert!((plane.truncate().length() - 1.0).abs() < 1e-5);
    assert!(plane.z < 0.0, "the normal points away from the camera");
}

#[test]
fn frustum_planes_come_out_in_source_units() {
    let camera = Transform::from_translation(to_bevy([0.0, 0.0, 64.0]))
        .looking_to(to_bevy_dir([1.0, 0.0, 0.0]), Vec3::Y);
    let projection = PerspectiveProjection {
        fov: 90f32.to_radians(),
        aspect_ratio: 1.0,
        near: 4.0 * METRES_PER_UNIT,
        ..default()
    };
    use bevy::camera::CameraProjection;
    let clip_from_world = projection.get_clip_from_view() * camera.to_matrix().inverse();
    let planes = frustum_planes(&clip_from_world);
    assert_eq!(planes.len(), 5, "no far plane");
    let inside = |p: [f32; 3]| planes.iter().all(|q: &Plane| dot(q.normal, p) > q.dist);
    assert!(inside([100.0, 0.0, 64.0]));
    assert!(inside([100.0, 99.0, 64.0]));
    assert!(!inside([100.0, 101.0, 64.0]));
    assert!(!inside([3.0, 0.0, 64.0]), "nearer than the near plane");
    assert!(inside([5.0, 0.0, 64.0]));
    let near = planes
        .iter()
        .find(|q| q.normal[0] > 0.99)
        .expect("a near plane facing forward");
    assert!((near.dist - 4.0).abs() < 1e-3, "{near:?}");
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
