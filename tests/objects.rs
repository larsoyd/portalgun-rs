// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::carry::{RigidCube, IDENTITY};
use portal::pair::Portal;
use portal::placement::Surface;
use portalgun::chamber::{Block, Chamber};
use portalgun::objects::Objects;
use portalgun::simulator::Space;
use portalgun::walker::Walker;

fn block(min: [f32; 3], max: [f32; 3]) -> Block {
    Block {
        min,
        max,
        material: "",
        surface: Surface::Portalable,
        surfaceprop: "metalpanel".into(),
    }
}
fn room() -> Chamber {
    Chamber {
        blocks: vec![
            block([-512.0, -512.0, -16.0], [512.0, 512.0, 0.0]),
            block([-16.0, -512.0, 0.0], [0.0, 512.0, 512.0]),
        ],
    }
}

#[test]
fn cube_settles_on_the_floor_inside_the_contact_gap_and_sleeps() {
    let mut objects = Objects::default();
    objects.cube = RigidCube::at([100.0, 0.0, 100.0]);
    let player = Walker::at([300.0, 0.0, 0.0]);
    let chamber = room();
    for i in 0..600 {
        objects.advance(0.015, &player, &chamber, None, None, i as f32 * 0.015);
    }
    let gap = objects.cube.position[2] - 20.0;
    assert!(
        (0.2499..=0.625).contains(&gap),
        "rests {gap} above the floor at {:?}",
        objects.cube.position
    );
    assert_eq!(objects.cube.velocity, [0.0; 3], "still moving");
}

#[test]
fn use_picks_up_and_releases_a_cube_in_front() {
    let mut objects = Objects::default();
    objects.cube = RigidCube::at([100.0, 0.0, 40.0]);
    let mut player = Walker::at([40.0, 0.0, 0.0]);
    player.yaw = 0.0;
    player.pitch = 20.0;
    let space = Space::new(&room(), None);
    assert!(objects.use_object(&player, &space, 0.0));
    assert!(objects.holding());
    assert!(objects.use_object(&player, &space, 0.1));
    assert!(!objects.holding());
    assert_eq!(objects.blocked_until, 0.6);
}

#[test]
fn use_cannot_pick_up_through_an_unportaled_wall() {
    let mut objects = Objects::default();
    objects.cube = RigidCube::at([-45.0, 0.0, 40.0]);
    let mut player = Walker::at([25.0, 0.0, 0.0]);
    player.yaw = 180.0;
    player.pitch = 20.0;
    assert!(!objects.use_object(&player, &Space::new(&room(), None), 0.0));
}

#[test]
fn use_selects_a_visible_cube_inside_the_fallback_cone() {
    let mut objects = Objects::default();
    objects.cube = RigidCube::at([100.0, 40.0, 64.0]);
    let mut player = Walker::at([40.0, 0.0, 0.0]);
    player.yaw = 0.0;
    player.pitch = 0.0;
    assert!(objects.use_object(&player, &Space::new(&room(), None), 0.0));
}

