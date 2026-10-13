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
fn cube_settles_on_the_floor_without_sinking() {
    let mut objects = Objects::default();
    objects.cube = RigidCube::at([100.0, 0.0, 100.0]);
    let player = Walker::at([300.0, 0.0, 0.0]);
    let chamber = room();
    for i in 0..600 {
        objects.advance(0.015, &player, &chamber, None, None, i as f32 * 0.015);
    }
    assert!(
        objects.cube.position[2] >= 20.0,
        "{:?}",
        objects.cube.position
    );
    assert!(
        objects.cube.position[2] < 20.1,
        "{:?}",
        objects.cube.position
    );
    assert!(objects.cube.velocity[2].abs() < 0.01);
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
