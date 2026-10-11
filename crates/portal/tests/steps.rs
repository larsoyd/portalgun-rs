// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::steps::{landing_volume, Step, Steps};

const TICK: f32 = 0.015;

fn walk(steps: &mut Steps, speed: f32, ticks: usize) -> Vec<(usize, Step)> {
    (0..ticks)
        .filter_map(|t| {
            steps
                .update(TICK, [speed, 0.0, 0.0], true, b'C')
                .map(|s| (t, s))
        })
        .collect()
}

#[test]
fn a_first_step_plays_at_once_then_every_400_ms_walking() {
    let mut s = Steps::default();
    let heard = walk(&mut s, 150.0, 60);
    let ticks: Vec<usize> = heard.iter().map(|(t, _)| *t).collect();
    assert_eq!(
        ticks,
        vec![0, 27, 54],
        "400 ms is 26.7 ticks, a step lands on the tick it reaches 0 or less"
    );
    assert!(heard.iter().all(|(_, s)| s.volume == 0.2));
}

#[test]
fn running_steps_are_louder_and_every_300_ms() {
    let mut s = Steps::default();
    let heard = walk(&mut s, 220.0, 45);
    let ticks: Vec<usize> = heard.iter().map(|(t, _)| *t).collect();
    assert_eq!(ticks, vec![0, 20, 40]);
    assert!(heard.iter().all(|(_, s)| s.volume == 0.5));
    let mut slow = Steps::default();
    assert_eq!(walk(&mut slow, 219.9, 1)[0].1.volume, 0.2);
}

#[test]
fn feet_alternate_starting_on_the_right() {
    let mut s = Steps::default();
    let sides: Vec<bool> = walk(&mut s, 220.0, 45)
        .iter()
        .map(|(_, s)| s.left)
        .collect();
    assert_eq!(sides, vec![false, true, false]);
}

#[test]
fn no_steps_in_the_air_or_standing_still() {
    let mut s = Steps::default();
    assert!(s.update(TICK, [200.0, 0.0, 0.0], false, b'C').is_none());
    assert!(s.update(TICK, [0.0, 0.0, -50.0], true, b'C').is_none());
    assert!(
        s.update(TICK, [1e-4, 0.0, 0.0], true, b'C').is_none(),
        "1e-8 squared is still"
    );
    assert!(
        s.update(TICK, [1.1e-4, 0.0, 0.0], true, b'C').is_some(),
        "any speed above plays"
    );
}

#[test]
fn dirt_and_vents_step_louder() {
    let one = |m: u8, speed: f32| {
        Steps::default()
            .update(TICK, [speed, 0.0, 0.0], true, m)
            .unwrap()
            .volume
    };
    assert_eq!((one(b'D', 100.0), one(b'D', 300.0)), (0.25, 0.55));
    assert_eq!((one(b'V', 100.0), one(b'V', 300.0)), (0.4, 0.7));
    assert_eq!((one(b'M', 100.0), one(b'T', 300.0)), (0.2, 0.5));
}

#[test]
fn a_jump_is_a_full_volume_step_on_the_next_foot() {
    let mut s = Steps::default();
    let j = s.jump();
    assert_eq!(
        j,
        Step {
            left: false,
            volume: 1.0
        }
    );
    assert!(s.jump().left);
    assert_eq!(s.time, 0.0, "a jump leaves the step timer");
}

#[test]
fn landing_volume_follows_the_fall_speed() {
    assert_eq!(landing_volume(1000.0), 1.0);
    assert_eq!(landing_volume(526.6), 1.0);
    assert_eq!(landing_volume(526.5), 0.85);
    assert_eq!(landing_volume(263.3), 0.85);
    assert_eq!(landing_volume(263.25), 0.5);
    assert_eq!(landing_volume(173.0), 0.5);
    assert_eq!(landing_volume(172.9), 0.0);
}

#[test]
fn soft_landings_make_no_sound() {
    let mut s = Steps::default();
    assert!(s.land(302.9).is_none());
    assert!(s.land(0.0).is_none());
    let l = s.land(303.0).expect("303 is heard");
    assert_eq!(l.step.volume, 0.85);
    assert!(l.recover.is_none());
    assert_eq!(s.time, 400.0, "a landing holds the next step 400 ms");
    assert!((l.punch_roll - 303.0 * 0.013).abs() < 1e-4);
}

#[test]
fn hard_landings_add_the_long_fall_boots_sound() {
    let mut s = Steps::default();
    let l = s.land(600.0).unwrap();
    assert_eq!(l.step.volume, 1.0);
    let (pitch, volume) = l.recover.unwrap();
    assert_eq!(pitch, 107, "125 - 0.03 * 600 = 107");
    assert!((volume - (600.0 * 0.00075 - 0.38)).abs() < 1e-5, "{volume}");
    let (low, loud) = s.land(1840.0).unwrap().recover.unwrap();
    assert_eq!(low, 69);
    assert!((loud - 1.0).abs() < 1e-5, "{loud}");
    let (_, top) = s.land(2000.0).unwrap().recover.unwrap();
    assert_eq!(top, 1.0);
}

#[test]
fn crouching_uses_quieter_steps_with_a_longer_interval_and_lower_run_threshold() {
    let mut steps = portal::steps::Steps::default();
    let walking = steps
        .update_duck(0.015, [50.0, 0.0, 0.0], true, b'C', true)
        .unwrap();
    assert!((walking.volume - 0.13).abs() < 0.00001);
    assert_eq!(steps.time, 500.0);
    steps.time = 0.0;
    let running = steps
        .update_duck(0.015, [60.0, 0.0, 0.0], true, b'C', true)
        .unwrap();
    assert!((running.volume - 0.325).abs() < 0.00001);
    assert_eq!(steps.time, 400.0);
}