#[test]
fn pickup_through_a_close_portal_survives_the_first_physics_tick() {
    let pair = [
        Portal::new(
            [0.0, 0.0, 64.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [200.0, 0.0, 64.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
    ];
    let mut objects = Objects::default();
    objects.cube = RigidCube::at([165.0, 0.0, 64.0]);
    let mut player = Walker::at([25.0, 0.0, 0.0]);
    player.yaw = 180.0;
    player.pitch = 0.0;
    let chamber = room();
    assert!(objects.use_object(&player, &Space::new(&chamber, Some(&pair)), 0.0));
    assert_eq!(objects.carry.as_ref().unwrap().portal, Some(0));
    objects.advance(0.015, &player, &chamber, Some(pair), None, 0.015);
    assert!(objects.holding());
    objects.advance(0.015, &player, &chamber, None, None, 0.03);
    assert!(!objects.holding());
}

#[test]
fn cube_crosses_wall_portal_and_retains_a_rotated_basis() {
    let p = [
        Portal::new(
            [0.0, 0.0, 54.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [200.0, 100.0, 100.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
    ];
    let mut objects = Objects::default();
    objects.cube = RigidCube::at([30.0, 0.0, 54.0]);
    objects.cube.velocity = [-200.0, 0.0, 0.0];
    let player = Walker::at([300.0, 0.0, 0.0]);
    let chamber = room();
    for i in 0..20 {
        objects.advance(0.015, &player, &chamber, Some(p), None, i as f32 * 0.015);
    }
    assert!(
        objects.cube.position[0] > 150.0,
        "{:?}",
        objects.cube.position
    );
    assert_ne!(objects.cube.rotation, IDENTITY);
    assert!(objects.cube.velocity[1] < 0.0);
}

#[test]
fn floor_cube_crossings_work_with_wall_floor_and_ceiling_exits() {
    for normal in [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]] {
        let up = if normal[2] == 0.0 {
            [0.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let pair = [
            Portal::new(
                [100.0, 0.0, 0.0],
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
                (32.0, 54.0),
            ),
            Portal::new([250.0, 0.0, 150.0], normal, up, (32.0, 54.0)),
        ];
        for crouched in [false, true] {
            let mut player = Walker::at([350.0, 0.0, 0.0]);
            player.duck.ducked = crouched;
            let mut objects = Objects::default();
            objects.cube = RigidCube::at([100.0, 0.0, 30.0]);
            objects.cube.velocity = [0.0, 0.0, -200.0];
            for i in 0..20 {
                objects.advance(0.015, &player, &room(), Some(pair), None, i as f32 * 0.015);
            }
            assert!(
                objects.cube.position[0] > 200.0,
                "exit={normal:?} cube={:?}",
                objects.cube.position
            );
            assert!(objects.cube.position.iter().all(|v| v.is_finite()));
        }
    }
}

#[test]
fn oriented_cube_has_physical_player_collision() {
    let chamber = room();
    let mut space = Space::new(&chamber, None);
    space.add_object([100.0, 0.0, 20.0], IDENTITY, 20.0);
    let hit = space.trace(
        None,
        [40.0, 0.0, 0.0],
        [120.0, 0.0, 0.0],
        [-16.0, -16.0, 0.0],
        [16.0, 16.0, 72.0],
    );
    assert!(hit.fraction < 1.0);
    assert!((hit.end[0] - 63.96875).abs() < 0.001);
}

#[test]
fn standing_on_the_cube_prevents_picking_it_up() {
    let mut objects = Objects::default();
    objects.cube = RigidCube::at([100.0, 0.0, 20.0]);
    let mut player = Walker::at([100.0, 0.0, 40.0]);
    player.pitch = 89.0;
    player.ground = Some(usize::MAX - 1);
    assert!(!objects.use_object(&player, &Space::new(&room(), None), 0.0));
}

#[test]
fn a_released_cube_collides_with_the_player() {
    let mut objects = Objects::default();
    objects.cube = RigidCube::at([100.0, 0.0, 40.0]);
    objects.cube.velocity = [200.0, 0.0, 0.0];
    let player = Walker::at([180.0, 0.0, 0.0]);
    for i in 0..60 {
        objects.advance(0.015, &player, &room(), None, None, i as f32 * 0.015);
    }
    assert!(
        objects.cube.position[0] < 145.0,
        "{:?}",
        objects.cube.position
    );
}

#[test]
fn walking_pushes_a_cube_but_not_the_one_supporting_the_player() {
    let mut objects = Objects::default();
    objects.cube = RigidCube::at([100.0, 0.0, 20.0]);
    let mut player = Walker::at([63.9, 0.0, 0.0]);
    player.yaw = 0.0;
    objects.push(&player, 1.0, 0.0);
    assert!(objects.cube.velocity[0] > 0.0);
    objects.cube.velocity = [0.0; 3];
    player.ground = Some(usize::MAX - 1);
    objects.push(&player, 1.0, 0.0);
    assert_eq!(objects.cube.velocity, [0.0; 3]);
}

#[test]
fn rotated_cube_clears_a_brush_corner_without_a_phantom_overlap() {
    let chamber = Chamber {
        blocks: vec![block([-100.0, -100.0, -100.0], [0.0, 0.0, 100.0])],
    };
    let space = Space::new(&chamber, None);
    let basis = IDENTITY.map(|v| portal::carry::yaw_rotate(v, 45.0));
    let p = [20.0, 20.0, 0.0];
    let trace = space.trace_oriented(None, p, p, basis, 20.0);
    assert!(!trace.start_solid);
    let p = [10.0, 10.0, 0.0];
    assert!(space.trace_oriented(None, p, p, basis, 20.0).start_solid);
}

#[test]
fn released_cube_overlap_at_a_portal_rim_keeps_the_player_inside_the_room() {
    let dir = portal::install::root(
        None,
        None,
        std::path::Path::new(&std::env::var("HOME").unwrap()),
    );
    let install = portal::install::Install::open(&dir).unwrap();
    let chamber = Chamber::test_chamber(&install.search).unwrap();
    let pair = [
        Portal::new(
            [-384.0, -120.00001, 64.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [-320.00003, 256.0, 64.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
    ];
    let mut objects = Objects::default();
    objects.cube = RigidCube::at([-220.0, -80.0, 20.03125]);
    let mut player = Walker::at([-257.9, -95.17312, 0.03125]);
    player.yaw = 21.8014;
    player.pitch = 22.36;
    assert!(objects.use_object(&player, &Space::new(&chamber, Some(&pair)), 2.7));
    for tick in 0..280 {
        let now = 2.7 + tick as f32 * 0.015;
        if tick == 40 {
            objects.release(now);
        }
        if tick == 93 {
            player.pitch = 32.36;
            assert!(objects.use_object(&player, &Space::new(&chamber, Some(&pair)), now));
        }
        if tick == 140 {
            player.yaw = 180.0;
            player.pitch = 0.0;
        }
        let input = portalgun::walker::Input {
            forward: if (5.11..6.3).contains(&now) { 1.0 } else { 0.0 },
            ..Default::default()
        };
        objects.push(&player, input.forward, 0.0);
        let crossed = player.step_with_object(
            &input,
            0.015,
            &chamber,
            Some(pair),
            (!objects.holding()).then_some(&objects.cube),
        );
        objects.advance(0.015, &player, &chamber, Some(pair), crossed, now);
        assert!(
            player.feet[2] >= -0.001,
            "tick={tick} feet={:?}",
            player.feet
        );
    }
}

#[test]
fn object_overlap_waits_when_every_separation_path_crosses_static_solids() {
    let chamber = Chamber {
        blocks: vec![
            block([-64.0, -64.0, -16.0], [64.0, 64.0, 0.0]),
            block([-64.0, -64.0, 73.0], [64.0, 64.0, 89.0]),
            block([-32.0, -64.0, 0.0], [-16.0, 64.0, 73.0]),
            block([16.0, -64.0, 0.0], [32.0, 64.0, 73.0]),
            block([-64.0, -32.0, 0.0], [64.0, -16.0, 73.0]),
            block([-64.0, 16.0, 0.0], [64.0, 32.0, 73.0]),
        ],
    };
    let mut player = Walker::at([0.0, 0.0, 0.03125]);
    let cube = RigidCube::at([0.0, 0.0, 40.0]);
    let start = player.feet;
    for _ in 0..30 {
        player.step_with_object(
            &portalgun::walker::Input {
                forward: 1.0,
                ..Default::default()
            },
            0.015,
            &chamber,
            None,
            Some(&cube),
        );
        assert_eq!(player.feet, start);
    }
}

#[test]
fn releasing_caps_linear_and_angular_speed_without_accelerating_the_cube() {
    for speed in [0.0, 284.999, 285.0, 285.001, 1000.0] {
        let mut objects = Objects::default();
        objects.cube.velocity = [speed * 0.6, speed * 0.8, 0.0];
        objects.release(0.0);
        let limit = speed.min(285.0);
        assert!((objects.cube.velocity[0] - limit * 0.6).abs() < 0.0001);
        assert!((objects.cube.velocity[1] - limit * 0.8).abs() < 0.0001);
    }
    for speed in [0.0, 719.999, 720.0, 720.001, 3600.0] {
        let mut objects = Objects::default();
        objects.cube.angular_velocity = [0.0, 0.0, -speed];
        objects.release(0.0);
        assert!((objects.cube.angular_velocity[2] + speed.min(720.0)).abs() < 0.0001);
    }
}

#[test]
fn a_tilted_cube_teleported_onto_a_wall_portal_rim_does_not_freeze() {
    let pair = [
        Portal::new(
            [0.0, 0.0, 120.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            (32.0, 54.0),
        ),
        Portal::new(
            [300.0, 250.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            (32.0, 54.0),
        ),
    ];
    let player = Walker::at([400.0, -400.0, 0.0]);
    let mut objects = Objects::default();
    objects.advance(0.015, &player, &room(), Some(pair), None, 0.0);
    objects.cube = RigidCube::at([0.21611622, 4.719696, 90.92911]);
    objects.cube.rotation = [
        [0.24367645, 0.00083635125, -0.96985507],
        [-0.50043225, 0.8567059, -0.124995485],
        [0.830777, 0.51580584, 0.20917593],
    ];
    objects.cube.owner = Some(0);
    let start = objects.cube.position;
    for i in 1..60 {
        objects.advance(0.015, &player, &room(), Some(pair), None, i as f32 * 0.015);
    }
    let moved = (0..3)
        .map(|i| (objects.cube.position[i] - start[i]).powi(2))
        .sum::<f32>()
        .sqrt();
    assert!(
        moved > 10.0 || objects.visual_revision > 0,
        "cube froze at {:?} moved={moved}",
        objects.cube.position
    );
}

fn xorshift(s: &mut u64) -> f32 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    (*s >> 40) as f32 / (1u64 << 24) as f32
}

#[test]
fn random_drops_never_dip_into_the_floor_and_all_come_to_rest() {
    let mut seed = 0x9e3779b97f4a7c15u64;
    let player = Walker::at([-400.0, 400.0, 0.0]);
    for run in 0..200 {
        let mut objects = Objects::default();
        objects.cube = RigidCube::at([150.0, 0.0, 60.0 + xorshift(&mut seed) * 200.0]);
        objects.cube.velocity = [
            xorshift(&mut seed) * 400.0 - 200.0,
            xorshift(&mut seed) * 400.0 - 200.0,
            0.0,
        ];
        objects.cube.angular_velocity = [0, 1, 2].map(|_| xorshift(&mut seed) * 720.0 - 360.0);
        let mut deepest = f32::MAX;
        for i in 0..400 {
            objects.advance(0.015, &player, &room(), None, None, i as f32 * 0.015);
            deepest = deepest.min(objects.cube.position[2] - objects.cube.half()[2]);
        }
        assert!(deepest > -0.5, "run {run} dipped to {deepest}");
        assert_eq!(objects.cube.velocity, [0.0; 3], "run {run} still moving");
    }
}
