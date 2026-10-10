use std::path::Path;

use portal::install::{root, Install};
use portal::placement::Placement;
use portalgun::chamber::Chamber;
use portalgun::gun::{flight_time, stream_at, test_shot, tracer_origin, Gun, REFIRE};
use portalgun::portals::{Colour, Pair};
use portalgun::walker::Walker;

fn chamber() -> Chamber {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    let install = Install::open(&dir).unwrap_or_else(|e| panic!("install at {dir:?}: {e}"));
    Chamber::test_chamber(&install.search).unwrap()
}

fn close(a: [f32; 3], b: [f32; 3]) {
    assert!((0..3).all(|i| (a[i] - b[i]).abs() < 1e-3), "{a:?} vs {b:?}");
}

#[test]
fn the_shot_leaves_thirty_ahead_four_right_and_five_down_of_the_eye() {
    let mut w = Walker::at([0.0, 0.0, 0.0]);
    close(tracer_origin(&w), [30.0, -4.0, 59.0]);
    w.yaw = 90.0;
    close(tracer_origin(&w), [4.0, 30.0, 59.0]);
    w.pitch = 90.0;
    close(tracer_origin(&w), [4.0, -5.0, 34.0]);
}

#[test]
fn a_player_shot_flies_three_thousand_units_a_second() {
    assert_eq!(flight_time([0.0; 3], [3000.0, 0.0, 0.0]), 1.0);
    assert_eq!(flight_time([0.0; 3], [0.0, 0.0, -750.0]), 0.25);
}

#[test]
fn the_stream_bends_from_the_aim_onto_the_landing() {
    let (s, a, e) = ([0.0; 3], [100.0, 0.0, 0.0], [0.0, 100.0, 0.0]);
    close(stream_at(s, a, e, 0.0), s);
    close(stream_at(s, a, e, 1.0), e);
    close(stream_at(s, a, e, 0.5), [25.0, 25.0, 0.0]);
    close(stream_at([10.0; 3], [10.0; 3], [10.0; 3], 0.3), [10.0; 3]);
}

#[test]
fn either_button_waits_half_a_second_and_the_draw_does_not() {
    let c = chamber();
    let pair = Pair::default();
    let w = Walker::at([-200.0, 0.0, 0.0]);
    let mut gun = Gun::default();
    gun.deploy(10.0);
    assert!(gun.fire(Colour::Blue, 10.0, &w, &c, &pair).is_some());
    assert!(gun
        .fire(Colour::Orange, 10.0 + REFIRE - 0.001, &w, &c, &pair)
        .is_none());
    assert!(gun
        .fire(Colour::Orange, 10.0 + REFIRE, &w, &c, &pair)
        .is_some());
    assert_eq!(REFIRE, 0.5);
}

#[test]
fn a_shot_opens_its_portal_when_it_arrives_not_before() {
    let c = chamber();
    let mut pair = Pair::default();
    let mut w = Walker::at([-200.0, 0.0, 0.0]);
    w.yaw = 180.0;
    let mut gun = Gun::default();
    let shot = gun.fire(Colour::Blue, 0.0, &w, &c, &pair).unwrap();
    let Placement::Placed { portal, .. } = shot.placement else {
        panic!("west wall takes a portal: {:?}", shot.placement)
    };
    assert!((portal.origin[0] + 384.0).abs() < 1e-3);
    assert_eq!(shot.ends, portal.origin, "the stream ends on the portal");
    let arrives = flight_time(shot.from, shot.landed);
    assert!((shot.arrives - arrives).abs() < 1e-6);
    assert!(gun.land(arrives - 0.001).is_empty());
    let landed = gun.land(arrives);
    assert_eq!(landed.len(), 1);
    pair.place(Colour::Blue, portal);
    assert!(gun.land(arrives + 1.0).is_empty());
}

#[test]
fn the_streams_end_is_held_off_at_least_a_tenth_of_a_second() {
    let c = chamber();
    let pair = Pair::default();
    let mut w = Walker::at([-360.0, 0.0, 0.0]);
    w.yaw = 180.0;
    let mut gun = Gun::default();
    let shot = gun.fire(Colour::Blue, 2.0, &w, &c, &pair).unwrap();
    assert!(
        shot.arrives - 2.0 < 0.1,
        "a near wall is reached fast: {}",
        shot.arrives
    );
    assert!(
        (shot.stream_ends - 2.1).abs() < 1e-5,
        "{}",
        shot.stream_ends
    );
}

#[test]
fn a_shot_near_the_floor_streams_to_the_bumped_portal_but_times_by_the_hit() {
    let c = chamber();
    let pair = Pair::default();
    let mut w = Walker::at([-300.0, 0.0, 0.0]);
    w.yaw = 180.0;
    w.pitch = (60.0f32 / 84.0).atan().to_degrees();
    let mut gun = Gun::default();
    let shot = gun.fire(Colour::Blue, 0.0, &w, &c, &pair).unwrap();
    let Placement::Placed { portal, .. } = shot.placement else {
        panic!("bumped up the wall: {:?}", shot.placement)
    };
    assert!(shot.landed[2] < 10.0, "hit low at {:?}", shot.landed);
    assert!(
        portal.origin[2] >= 54.0 - 1e-3,
        "portal bumped up to {:?}",
        portal.origin
    );
    assert_eq!(shot.ends, portal.origin);
    assert!((shot.arrives - flight_time(shot.from, shot.landed)).abs() < 1e-6);
}

#[test]
fn the_gun_remembers_which_colour_it_fired_last() {
    let c = chamber();
    let pair = Pair::default();
    let w = Walker::at([-200.0, 0.0, 0.0]);
    let mut gun = Gun::default();
    assert_eq!(gun.last_fired, 0);
    gun.fire(Colour::Orange, 0.0, &w, &c, &pair);
    assert_eq!(gun.last_fired, 2);
    gun.fire(Colour::Blue, 0.1, &w, &c, &pair);
    assert_eq!(
        gun.last_fired, 2,
        "a shot held back by the refire changes nothing"
    );
    gun.fire(Colour::Blue, 1.0, &w, &c, &pair);
    assert_eq!(gun.last_fired, 1);
}

#[test]
fn the_test_shot_scores_a_fit_by_its_quality_and_a_failure_as_nothing() {
    let c = chamber();
    let pair = Pair::default();
    let mut w = Walker::at([-200.0, 0.0, 0.0]);
    assert_eq!(test_shot(Colour::Blue, &w, &c, &pair), 1.0);
    w.yaw = 90.0;
    w.feet = [0.0, 0.0, 0.0];
    assert_eq!(
        test_shot(Colour::Blue, &w, &c, &pair),
        0.0,
        "the metal strip takes no portal"
    );
}
