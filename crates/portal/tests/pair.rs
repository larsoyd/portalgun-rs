use portal::pair::{crossing, through_dir, through_point, trace, Hit, Portal};

const X: [f32; 3] = [1.0, 0.0, 0.0];
const Y: [f32; 3] = [0.0, 1.0, 0.0];
const Z: [f32; 3] = [0.0, 0.0, 1.0];

fn neg(v: [f32; 3]) -> [f32; 3] {
    v.map(|x| -x)
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3)
}

/// a wall portal facing +x at the origin and a floor portal facing up at (500, 0, 0)
fn wall_and_floor() -> (Portal, Portal) {
    (
        Portal::new([0.0, 0.0, 0.0], X, Z, (32.0, 56.0)),
        Portal::new([500.0, 0.0, 0.0], Z, X, (32.0, 56.0)),
    )
}

#[test]
fn the_frame_is_right_handed_with_right_across() {
    let (wall, _) = wall_and_floor();
    assert!(close(wall.right, neg(Y)));
    assert!(close(wall.local([3.0, -2.0, 5.0]), [3.0, 2.0, 5.0]));
    assert!(close(wall.world([3.0, 2.0, 5.0]), [3.0, -2.0, 5.0]));
}

#[test]
fn walking_into_one_comes_out_of_the_other_facing_away() {
    let a = Portal::new([0.0, 0.0, 0.0], X, Z, (32.0, 56.0));
    let b = Portal::new([100.0, 300.0, 0.0], Y, Z, (32.0, 56.0));
    assert!(close(through_dir(&a, &b, neg(X)), Y));
    assert!(close(
        through_point(&a, &b, [-5.0, 0.0, 10.0]),
        [100.0, 305.0, 10.0]
    ));
    // a point to the portal's right comes out on the exit's left, as through a doorway
    let right_of_a = [0.0, -10.0, 0.0];
    let out = through_point(&a, &b, right_of_a);
    assert!(close(b.local(out), [0.0, -10.0, 0.0]), "{:?}", b.local(out));
}

#[test]
fn falling_into_a_floor_portal_flings_out_of_a_wall_one() {
    let (wall, floor) = wall_and_floor();
    let fall = [0.0, 0.0, -900.0];
    let out = through_dir(&floor, &wall, fall);
    assert!(close(out, [900.0, 0.0, 0.0]), "{out:?}");
    let back = through_dir(&wall, &floor, out.map(|v| -v));
    assert!(close(back, [0.0, 0.0, 900.0]), "{back:?}");
}

#[test]
fn the_round_trip_is_the_identity() {
    let a = Portal::new([10.0, -40.0, 7.0], [0.6, 0.8, 0.0], Z, (32.0, 56.0));
    let b = Portal::new([-300.0, 20.0, 90.0], [0.0, 0.0, -1.0], Y, (32.0, 56.0));
    for p in [[1.0, 2.0, 3.0], [-50.0, 9.0, 0.5], [0.0, 0.0, 0.0]] {
        assert!(close(through_point(&b, &a, through_point(&a, &b, p)), p));
    }
}

#[test]
fn crossing_needs_the_front_to_back_move_inside_the_opening() {
    let (wall, _) = wall_and_floor();
    assert!(crossing(&wall, [10.0, 0.0, 0.0], [-10.0, 0.0, 0.0]).is_some());
    let t = crossing(&wall, [30.0, 0.0, 0.0], [-10.0, 0.0, 0.0]).unwrap();
    assert!((t - 0.75).abs() < 1e-5);
    assert!(crossing(&wall, [-10.0, 0.0, 0.0], [10.0, 0.0, 0.0]).is_none());
    assert!(crossing(&wall, [10.0, 40.0, 0.0], [-10.0, 40.0, 0.0]).is_none());
    assert!(crossing(&wall, [10.0, 0.0, 60.0], [-10.0, 0.0, 60.0]).is_none());
    assert!(crossing(&wall, [10.0, 0.0, 0.0], [5.0, 0.0, 0.0]).is_none());
    // a fast mover skips past the plane in one step and still crosses
    assert!(crossing(&wall, [5000.0, 0.0, 20.0], [-5000.0, 0.0, -20.0]).is_some());
}

