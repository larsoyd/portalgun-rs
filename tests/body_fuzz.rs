// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::prelude::*;
use portal::install::{root, Install};
use portal::pair::Portal;
use portalgun::bodyanim::{BodyAnimation, PlayerClips, Stance};
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
    let mut crossings = [0; 5];
    let mut takeoffs = 0;
    let mut crouches = 0;
    for seed in seeds() {
        let mut rng = Rng(seed.wrapping_add(1).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let scenario = (seed % 5) as usize;
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
            2 => {
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
            _ => {
                let mut w = Walker::at([0.0, -120.0, 80.0]);
                w.velocity[2] = -(150.0 + rng.next() * 700.0);
                w.duck.ducked = seed % 2 == 0;
                w.duck.eye = if w.duck.ducked { 28.0 } else { 64.0 };
                let (height, normal) = if scenario == 3 {
                    (384.0, -1.0)
                } else {
                    (0.0, 1.0)
                };
                (
                    [
                        Portal::new(
                            [0.0, -120.0, 0.0],
                            [0.0, 0.0, 1.0],
                            [0.0, 1.0, 0.0],
                            (32.0, 54.0),
                        ),
                        Portal::new(
                            [-250.0, 0.0, height],
                            [0.0, 0.0, normal],
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
        let mut accumulator = 0.0;
        let mut tick = 0;
        let mut frame = 0;
        let mut steps = portal::steps::Steps::default();
        while tick < 600 {
            let dt = [1.0 / 240.0, 1.0 / 120.0, 1.0 / 60.0, 1.0 / 30.0, 0.2]
                [(rng.next() * 5.0) as usize];
            now += dt;
            accumulator += dt;
            while accumulator >= 0.015 {
                accumulator -= 0.015;
                if tick < 30 {
                    input.forward = if scenario == 0 { 1.0 } else { 0.0 };
                } else if tick % 7 == 0 {
                    walker.yaw += rng.next() * 180.0 - 90.0;
                    walker.pitch = rng.next() * 178.0 - 89.0;
                    input.forward = rng.next() * 2.0 - 1.0;
                    input.side = rng.next() * 2.0 - 1.0;
                    input.crouch = rng.next() < 0.5;
                }
                input.jump = tick > 30 && rng.next() < 0.08;
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
                    "seed={seed} tick={tick} feet={:?} trail={trail:?}",
                    walker.feet
                );
                takeoffs += usize::from(walker.jumped);
                crouches += usize::from(walker.grounded && walker.duck.ducked);
                if walker.jumped {
                    steps.jump();
                }
                if let Some(fall) = walker.landed {
                    steps.land(fall);
                }
                if let Some(step) = steps.update_duck(
                    0.015,
                    walker.velocity,
                    walker.grounded,
                    b'C',
                    walker.duck.ducked,
                ) {
                    animation.footfall = Some(portalgun::bodyanim::Footfall {
                        left: step.left,
                        at: now - accumulator,
                        interval: steps.time / 1000.0,
                    });
                }
                trail.push((
                    tick,
                    input.crouch,
                    walker.grounded,
                    walker.feet[2],
                    walker.duck.eye,
                ));
                if trail.len() > 8 {
                    trail.remove(0);
                }
                tick += 1;
            }
            if rng.next() < 0.06 {
                animation.fire(now);
            }
            let visual = history.sample(&walker, accumulator / 0.015);
            let feet = visual.pose.centre - Vec3::Z * visual.pose.half.z;
            let support = portalgun::walker::body_support(feet.to_array(), &chamber, Some(&pair));
            let mut pose = animation
                .pose(&clips, &visual, Stance::of(&walker), now, dt)
                .unwrap_or_else(|e| panic!("seed={seed} tick={tick}: {e}"));
            let probe = |at: Vec3| {
                portalgun::walker::body_support(
                    [at.x, at.y, feet.z.max(at.z)],
                    &chamber,
                    Some(&pair),
                )
            };
            portalgun::feet::ground(&clips, &mut pose, walker.grounded, probe);
            assert!(
                pose.origin.abs_diff_eq(feet, 1e-4),
                "body left the entity origin seed={seed} tick={tick} origin={:?} feet={feet:?}",
                pose.origin
            );
            if let Some(floor) = support {
                assert!(
                    pose.origin.z >= floor - 0.01,
                    "body underground seed={seed} tick={tick} origin={:?} floor={floor}",
                    pose.origin
                );
            }
            let yaw = Quat::from_rotation_z(pose.yaw.to_radians());
            for side in 0..2 {
                let boot = portalgun::feet::boot(&clips, &pose, side);
                let at = pose.origin + yaw * boot.w_axis.truncate();
                if let Some(floor) = probe(at) {
                    let bottom = pose.origin.z + portalgun::feet::sole(boot);
                    assert!(bottom >= floor - 0.1,
                        "foot underground seed={seed} tick={tick} side={side} sole={bottom} floor={floor} trail={trail:?}");
                }
            }
            let step = frame;
            frame += 1;
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
