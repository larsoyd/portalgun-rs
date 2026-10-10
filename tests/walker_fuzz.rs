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