#[test]
fn the_opening_is_its_rectangle() {
    let (wall, _) = wall_and_floor();
    assert!(crossing(&wall, [10.0, 31.0, 55.0], [-10.0, 31.0, 55.0]).is_some());
    assert!(crossing(&wall, [10.0, -33.0, 0.0], [-10.0, -33.0, 0.0]).is_none());
    assert!(crossing(&wall, [10.0, 0.0, -57.0], [-10.0, 0.0, -57.0]).is_none());
}

#[test]
fn a_shot_into_a_portal_continues_out_of_the_other() {
    let (wall, floor) = wall_and_floor();
    // world: the floor the exit sits on and a ceiling at z = 400 above it
    let world = |o: [f32; 3], d: [f32; 3], max: f32| {
        let floor = (d[2] != 0.0).then(|| -o[2] / d[2]).filter(|t| *t >= 0.0);
        let ceiling = (d[2] > 0.0).then(|| (400.0 - o[2]) / d[2]);
        floor.or(ceiling).filter(|t| *t <= max)
    };
    let path = trace(&[wall, floor], [100.0, 0.0, 10.0], neg(X), 2000.0, 4, world);
    assert_eq!(path.legs.len(), 2);
    assert!((path.legs[0].length - 100.0).abs() < 1e-3);
    assert!(close(path.legs[1].dir, Z));
    match path.end {
        Hit::World(p) => assert!(close(p, [510.0, 0.0, 400.0]), "{p:?}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn range_spent_before_a_portal_is_gone_after_it() {
    let (wall, floor) = wall_and_floor();
    let path = trace(
        &[wall, floor],
        [100.0, 0.0, 10.0],
        neg(X),
        150.0,
        4,
        |_, _, _| None,
    );
    assert_eq!(path.end, Hit::Nothing);
    assert_eq!(path.legs.len(), 2);
    assert!(
        (path.legs[1].length - 50.0).abs() < 0.1,
        "{}",
        path.legs[1].length
    );
}

#[test]
fn the_nearer_portal_takes_the_shot() {
    let near = Portal::new([0.0, 0.0, 0.0], X, Z, (32.0, 56.0));
    let far = Portal::new([-200.0, 0.0, 0.0], X, Z, (32.0, 56.0));
    let path = trace(
        &[far, near],
        [100.0, 0.0, 0.0],
        neg(X),
        1000.0,
        1,
        |_, _, _| None,
    );
    assert!(
        (path.legs[0].length - 100.0).abs() < 1e-3,
        "{}",
        path.legs[0].length
    );
}

#[test]
fn facing_portals_stop_after_the_pass_limit() {
    let a = Portal::new([0.0, 0.0, 0.0], X, Z, (32.0, 56.0));
    let b = Portal::new([300.0, 0.0, 0.0], neg(X), Z, (32.0, 56.0));
    let world = |_: [f32; 3], _: [f32; 3], _: f32| None;
    let path = trace(&[a, b], [100.0, 0.0, 0.0], neg(X), 100_000.0, 6, world);
    assert_eq!(path.legs.len(), 7);
    assert_eq!(path.end, Hit::TooManyPortals);
}

#[test]
fn a_shot_from_behind_a_portal_hits_its_wall() {
    let (wall, floor) = wall_and_floor();
    let world = |o: [f32; 3], d: [f32; 3], max: f32| {
        (d[0] > 0.0 && o[0] < -5.0)
            .then(|| (-5.0 - o[0]) / d[0])
            .filter(|t| *t <= max)
    };
    let path = trace(&[wall, floor], [-100.0, 0.0, 0.0], X, 1000.0, 4, world);
    assert_eq!(path.legs.len(), 1);
    assert!(matches!(path.end, Hit::World(_)));
}

#[test]
fn nothing_hit_runs_out_of_range() {
    let (wall, floor) = wall_and_floor();
    let path = trace(
        &[wall, floor],
        [100.0, 0.0, 200.0],
        neg(X),
        50.0,
        4,
        |_, _, _| None,
    );
    assert_eq!(path.end, Hit::Nothing);
    assert!((path.legs[0].length - 50.0).abs() < 1e-3);
}
