// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::prelude::*;
use portal::install::{root, Install};
use portalgun::bodyanim::{BodyAnimation, PlayerClips};
use portalgun::visual::VisualHistory;
use portalgun::walker::Walker;

fn clips() -> PlayerClips {
    let home = std::env::var("HOME").unwrap();
    let install = Install::open(&root(None, None, std::path::Path::new(&home))).unwrap();
    PlayerClips::read(&install.search).unwrap()
}

#[test]
fn installed_clips_decode_across_sections_and_all_blend_corners() {
    let clips = clips();
    for (index, seq) in clips.mdl.sequences.iter().enumerate() {
        for cycle in [
            0.0,
            29.9 / 160.0,
            30.0 / 160.0,
            30.1 / 160.0,
            0.5,
            0.999,
            1.0,
        ] {
            for params in [seq.param_start, seq.param_end, [0.0; 2]] {
                let pose = clips.sequence(index, cycle, params).unwrap();
                assert_eq!(pose.len(), clips.mdl.bones.len());
                for p in pose {
                    assert!(Vec3::from(p.pos).is_finite(), "{} {cycle}", seq.label);
                    assert!(
                        (Quat::from_array(p.quat).length() - 1.0).abs() < 0.002,
                        "{} {cycle}",
                        seq.label
                    );
                }
            }
        }
    }
    let idle = clips.mdl.sequence_named("idle_PORTALGUN").unwrap();
    for boundary in [30.0, 60.0, 90.0, 120.0, 150.0, 160.0] {
        let a = clips
            .sequence(idle, (boundary - 0.01) / 160.0, [0.0; 2])
            .unwrap();
        let b = clips.sequence(idle, boundary / 160.0, [0.0; 2]).unwrap();
        for (a, b) in a.iter().zip(b) {
            assert!(
                Vec3::from(a.pos).distance(b.pos.into()) < 0.1,
                "boundary={boundary}"
            );
            assert!(
                Quat::from_array(a.quat)
                    .angle_between(Quat::from_array(b.quat))
                    .abs()
                    < 0.02,
                "boundary={boundary}"
            );
        }
    }
}

#[test]
fn takeoff_and_landing_preserve_the_previous_leg_pose_then_blend() {
    let clips = clips();
    let walker = Walker::at([0.0; 3]);
    let visual = VisualHistory::new(&walker).sample(&walker, 0.0);
    let mut animation = BodyAnimation::default();
    let standing = animation
        .pose(&clips, &visual, true, false, 0.0, 0.0)
        .unwrap();
    let takeoff = animation
        .pose(&clips, &visual, false, false, 0.016, 0.016)
        .unwrap();
    for name in [
        "thigh_L", "knee_L", "ankle_L", "thigh_R", "knee_R", "ankle_R",
    ] {
        let i = clips.bone(name);
        assert!(
            standing.bones[i].abs_diff_eq(takeoff.bones[i], 1e-5),
            "takeoff snapped {name}"
        );
    }
    let middle = animation
        .pose(&clips, &visual, false, false, 0.116, 0.1)
        .unwrap();
    let airborne = animation
        .pose(&clips, &visual, false, false, 0.216, 0.1)
        .unwrap();
    let knee = clips.bone("knee_L");
    assert!(!middle.bones[knee].abs_diff_eq(standing.bones[knee], 0.01));
    assert!(!middle.bones[knee].abs_diff_eq(airborne.bones[knee], 0.01));
    let landing = animation
        .pose(&clips, &visual, true, false, 0.232, 0.016)
        .unwrap();
    assert!(
        airborne.bones[knee].abs_diff_eq(landing.bones[knee], 1e-5),
        "landing snapped"
    );
    let landed = animation
        .pose(&clips, &visual, true, false, 0.432, 0.2)
        .unwrap();
    assert!(!landed.bones[knee].abs_diff_eq(airborne.bones[knee], 0.01));
}

#[test]
fn idle_aim_and_fire_use_the_weapon_bone_and_crouch_changes_height() {
    let clips = clips();
    let walker = Walker::at([0.0; 3]);
    let mut visual = VisualHistory::new(&walker).sample(&walker, 0.0);
    let mut animation = BodyAnimation::default();
    let idle = animation
        .pose(&clips, &visual, true, false, 0.0, 0.0)
        .unwrap();
    let gun = clips.bone("weapon_bone");
    let muzzle = idle.bones[gun].transform_point3(Vec3::new(0.0, 0.0, -21.0));
    assert!(muzzle.x > 15.0 && muzzle.z > 40.0);
    visual.angles.pitch = -40.0;
    let up = animation
        .pose(&clips, &visual, true, false, 0.0, 0.0)
        .unwrap();
    assert!(!up.bones[gun].abs_diff_eq(idle.bones[gun], 0.01));
    animation.fire(0.0);
    let fired = animation
        .pose(&clips, &visual, true, false, 0.1, 0.1)
        .unwrap();
    assert!(!fired.bones[gun].abs_diff_eq(up.bones[gun], 0.01));
    visual.angles.pitch = 0.0;
    animation
        .pose(&clips, &visual, true, true, 1.0, 0.0)
        .unwrap();
    let crouch = animation
        .pose(&clips, &visual, true, true, 1.3, 0.3)
        .unwrap();
    let head = clips.bone("head");
    assert!(crouch.bones[head].w_axis.z < idle.bones[head].w_axis.z - 10.0);
}

