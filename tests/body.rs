// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::prelude::*;
use portal::install::{root, Install};
use portalgun::bodyanim::{BodyAnimation, PlayerClips, Stance};
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
    let air = Stance {
        jumps: 1,
        ..Stance::new(false, false)
    };
    let landed = Stance {
        grounded: true,
        ..air
    };
    let standing = animation
        .pose(&clips, &visual, Stance::new(true, false), 0.0, 0.0)
        .unwrap();
    let takeoff = animation.pose(&clips, &visual, air, 0.016, 0.016).unwrap();
    for name in [
        "thigh_L", "knee_L", "ankle_L", "thigh_R", "knee_R", "ankle_R",
    ] {
        let i = clips.bone(name);
        assert!(
            standing.bones[i].abs_diff_eq(takeoff.bones[i], 1e-5),
            "takeoff snapped {name}"
        );
    }
    let middle = animation.pose(&clips, &visual, air, 0.116, 0.1).unwrap();
    let airborne = animation.pose(&clips, &visual, air, 0.216, 0.1).unwrap();
    let knee = clips.bone("knee_L");
    assert!(!middle.bones[knee].abs_diff_eq(standing.bones[knee], 0.01));
    assert!(!middle.bones[knee].abs_diff_eq(airborne.bones[knee], 0.01));
    let landing = animation
        .pose(&clips, &visual, landed, 0.232, 0.016)
        .unwrap();
    assert!(
        airborne.bones[knee].abs_diff_eq(landing.bones[knee], 1e-5),
        "landing snapped"
    );
    let landed = animation.pose(&clips, &visual, landed, 0.432, 0.2).unwrap();
    assert!(!landed.bones[knee].abs_diff_eq(airborne.bones[knee], 0.01));
}

