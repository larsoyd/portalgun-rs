// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::Path;

use portal::install::{root, Install};
use portal::pair::Portal;
use portalgun::chamber::Chamber;
use portalgun::walker::{Input, Walker};

fn chamber() -> Chamber {
    let dir = root(None, None, Path::new(&std::env::var("HOME").unwrap()));
    let install = Install::open(&dir).unwrap_or_else(|e| panic!("install at {dir:?}: {e}"));
    Chamber::test_chamber(&install.search).unwrap()
}

/// the corner pair from a playtest that let the walker out of the room
fn corner() -> [Portal; 2] {
    [
        Portal::new(
            [343.0, 256.0, 54.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [384.0, 223.0, 54.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
    ]
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

fn inside_room(feet: [f32; 3]) -> bool {
    feet[0] > -385.0
        && feet[0] < 401.0
        && feet[1] > -257.0
        && feet[1] < 273.0
        && feet[2] > -0.5
        && feet[2] < 313.0
}

#[test]
fn wandering_through_a_corner_pair_never_leaves_the_room() {
    let c = chamber();
    let pair = corner();
    for seed in 1..200u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let mut w = Walker::at([330.0, 200.0, 0.0]);
        let mut trail = Vec::new();
        for step in 0..1500 {
            if step % 20 == 0 {
                w.yaw = rng.next() * 360.0;
            }
            let input = Input {
                forward: rng.next() * 2.0 - 1.0,
                side: rng.next() * 2.0 - 1.0,
                jump: rng.next() < 0.02,
                ..Input::default()
            };
            let crossed = w.step(&input, 0.015, &c, Some(pair));
            trail.push((step, w.feet.map(|v| v.round()), crossed));
            if trail.len() > 8 {
                trail.remove(0);
            }
            assert!(
                inside_room(w.feet),
                "seed={seed} step={step} feet={:?} trail={trail:?}",
                w.feet
            );
        }
    }
}

fn within_walls(feet: [f32; 3]) -> bool {
    feet[0].abs() < 401.0
        && feet[1] > -273.0
        && feet[1] < 273.0
        && feet[2] > -110.0
        && feet[2] < 400.0
}

#[test]
fn wandering_between_random_portal_pairs_never_leaves_the_walls() {
    use portal::placement::{place, Placement};
    let c = chamber();
    let mut tried = 0;
    for seed in 1..400u64 {
        let mut rng = Rng(seed.wrapping_mul(0x2545_f491_4f6c_dd1d));
        let shoot = |rng: &mut Rng, other: Option<&Portal>| {
            let eye = [
                rng.next() * 700.0 - 350.0,
                rng.next() * 480.0 - 240.0,
                20.0 + rng.next() * 300.0,
            ];
            let solid = c
                .blocks
                .iter()
                .any(|b| (0..3).all(|i| eye[i] >= b.min[i] && eye[i] <= b.max[i]));
            if solid {
                return None;
            }
            let (yaw, pitch) = (rng.next() * std::f32::consts::TAU, rng.next() * 3.0 - 1.5);
            let dir = [
                pitch.cos() * yaw.cos(),
                pitch.cos() * yaw.sin(),
                pitch.sin(),
            ];
            match place(&c, eye, dir, 56755.84, other).0 {
                Placement::Placed { portal, .. } => Some(portal),
                Placement::Failed(_) => None,
            }
        };
        let Some(blue) = shoot(&mut rng, None) else {
            continue;
        };
        let Some(orange) = shoot(&mut rng, Some(&blue)) else {
            continue;
        };
        tried += 1;
        let pair = [blue, orange];
        let start = blue.world([40.0, 0.0, 0.0]);
        let mut w = Walker::at([start[0], start[1], start[2].clamp(0.0, 300.0)]);
        let mut trail = Vec::new();
        for step in 0..1500 {
            if step % 25 == 0 {
                w.yaw = rng.next() * 360.0;
            }
            let input = Input {
                forward: rng.next() * 2.0 - 1.0,
                side: rng.next() * 2.0 - 1.0,
                jump: rng.next() < 0.02,
                ..Input::default()
            };
            let crossed = w.step(&input, 0.015, &c, Some(pair));
            trail.push((step, w.feet.map(|v| v.round()), crossed));
            if trail.len() > 8 {
                trail.remove(0);
            }
            assert!(
                within_walls(w.feet),
                "seed={seed} step={step} blue={:?}/{:?} orange={:?}/{:?} trail={trail:?}",
                blue.origin,
                blue.forward,
                orange.origin,
                orange.forward
            );
        }
    }
    assert!(tried > 100, "only {tried} pairs placed");
}

#[test]
fn seeded_edge_cases_never_stay_stuck_leave_the_walls_or_break_the_hull() {
    use portal::movement::MAX_VELOCITY;
    use portal::placement::{place, Placement};
    use portalgun::simulator::Space;
    let c = chamber();
    let (mut tried, mut crossings, mut freed) = (0, 0, 0);
    for seed in 1..=2048u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x5851_f42d);
        let mut shoot = |other: Option<&Portal>| {
            let eye = [
                rng.next() * 700.0 - 350.0,
                rng.next() * 480.0 - 240.0,
                20.0 + rng.next() * 300.0,
            ];
            let (yaw, pitch) = (rng.next() * std::f32::consts::TAU, rng.next() * 3.0 - 1.5);
            let dir = [
                pitch.cos() * yaw.cos(),
                pitch.cos() * yaw.sin(),
                pitch.sin(),
            ];
            match place(&c, eye, dir, 56755.84, other).0 {
                Placement::Placed { portal, .. } => Some(portal),
                Placement::Failed(_) => None,
            }
        };
        let Some(blue) = shoot(None) else { continue };
        let Some(orange) = shoot(Some(&blue)) else {
            continue;
        };
        tried += 1;
        let pair = [blue, orange];
        let space = Space::new(&c, Some(&pair));
        let mut rng = Rng(seed);
        // walk-ins, drops from above and flings up to the max velocity, standing or crouched
        let reach = 20.0 + rng.next() * 140.0;
        let start = blue.world([reach, (rng.next() - 0.5) * 60.0, (rng.next() - 0.5) * 80.0]);
        let mut w = Walker::at([start[0], start[1], start[2].clamp(0.0, 300.0)]);
        let standing = ([-16.0, -16.0, 0.0], [16.0, 16.0, 72.0]);
        if space
            .trace(None, w.feet, w.feet, standing.0, standing.1)
            .start_solid
        {
            continue;
        }
        let speed = rng.next() * MAX_VELOCITY;
        w.velocity = blue.forward.map(|f| -f * speed);
        w.yaw = rng.next() * 360.0;
        let crouch = rng.next() < 0.5;
        let (mut stuck, mut trail) = (0, Vec::new());
        for step in 0..600 {
            let input = Input {
                forward: if step < 40 {
                    1.0
                } else {
                    rng.next() * 2.0 - 1.0
                },
                side: rng.next() * 2.0 - 1.0,
                jump: rng.next() < 0.03,
                jump_pressed: false,
                crouch: crouch ^ (rng.next() < 0.02),
            };
            let crossed = w.step(&input, 0.015, &c, Some(pair));
            crossings += crossed.is_some() as usize;
            trail.push((step, w.feet.map(|v| v.round()), w.owner, crossed));
            if trail.len() > 10 {
                trail.remove(0);
            }
            let hull = ([-16.0, -16.0, 0.0], [16.0, 16.0, w.height()]);
            let solid = space
                .trace(w.owner, w.feet, w.feet, hull.0, hull.1)
                .start_solid;
            freed += (stuck > 0 && !solid) as usize;
            stuck = if solid { stuck + 1 } else { 0 };
            let why = format!(
                "seed={seed} step={step} blue={:?}/{:?} orange={:?}/{:?} v={:?} trail={trail:?}",
                blue.origin, blue.forward, orange.origin, orange.forward, w.velocity
            );
            // the stuck check runs every 13 ticks, then every tick tries one of 54 nudges each 0.05 s
            assert!(
                stuck <= 13 + 54 * 4,
                "stuck in solid for {stuck} ticks {why}"
            );
            assert!(within_walls(w.feet), "outside the walls {why}");
            assert!(w.velocity.iter().all(|v| v.abs() <= MAX_VELOCITY), "{why}");
            // the eye rises for 0.2 s of an unduck while the hull is still ducked, as in Source
            assert!(
                (28.0..=64.0).contains(&w.duck.eye),
                "eye {} outside the duck range {why}",
                w.duck.eye
            );
        }
    }
    println!("[fuzz] pairs={tried} crossings={crossings} freed={freed}");
    assert!(
        tried > 500 && crossings > 500,
        "pairs={tried} crossings={crossings}"
    );
}