#[test]
fn every_body_and_gun_mesh_is_on_the_portal_only_layer_and_follows_teleports() {
    use bevy::camera::visibility::RenderLayers;
    use portalgun::body::{BodyPlugin, HeldGun, PlayerBody, BODY_LAYER};
    use portalgun::player::Player;
    use portalgun::sound::Files;
    use portalgun::units::to_bevy;
    let home = std::env::var("HOME").unwrap();
    let install = Install::open(&root(None, None, std::path::Path::new(&home))).unwrap();
    let walker = Walker::at([10.0, 20.0, 0.0]);
    let visual = VisualHistory::new(&walker).sample(&walker, 0.0);
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<StandardMaterial>()
        .insert_resource(Files(install.search))
        .insert_resource(Player(walker))
        .insert_resource(visual)
        .add_plugins(BodyPlugin);
    app.update();
    let world = app.world_mut();
    let mut count = 0;
    for layers in world
        .query_filtered::<&RenderLayers, With<Mesh3d>>()
        .iter(world)
    {
        count += 1;
        assert!(
            layers == &RenderLayers::layer(BODY_LAYER)
                || layers == &RenderLayers::layer(portalgun::bodyportal::GHOST_LAYER)
        );
        assert!(!layers.intersects(&RenderLayers::from_layers(&[0, 1, 2])));
    }
    assert!(count > 40);
    assert_eq!(
        world
            .query_filtered::<Entity, With<HeldGun>>()
            .iter(world)
            .count(),
        1
    );
    let body = world
        .query_filtered::<Entity, With<PlayerBody>>()
        .single(world)
        .unwrap();
    assert!(world
        .get::<Transform>(body)
        .unwrap()
        .translation
        .abs_diff_eq(to_bevy([10.0, 20.0, 0.0]), 1e-5));
    let mut walker = Walker::at([-200.0, 150.0, 80.0]);
    walker.yaw = 90.0;
    let mut history = VisualHistory::new(&walker);
    history.reset(&walker);
    app.insert_resource(history.sample(&walker, 0.0));
    app.insert_resource(Player(walker));
    app.update();
    let root = app.world().get::<Transform>(body).unwrap();
    assert!(root
        .translation
        .abs_diff_eq(to_bevy([-200.0, 150.0, 80.0]), 1e-5));
    assert!(root
        .rotation
        .abs_diff_eq(Quat::from_rotation_y(90.0_f32.to_radians()), 1e-5));
}

#[test]
fn jump_duck_and_midair_crouch_toggles_keep_the_body_continuous() {
    use portalgun::chamber::Chamber;
    use portalgun::walker::Input;
    let home = std::env::var("HOME").unwrap();
    let install = Install::open(&root(None, None, std::path::Path::new(&home))).unwrap();
    let chamber = Chamber::test_chamber(&install.search).unwrap();
    let clips = clips();
    for held in [false, true] {
        let mut player = Walker::at([-320.0, -120.0, 0.0]);
        let mut animation = BodyAnimation::default();
        animation.support = Some(0.0);
        let mut previous = Vec3::from(player.feet);
        for step in 0..80 {
            let input = Input {
                jump: step == 1,
                jump_pressed: step == 1,
                crouch: held && (8..55).contains(&step),
                ..default()
            };
            player.step(&input, 0.015, &chamber, None);
            let visual = VisualHistory::new(&player).sample(&player, 0.0);
            let pose = animation
                .pose(
                    &clips,
                    &visual,
                    player.grounded,
                    player.duck.ducked || player.duck.ducking,
                    step as f32 * 0.015,
                    0.015,
                )
                .unwrap();
            assert!(
                pose.origin.distance(previous) < 6.0,
                "held={held} step={step} origin={:?} previous={previous:?}",
                pose.origin
            );
            if step == 2 {
                assert!(player.duck.ducked && player.duck.in_duck_jump);
                assert!(player.feet[2] - pose.origin.z > 35.9);
            }
            previous = pose.origin;
        }
        assert!(player.grounded);
        assert!(previous.abs_diff_eq(Vec3::from(player.feet), 0.001));
    }
}

