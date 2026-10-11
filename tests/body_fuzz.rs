use bevy::prelude::*;
use portal::install::{root, Install};
use portal::pair::Portal;
use portalgun::bodyanim::{BodyAnimation, PlayerClips};
use portalgun::chamber::Chamber;
use portalgun::visual::VisualHistory;
use portalgun::walker::{Input, Walker};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

fn seeds() -> std::ops::Range<u64> {
    let first = std::env::var("BODY_FUZZ_SEED")
        .map(|v| v.parse().expect("BODY_FUZZ_SEED must be an integer"))
        .unwrap_or(1);
    let count = std::env::var("BODY_FUZZ_COUNT")
        .map(|v| v.parse().expect("BODY_FUZZ_COUNT must be an integer"))
        .unwrap_or(128);
    first..first + count
}

#[test]
fn seeded_movement_keeps_the_rig_attached_through_duck_jump_and_portals() {
    let home = std::env::var("HOME").unwrap();
    let install = Install::open(&root(None, None, std::path::Path::new(&home))).unwrap();
    let chamber = Chamber::test_chamber(&install.search).unwrap();
    let clips = PlayerClips::read(&install.search).unwrap();
    let walls = [
        Portal::new(
            [-384.0, -120.0, 54.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [0.0, -256.0, 54.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
    ];
    let mut crossings = [0; 3];
    let mut takeoffs = 0;
    let mut crouches = 0;
    for seed in seeds() {
        let mut rng = Rng(seed.wrapping_add(1).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let scenario = (seed % 3) as usize;
        let (pair, mut walker) = match scenario {
            0 => (walls, Walker::at([-350.0, -120.0, 0.0])),
            1 => (
                [
                    Portal::new(
                        [0.0, -120.0, 0.0],
                        [0.0, 0.0, 1.0],
                        [0.0, 1.0, 0.0],
                        (32.0, 54.0),
                    ),
                    walls[0],
                ],
                Walker::at([0.0, -120.0, 20.0]),
            ),
            _ => {
                let mut w = Walker::at([0.0, -120.0, 280.0]);
                w.velocity[2] = 500.0;
                (
                    [
                        Portal::new(
                            [0.0, -120.0, 384.0],
                            [0.0, 0.0, -1.0],
                            [0.0, 1.0, 0.0],
                            (32.0, 54.0),
                        ),
                        Portal::new(
                            [-250.0, 0.0, 0.0],
                            [0.0, 0.0, 1.0],
                            [0.0, 1.0, 0.0],
                            (32.0, 54.0),
                        ),
                    ],
                    w,
                )
            }
        };
        walker.yaw = 180.0;
        let mut history = VisualHistory::new(&walker);
        let mut animation = BodyAnimation::default();
        let mut input = Input::default();
        let mut trail = Vec::new();
        let mut now = 0.0;
        let mut previous_offset: Option<Vec3> = None;
        for step in 0..600 {
            if step < 30 {
                input.forward = if scenario == 0 { 1.0 } else { 0.0 };
            } else if step % 7 == 0 {
                walker.yaw += rng.next() * 180.0 - 90.0;
                walker.pitch = rng.next() * 178.0 - 89.0;
                input.forward = rng.next() * 2.0 - 1.0;
                input.side = rng.next() * 2.0 - 1.0;
                input.crouch = rng.next() < 0.5;
            }
            input.jump = step > 30 && rng.next() < 0.08;
            input.jump_pressed = input.jump;
            let crossed = walker.step(&input, 0.015, &chamber, Some(pair));
            if let Some(i) = crossed {
                history.teleport(&walker, &pair[i], &pair[1 - i]);
                crossings[scenario] += 1;
            } else {
                history.advance(&walker, false);
            }
            assert!(
                walker.feet[0].abs() < 401.0
                    && walker.feet[1].abs() < 273.0
                    && (-110.0..400.0).contains(&walker.feet[2]),
                "seed={seed} step={step} feet={:?} trail={trail:?}",
                walker.feet
            );
            takeoffs += usize::from(walker.jumped);
            crouches += usize::from(walker.grounded && walker.duck.ducked);
            trail.push((
                step,
                input.crouch,
                walker.grounded,
                walker.feet[2],
                walker.duck.eye,
            ));
            if trail.len() > 8 {
                trail.remove(0);
            }
            // Mix high-refresh frames, ordinary frames and occasional render hitches.
            let dt = [1.0 / 240.0, 1.0 / 60.0, 1.0 / 30.0, 0.2][(rng.next() * 4.0) as usize];
            now += dt;
            if rng.next() < 0.06 {
                animation.fire(now);
            }
            let visual = history.sample(&walker, rng.next());
            let pose = animation
                .pose(
                    &clips,
                    &visual,
                    walker.grounded,
                    walker.duck.ducked || walker.duck.ducking,
                    now,
                    dt,
                )
                .unwrap_or_else(|e| panic!("seed={seed} step={step}: {e}"));
            let offset = pose.origin - visual.pose.eye;
            if let Some(previous) = previous_offset {
                assert!(offset.distance(previous) < 200.0 * dt + 0.01,
                    "seed={seed} step={step} offset={offset:?} previous={previous:?} dt={dt} trail={trail:?}");
            }
            previous_offset = Some(offset);
            assert!(pose.origin.is_finite(), "seed={seed} step={step}");
            assert!(pose.yaw.is_finite(), "seed={seed} step={step}");
            assert!(
                visual.portal_camera(Some(pair)).to_matrix().is_finite(),
                "seed={seed} step={step}"
            );
            for (i, matrix) in pose.bones.iter().enumerate() {
                assert!(
                    matrix.is_finite() && (matrix.determinant() - 1.0).abs() < 0.005,
                    "seed={seed} step={step} bone={i} matrix={matrix:?}"
                );
                let position = matrix.w_axis.truncate();
                assert!(
                    position.length() < 150.0,
                    "seed={seed} step={step} bone={i} position={position:?}"
                );
                if let Some(parent) = clips.mdl.bones[i].parent {
                    let rest = Vec3::from(clips.mdl.bones[i].pos).length();
                    let length = position.distance(pose.bones[parent].w_axis.truncate());
                    if rest > 2.0 {
                        assert!(
                            (rest * 0.5..rest * 1.5).contains(&length),
                            "seed={seed} step={step} bone={i} length={length} rest={rest}"
                        );
                    }
                }
            }
        }
    }
    assert!(
        crossings.iter().sum::<usize>() > 0 && takeoffs > 0 && crouches > 0,
        "coverage: crossings={crossings:?}, takeoffs={takeoffs}, crouches={crouches}"
    );
    eprintln!("coverage: crossings={crossings:?}, takeoffs={takeoffs}, crouches={crouches}");
}

#[test]
fn seeded_clip_seeks_keep_sectioned_and_additive_tracks_finite() {
    let home = std::env::var("HOME").unwrap();
    let install = Install::open(&root(None, None, std::path::Path::new(&home))).unwrap();
    let clips = PlayerClips::read(&install.search).unwrap();
    for seed in seeds() {
        let mut rng = Rng(seed.wrapping_add(1).wrapping_mul(0x2545_f491_4f6c_dd1d));
        for step in 0..300 {
            let seq = (rng.next() * clips.mdl.sequences.len() as f32) as usize;
            let s = &clips.mdl.sequences[seq];
            let params = [0, 1].map(|i| {
                s.param_start[i] + (s.param_end[i] - s.param_start[i]) * (rng.next() * 2.0 - 0.5)
            });
            let cycle = rng.next() * 1.2 - 0.1;
            let pose = clips.sequence(seq, cycle, params).unwrap_or_else(|e| {
                panic!(
                    "seed={seed} step={step} sequence={seq} cycle={cycle} params={params:?}: {e}"
                )
            });
            assert_eq!(pose.len(), clips.mdl.bones.len());
            for (bone, p) in pose.iter().enumerate() {
                assert!(Vec3::from(p.pos).is_finite() &&
                    (Quat::from_array(p.quat).length() - 1.0).abs() < 0.002,
                    "seed={seed} step={step} sequence={seq} cycle={cycle} params={params:?} bone={bone}");
            }
        }
    }
}