#[test]
fn idle_aim_and_fire_use_the_weapon_bone_and_crouch_changes_height() {
    let clips = clips();
    let walker = Walker::at([0.0; 3]);
    let mut visual = VisualHistory::new(&walker).sample(&walker, 0.0);
    let mut animation = BodyAnimation::default();
    let idle = animation
        .pose(&clips, &visual, Stance::new(true, false), 0.0, 0.0)
        .unwrap();
    let gun = clips.bone("weapon_bone");
    let muzzle = idle.bones[gun].transform_point3(Vec3::new(0.0, 0.0, -21.0));
    assert!(muzzle.x > 15.0 && muzzle.z > 40.0);
    visual.angles.pitch = -40.0;
    let up = animation
        .pose(&clips, &visual, Stance::new(true, false), 0.0, 0.0)
        .unwrap();
    assert!(!up.bones[gun].abs_diff_eq(idle.bones[gun], 0.01));
    animation.fire(0.0);
    let fired = animation
        .pose(&clips, &visual, Stance::new(true, false), 0.1, 0.1)
        .unwrap();
    assert!(!fired.bones[gun].abs_diff_eq(up.bones[gun], 0.01));
    visual.angles.pitch = 0.0;
    animation
        .pose(&clips, &visual, Stance::new(true, true), 1.0, 0.0)
        .unwrap();
    let crouch = animation
        .pose(&clips, &visual, Stance::new(true, true), 1.3, 0.3)
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
    let casting = world
        .query_filtered::<Entity, (With<Mesh3d>, Without<bevy::light::NotShadowCaster>)>()
        .iter(world)
        .count();
    assert!(casting > 40, "the body casts shadows by default");
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
fn jump_duck_and_midair_crouch_render_at_the_interpolated_entity_origin() {
    use portalgun::chamber::Chamber;
    use portalgun::walker::Input;
    let home = std::env::var("HOME").unwrap();
    let install = Install::open(&root(None, None, std::path::Path::new(&home))).unwrap();
    let chamber = Chamber::test_chamber(&install.search).unwrap();
    let clips = clips();
    for held in [false, true] {
        let mut player = Walker::at([-320.0, -120.0, 0.0]);
        let mut history = VisualHistory::new(&player);
        let mut animation = BodyAnimation::default();
        let mut raised = false;
        for step in 0..80 {
            let input = Input {
                jump: step == 1,
                jump_pressed: step == 1,
                crouch: held && (8..55).contains(&step),
                ..default()
            };
            let before = Vec3::from(player.feet);
            player.step(&input, 0.015, &chamber, None);
            history.advance(&player, false);
            for fraction in [0.0, 0.5, 1.0] {
                let visual = history.sample(&player, fraction);
                let pose = animation
                    .pose(
                        &clips,
                        &visual,
                        Stance::of(&player),
                        step as f32 * 0.015,
                        0.0,
                    )
                    .unwrap();
                let origin = before.lerp(Vec3::from(player.feet), fraction);
                assert!(
                    pose.origin.abs_diff_eq(origin, 1e-3),
                    "held={held} step={step} fraction={fraction} body={:?} entity={origin:?}",
                    pose.origin
                );
            }
            raised |= step == 2 && player.duck.ducked && player.duck.in_duck_jump;
        }
        assert!(raised, "step 2 should be a duck jump");
        assert!(player.grounded);
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
                    Stance::of(&player),
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
                        .pose(&clips, &visual, Stance::new(true, crouch), now, 0.015)
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
            .pose(&clips, &visual, Stance::new(true, crouch), now, 0.015)
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

fn still(velocity: [f32; 3]) -> portalgun::visual::VisualPlayer {
    let mut walker = Walker::at([0.0; 3]);
    walker.velocity = velocity;
    VisualHistory::new(&walker).sample(&walker, 0.0)
}

fn named(clips: &PlayerClips, name: &str) -> usize {
    clips.mdl.sequence_named(name).unwrap()
}

#[test]
fn falling_without_a_jump_event_keeps_the_ground_activity() {
    let clips = clips();
    let mut animation = BodyAnimation::default();
    let air = Stance::new(false, false);
    for (now, vz) in [(0.0, 0.0), (0.1, -100.0), (0.5, -500.0)] {
        let pose = animation
            .pose(&clips, &still([0.0, 0.0, vz]), air, now, 0.1)
            .unwrap();
        assert_eq!(
            pose.sequence,
            named(&clips, "idle_PORTALGUN"),
            "now={now} vz={vz}"
        );
    }
}

#[test]
fn a_jump_event_holds_jump_start_until_grounded_two_tenths_later() {
    let clips = clips();
    let mut animation = BodyAnimation::default();
    let jump = named(&clips, "portalgun_standing_jump");
    let idle = named(&clips, "idle_PORTALGUN");
    let at = |grounded, jumps| Stance {
        jumps,
        ..Stance::new(grounded, false)
    };
    let sequence = |a: &mut BodyAnimation, s, now| {
        a.pose(&clips, &still([0.0; 3]), s, now, 0.015)
            .unwrap()
            .sequence
    };
    assert_eq!(sequence(&mut animation, at(true, 0), 1.0), idle);
    assert_eq!(sequence(&mut animation, at(false, 1), 1.015), jump);
    assert_eq!(sequence(&mut animation, at(true, 1), 1.1), jump);
    assert_eq!(sequence(&mut animation, at(true, 1), 1.2), jump);
    assert_eq!(sequence(&mut animation, at(true, 1), 1.22), idle);
    assert_eq!(sequence(&mut animation, at(false, 1), 1.3), idle);
    assert_eq!(sequence(&mut animation, at(false, 2), 1.4), jump);
    assert_eq!(sequence(&mut animation, at(false, 2), 2.0), jump);
}

#[test]
fn rising_faster_than_300_is_airwalk_until_the_ground() {
    let clips = clips();
    let jump = named(&clips, "portalgun_standing_jump");
    let idle = named(&clips, "idle_PORTALGUN");
    for original in [false, true] {
        let mut animation = BodyAnimation::default();
        animation.original_airwalk = original;
        let mut sequence = |vz, grounded, now| {
            animation
                .pose(
                    &clips,
                    &still([0.0, 0.0, vz]),
                    Stance::new(grounded, false),
                    now,
                    0.015,
                )
                .unwrap()
                .sequence
        };
        assert_eq!(
            sequence(300.0, false, 0.0),
            idle,
            "exactly 300 is not airwalk"
        );
        let air = if original { 0 } else { jump };
        assert_eq!(sequence(300.5, false, 0.015), air, "original={original}");
        assert_eq!(
            sequence(-200.0, false, 0.5),
            air,
            "airwalk stays until landing"
        );
        assert_eq!(sequence(0.0, true, 0.6), idle);
    }
}

#[test]
fn the_original_airwalk_shows_the_bind_pose() {
    let clips = clips();
    let mut animation = BodyAnimation::default();
    animation.original_airwalk = true;
    let mut pose = None;
    for tick in 0..30 {
        pose = Some(
            animation
                .pose(
                    &clips,
                    &still([0.0, 0.0, 400.0]),
                    Stance::new(false, false),
                    tick as f32 * 0.015,
                    0.015,
                )
                .unwrap(),
        );
    }
    let pose = pose.unwrap();
    let knee = clips.bone("knee_L");
    let mut bind = Mat4::IDENTITY;
    let mut chain = vec![knee];
    while let Some(parent) = clips.mdl.bones[*chain.last().unwrap()].parent {
        chain.push(parent);
    }
    for &i in chain.iter().rev() {
        let b = &clips.mdl.bones[i];
        bind *= Mat4::from_rotation_translation(Quat::from_array(b.quat), Vec3::from(b.pos));
    }
    assert!(
        pose.bones[knee].abs_diff_eq(bind, 1e-3),
        "knee {:?} bind {:?}",
        pose.bones[knee],
        bind
    );
}

#[test]
fn the_crouch_fire_gesture_needs_a_finished_duck() {
    let clips = clips();
    let stand = named(&clips, "stand_fire_PORTALGUN");
    let crouch = named(&clips, "crouch_fire_PORTALGUN");
    for (ducked, ducking, gesture) in [
        (false, true, stand),
        (true, false, crouch),
        (true, true, crouch),
    ] {
        let mut animation = BodyAnimation::default();
        animation.fire(0.0);
        let stance = Stance {
            ducked,
            ducking,
            ..Stance::new(true, false)
        };
        let pose = animation
            .pose(&clips, &still([0.0; 3]), stance, 0.05, 0.05)
            .unwrap();
        assert_eq!(
            pose.gesture,
            Some(gesture),
            "ducked={ducked} ducking={ducking}"
        );
    }
}

#[test]
fn feet_yaw_holds_until_squared_speed_passes_the_next_float_above_one() {
    let clips = clips();
    let creep = [1.0, 0.0, f32::EPSILON.sqrt()];
    assert_eq!(Vec3::from(creep).length_squared(), 1.0 + f32::EPSILON);
    let mut animation = BodyAnimation::default();
    let mut visual = still(creep);
    animation
        .pose(&clips, &visual, Stance::new(true, false), 0.0, 0.1)
        .unwrap();
    visual.angles.yaw = 30.0;
    let held = animation
        .pose(&clips, &visual, Stance::new(true, false), 0.1, 0.1)
        .unwrap();
    assert_eq!(held.yaw, 0.0, "a creep at the threshold is still standing");
}

#[test]
fn without_foot_ik_the_feet_keep_the_authored_pose() {
    let clips = clips();
    for ik in [true, false] {
        let mut animation = BodyAnimation::default();
        animation.original_feet = !ik;
        let mut pose = animation
            .pose(&clips, &still([0.0; 3]), Stance::new(true, false), 0.0, 0.0)
            .unwrap();
        let before = pose.bones.clone();
        portalgun::feet::ground(&clips, &mut pose, true, |_| Some(6.0));
        assert_eq!(pose.bones == before, !ik, "ik={ik}");
    }
}

#[test]
fn the_original_gait_ignores_the_footstep_clock() {
    use portalgun::bodyanim::Footfall;
    let clips = clips();
    let walking = still([150.0, 0.0, 0.0]);
    let step = Footfall {
        left: false,
        at: 0.0,
        interval: 0.3,
    };
    let sample = |audio: bool, footfall: Option<Footfall>| {
        let mut animation = BodyAnimation::default();
        animation.original_gait = !audio;
        animation.footfall = footfall;
        let mut last = None;
        for tick in 0..10 {
            last = Some(
                animation
                    .pose(
                        &clips,
                        &walking,
                        Stance::new(true, false),
                        tick as f32 * 0.02,
                        0.02,
                    )
                    .unwrap()
                    .bones,
            );
        }
        last.unwrap()
    };
    assert!(
        sample(true, Some(step)) != sample(true, None),
        "the gait follows the clock"
    );
    assert!(
        sample(false, Some(step)) == sample(false, None),
        "the original ignores it"
    );
}

#[test]
fn the_original_body_casts_no_shadows_but_its_held_gun_and_ghost_still_do() {
    use bevy::camera::visibility::RenderLayers;
    use bevy::light::NotShadowCaster;
    use portalgun::body::{BodyPlugin, HeldGun, BODY_LAYER};
    use portalgun::player::{Config, Player};
    use portalgun::settings::Settings;
    use portalgun::sound::Files;
    let home = std::env::var("HOME").unwrap();
    let install = Install::open(&root(None, None, std::path::Path::new(&home))).unwrap();
    let walker = Walker::at([0.0; 3]);
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<StandardMaterial>()
        .insert_resource(Files(install.search))
        .insert_resource(Config(
            Settings::parse("original_body_shadows = true").unwrap(),
        ))
        .insert_resource(VisualHistory::new(&walker).sample(&walker, 0.0))
        .insert_resource(Player(walker))
        .add_plugins(BodyPlugin);
    app.update();
    let world = app.world_mut();
    let gun = world
        .query_filtered::<Entity, With<HeldGun>>()
        .single(world)
        .unwrap();
    let under_gun = |world: &World, mut e: Entity| loop {
        if e == gun {
            return true;
        }
        match world.get::<ChildOf>(e) {
            Some(parent) => e = parent.parent(),
            None => return false,
        }
    };
    let meshes: Vec<_> = world
        .query_filtered::<(Entity, &RenderLayers, Has<NotShadowCaster>), With<Mesh3d>>()
        .iter(world)
        .map(|(e, l, quiet)| (e, l.clone(), quiet))
        .collect();
    let (mut body, mut held, mut ghost) = (0, 0, 0);
    for (e, layers, quiet) in meshes {
        if layers == RenderLayers::layer(BODY_LAYER) && !under_gun(world, e) {
            assert!(quiet, "body mesh casts a shadow");
            body += 1;
        } else if layers == RenderLayers::layer(BODY_LAYER) {
            assert!(!quiet);
            held += 1;
        } else {
            assert!(!quiet);
            ghost += 1;
        }
    }
    assert!(
        body > 30 && held > 0 && ghost > 0,
        "body={body} held={held} ghost={ghost}"
    );
}

#[test]
fn body_shadow_improvement_can_be_enabled_and_disabled_without_respawning() {
    use bevy::camera::visibility::RenderLayers;
    use bevy::light::NotShadowCaster;
    use portalgun::body::{BodyPlugin, BODY_LAYER};
    use portalgun::player::{Config, Player};
    use portalgun::settings::Settings;
    use portalgun::sound::Files;
    let home = std::env::var("HOME").unwrap();
    let install = Install::open(&root(None, None, std::path::Path::new(&home))).unwrap();
    let walker = Walker::at([0.0; 3]);
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin))
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<StandardMaterial>()
        .insert_resource(Files(install.search))
        .insert_resource(Config(Settings::parse("").unwrap()))
        .insert_resource(VisualHistory::new(&walker).sample(&walker, 0.0))
        .insert_resource(Player(walker))
        .add_plugins(BodyPlugin);
    app.update();
    let body: Vec<Entity> = app
        .world_mut()
        .query_filtered::<(Entity, &RenderLayers), (With<Mesh3d>, With<NotShadowCaster>)>()
        .iter(app.world())
        .filter(|(_, layers)| **layers == RenderLayers::layer(BODY_LAYER))
        .map(|(e, _)| e)
        .collect();
    assert!(body.len() > 30);
    for original in [false, true, false] {
        app.world_mut()
            .resource_mut::<Config>()
            .0
            .original_body_shadows = original;
        app.update();
        for &entity in &body {
            assert_eq!(
                app.world().get::<NotShadowCaster>(entity).is_some(),
                original
            );
        }
    }
}

#[test]
fn feet_yaw_converges_on_the_raw_gap_and_overshoots_across_the_wrap() {
    use portalgun::bodyanim::converge_yaw;
    let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
    assert!(
        close(converge_yaw(170.0, -170.0, 0.01), -177.2),
        "the 340 raw gap saturates the rate"
    );
    assert!(
        close(converge_yaw(179.0, -179.0, 0.1), 109.0),
        "and steps 72 the short way round"
    );
    assert!(
        close(converge_yaw(10.0, 9.99, 0.01), 10.0),
        "inside one step it lands on the goal"
    );
    assert!(
        close(converge_yaw(30.0, 0.0, 0.01), 3.6),
        "30 of 60 runs at half rate"
    );
}