#[test]
fn jumping_from_crouch_keeps_the_body_above_the_floor() {
    use portalgun::chamber::Chamber;
    use portalgun::walker::Input;
    let home = std::env::var("HOME").unwrap();
    let install = Install::open(&root(None, None, std::path::Path::new(&home))).unwrap();
    let chamber = Chamber::test_chamber(&install.search).unwrap();
    let clips = clips();
    for release in [false, true] {
        let mut player = Walker::at([-320.0, -120.0, 0.0]);
        let mut animation = BodyAnimation::default();
        animation.support = Some(0.0);
        let mut takeoffs = 0;
        for tick in 0..160 {
            player.step(
                &Input {
                    crouch: !release || tick < 80,
                    jump: tick == 60,
                    jump_pressed: tick == 60,
                    ..default()
                },
                0.015,
                &chamber,
                None,
            );
            takeoffs += usize::from(player.jumped);
            let visual = VisualHistory::new(&player).sample(&player, 0.0);
            let pose = animation
                .pose(
                    &clips,
                    &visual,
                    player.grounded,
                    player.duck.ducked || player.duck.ducking,
                    tick as f32 * 0.015,
                    0.015,
                )
                .unwrap();
            assert!(
                pose.origin.z >= -0.01,
                "release={release} tick={tick} body={} feet={} eye={}",
                pose.origin.z,
                player.feet[2],
                player.eye()[2]
            );
            if player.grounded {
                assert!(
                    (pose.origin.z - player.feet[2]).abs() < 0.01,
                    "grounded body left its support: tick={tick} origin={:?}",
                    pose.origin
                );
            }
        }
        assert_eq!(takeoffs, 1);
        assert!(player.grounded);
    }
}

#[test]
fn directional_walk_contacts_follow_the_actual_footstep_clock() {
    use portal::steps::Steps;
    use portalgun::bodyanim::Footfall;
    let clips = clips();
    for crouch in [false, true] {
        for direction in 0..8 {
            for speed in [20.0, if crouch { 60.0 } else { 190.0 }, 230.0] {
                let mut walker = Walker::at([0.0; 3]);
                walker.grounded = true;
                walker.duck.ducked = crouch;
                walker.duck.eye = if crouch { 28.0 } else { 64.0 };
                let a = direction as f32 * std::f32::consts::FRAC_PI_4;
                walker.velocity = [speed * a.cos(), speed * a.sin(), 0.0];
                let visual = VisualHistory::new(&walker).sample(&walker, 0.0);
                let mut steps = Steps::default();
                let mut animation = BodyAnimation::default();
                let mut contacts = 0;
                for tick in 0..180 {
                    let now = tick as f32 * 0.015;
                    let step = steps.update_duck(0.015, walker.velocity, true, b'C', crouch);
                    if let Some(s) = step {
                        animation.footfall = Some(Footfall {
                            left: s.left,
                            at: now,
                            interval: steps.time / 1000.0,
                        });
                    }
                    let mut pose = animation
                        .pose(&clips, &visual, true, crouch, now, 0.015)
                        .unwrap();
                    portalgun::feet::ground(&clips, &mut pose, true, |_| Some(0.0));
                    if let Some(s) = step {
                        let side = usize::from(!s.left);
                        let height = pose.origin.z
                            + portalgun::feet::sole(portalgun::feet::boot(&clips, &pose, side));
                        assert!(height.abs() < 0.1,
                            "sound on lifted foot: crouch={crouch} direction={direction} speed={speed} tick={tick} left={} toe={height}", s.left);
                        contacts += 1;
                    }
                }
                assert!(contacts >= 5);
            }
        }
    }
}

#[test]
fn the_rig_has_two_soles_that_reach_the_support_surface() {
    use bevy::mesh::VertexAttributeValues;
    use portalgun::body::BodyPlugin;
    use portalgun::chamber::Chamber;
    use portalgun::player::Player;
    use portalgun::scene::Level;
    use portalgun::sound::Files;
    let home = std::env::var("HOME").unwrap();
    let install = Install::open(&root(None, None, std::path::Path::new(&home))).unwrap();
    let chamber = Chamber::test_chamber(&install.search).unwrap();
    let mut walker = Walker::at([0.0; 3]);
    walker.grounded = true;
    let visual = VisualHistory::new(&walker).sample(&walker, 0.0);
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<StandardMaterial>()
        .insert_resource(Files(install.search))
        .insert_resource(Level(chamber))
        .insert_resource(Player(walker))
        .insert_resource(visual)
        .add_plugins(BodyPlugin);
    app.update();
    let world = app.world_mut();
    let feet: Vec<_> = world
        .query::<(&Name, &Mesh3d, &GlobalTransform)>()
        .iter(world)
        .filter(|(n, _, _)| n.as_str().starts_with("Player boot"))
        .map(|(_, m, t)| (m.0.clone(), *t))
        .collect();
    assert_eq!(feet.len(), 2, "missing sole and heel geometry");
    for (mesh, transform) in feet {
        let mesh = world.resource::<Assets<Mesh>>().get(&mesh).unwrap();
        let Some(VertexAttributeValues::Float32x3(vertices)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("boot vertices")
        };
        let bottom = vertices
            .iter()
            .map(|v| transform.transform_point(Vec3::from(*v)).y)
            .fold(f32::INFINITY, f32::min);
        assert!(
            bottom.abs() < 0.002,
            "sole does not touch floor: {bottom} metres"
        );
    }
    let mut steps = portal::steps::Steps::default();
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        std::time::Duration::from_secs_f32(0.015),
    ));
    for _ in 0..90 {
        let visual = {
            let mut player = app.world_mut().resource_mut::<Player>();
            player.velocity = [150.0, 0.0, 0.0];
            VisualHistory::new(&player).sample(&player, 0.0)
        };
        let now = app.world().resource::<Time>().elapsed_secs() + 0.015;
        let step = steps.update(0.015, [150.0, 0.0, 0.0], true, b'C');
        if let Some(s) = step {
            app.insert_resource(portalgun::bodyanim::Gait(Some(
                portalgun::bodyanim::Footfall {
                    left: s.left,
                    at: now,
                    interval: steps.time / 1000.0,
                },
            )));
        }
        app.insert_resource(visual);
        app.update();
        if let Some(s) = step {
            let world = app.world_mut();
            let expected = format!("Player boot {}", usize::from(!s.left));
            let (mesh, transform) = world
                .query::<(&Name, &Mesh3d, &GlobalTransform)>()
                .iter(world)
                .find(|(n, _, _)| n.as_str() == expected)
                .map(|(_, m, t)| (m.0.clone(), *t))
                .unwrap();
            let mesh = world.resource::<Assets<Mesh>>().get(&mesh).unwrap();
            let Some(VertexAttributeValues::Float32x3(vertices)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("boot vertices")
            };
            let bottom = vertices
                .iter()
                .map(|v| transform.transform_point(Vec3::from(*v)).y)
                .fold(f32::INFINITY, f32::min);
            assert!(
                bottom.abs() < 0.002,
                "rendered sounding sole is off ground: {bottom}"
            );
        }
    }
}

#[test]
fn body_ground_probes_leave_linked_floor_openings_empty() {
    use portal::pair::Portal;
    use portalgun::{chamber::Chamber, walker::body_support};
    let home = std::env::var("HOME").unwrap();
    let install = Install::open(&root(None, None, std::path::Path::new(&home))).unwrap();
    let chamber = Chamber::test_chamber(&install.search).unwrap();
    let pair = [
        Portal::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], (32.0, 54.0)),
        Portal::new(
            [-384.0, 0.0, 64.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
    ];
    assert_eq!(body_support([0.0, 0.0, 36.0], &chamber, None), Some(0.0));
    assert_eq!(body_support([0.0, 0.0, 36.0], &chamber, Some(&pair)), None);
    assert_eq!(
        body_support([0.0, 70.0, 36.0], &chamber, Some(&pair)),
        Some(0.0)
    );
}

#[test]
fn footsteps_stay_supported_when_starting_stopping_and_changing_stance() {
    use portal::steps::Steps;
    use portalgun::bodyanim::Footfall;
    let clips = clips();
    let mut animation = BodyAnimation::default();
    let mut steps = Steps::default();
    let mut contacts = 0;
    for tick in 0..500 {
        let now = tick as f32 * 0.015;
        let crouch = tick % 97 < 48;
        let moving = tick % 81 < 60;
        let mut player = Walker::at([0.0; 3]);
        player.duck.ducked = crouch;
        player.duck.eye = if crouch { 28.0 } else { 64.0 };
        player.velocity = if moving {
            [if crouch { 60.0 } else { 190.0 }, 0.0, 0.0]
        } else {
            [0.0; 3]
        };
        let visual = VisualHistory::new(&player).sample(&player, 0.0);
        let step = steps.update_duck(0.015, player.velocity, true, b'C', crouch);
        if let Some(s) = step {
            animation.footfall = Some(Footfall {
                left: s.left,
                at: now,
                interval: steps.time / 1000.0,
            });
        }
        let mut pose = animation
            .pose(&clips, &visual, true, crouch, now, 0.015)
            .unwrap();
        portalgun::feet::ground(&clips, &mut pose, true, |_| Some(0.0));
        if let Some(s) = step {
            let sole =
                portalgun::feet::sole(portalgun::feet::boot(&clips, &pose, usize::from(!s.left)));
            assert!(sole.abs() < 0.1, "tick={tick} crouch={crouch} sole={sole}");
            contacts += 1;
        }
    }
    assert!(contacts > 10);
}
