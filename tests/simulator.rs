// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::pair::Portal;
use portal::placement::Surface;
use portalgun::chamber::{Block, Chamber};
use portalgun::simulator::Space;
use portalgun::walker::{Input, Walker};

const DT: f32 = 0.015;

fn block(min: [f32; 3], max: [f32; 3]) -> Block {
    Block::cuboid(min, max, "", Surface::Portalable, "default")
}

/// A floor at z 0 and a wall whose face is x 500.
fn room() -> Chamber {
    Chamber {
        blocks: vec![
            block([-1000.0, -1000.0, -64.0], [1000.0, 1000.0, 0.0]),
            block([500.0, -1000.0, 0.0], [564.0, 1000.0, 400.0]),
        ],
        ..Default::default()
    }
}

/// Floor portal at the origin, its up along x, so world x is its local z and world y its local y.
fn floor_portal() -> Portal {
    Portal::new([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], (32.0, 54.0))
}

fn wall_portal() -> Portal {
    Portal::new(
        [500.0, 0.0, 54.0],
        [-1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    )
}

fn idle(w: &mut Walker, c: &Chamber, pair: [Portal; 2], ticks: usize) -> Option<usize> {
    (0..ticks).find_map(|_| w.step(&Input::default(), DT, c, Some(pair)))
}

#[test]
fn standing_on_a_floor_portal_drops_through_only_inside_the_inner_tube() {
    let c = room();
    let pair = [floor_portal(), wall_portal()];
    for (at, falls) in [
        ([0.0, 16.05, 0.0], true),
        ([0.0, -16.05, 0.0], true),
        ([0.0, 16.15, 0.0], false),
        ([38.05, 0.0, 0.0], true),
        ([38.15, 0.0, 0.0], false),
        ([-38.15, 0.0, 0.0], false),
    ] {
        let mut w = Walker::at(at);
        let crossed = idle(&mut w, &c, pair, 60);
        assert_eq!(
            crossed.is_some(),
            falls,
            "standing at {at:?} ended at {:?}",
            w.feet
        );
        if !falls {
            assert!(
                w.grounded && (-0.011..=0.0).contains(&w.feet[2]),
                "rests on the tube or carved floor: {:?}",
                w.feet
            );
            assert_eq!(
                w.ground,
                Some(0),
                "standing on the floor's own surface at {at:?}"
            );
        }
    }
}

#[test]
fn a_fling_into_a_wall_portal_at_the_max_velocity_crosses_without_losing_speed() {
    let c = room();
    let exit = Portal::new(
        [0.0, 300.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        (32.0, 54.0),
    );
    let pair = [wall_portal(), exit];
    for start in [300.0, 320.0, 340.0, 360.0, 380.0] {
        let mut w = Walker::at([start, 0.0, 0.0]);
        w.velocity = [3500.0, 0.0, 0.0];
        let crossed = (0..20).find_map(|_| w.step(&Input::default(), DT, &c, Some(pair)));
        assert_eq!(crossed, Some(0), "from x {start} ended at {:?}", w.feet);
        let speed = w.velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!(
            speed > 990.0,
            "the exit keeps the capped speed from x {start}: {:?}",
            w.velocity
        );
    }
}

fn real_solid(w: &Walker, c: &Chamber) -> bool {
    let solids: Vec<_> = c
        .blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (i, b.min, b.max))
        .collect();
    let (lo, hi) = ([-16.0, -16.0, 0.0], [16.0, 16.0, w.height()]);
    portalgun::collision::trace(w.feet, w.feet, lo, hi, &solids).start_solid
}

#[test]
fn geometry_in_front_of_the_exit_stops_the_player_at_the_entrance() {
    let mut c = room();
    c.blocks
        .push(block([-40.0, 290.0, 0.0], [40.0, 310.0, 20.0]));
    let exit = Portal::new(
        [0.0, 300.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        (32.0, 54.0),
    );
    let pair = [wall_portal(), exit];
    let mut w = Walker::at([470.0, 0.0, 0.0]);
    w.velocity = [300.0, 0.0, 0.0];
    let crossed = idle(&mut w, &c, pair, 40);
    assert_eq!(
        crossed, None,
        "the exit's own front world blocks the way in: {:?}",
        w.feet
    );
    assert!(
        w.feet[0] < 484.1 && w.velocity[0] == 0.0,
        "stopped with the hull at the wall's face: {:?} {:?}",
        w.feet,
        w.velocity
    );
}

#[test]
fn a_player_in_solid_beside_a_portal_is_freed_by_the_passable_space_search() {
    let mut c = room();
    c.blocks.push(block([470.0, -5.0, 0.0], [475.0, 5.0, 40.0]));
    let exit = Portal::new(
        [0.0, 300.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        (32.0, 54.0),
    );
    let pair = [wall_portal(), exit];
    let mut w = Walker::at([480.0, 0.0, 0.0]);
    assert!(real_solid(&w, &c), "the setup starts stuck");
    idle(&mut w, &c, pair, 40);
    assert_eq!(w.owner, Some(0), "the wall portal holds the player");
    assert!(!real_solid(&w, &c), "freed: {:?}", w.feet);
    let moved = ((w.feet[0] - 480.0).powi(2) + w.feet[1].powi(2)).sqrt();
    assert!(moved < 30.0, "to nearby open space: {:?}", w.feet);
    assert!(
        (w.feet[0] - 464.97).abs() < 0.05
            && w.feet[1].abs() < 0.01
            && (w.feet[2] - 40.03125).abs() < 1e-3,
        "the corner weighted search, shrinking by a 101st each try, lands back and on top: {:?}",
        w.feet
    );
}

#[test]
fn a_ducked_player_behind_a_ceiling_portal_keeps_it_on_leaving_its_box() {
    let mut c = room();
    c.blocks
        .push(block([-200.0, -200.0, 300.0], [200.0, 200.0, 364.0]));
    let ceiling = Portal::new(
        [0.0, 0.0, 300.0],
        [0.0, 0.0, -1.0],
        [1.0, 0.0, 0.0],
        (32.0, 54.0),
    );
    let pair = [ceiling, wall_portal()];
    let mut w = Walker::at([0.0, 0.0, 200.0]);
    w.step(&Input::default(), DT, &c, Some(pair));
    assert_eq!(w.owner, Some(0), "taken while below the ceiling portal");
    w.feet = [0.0, 60.0, 302.0];
    w.duck.ducked = true;
    w.duck.in_duck_jump = true;
    w.step(&Input::default(), DT, &c, Some(pair));
    assert_eq!(
        w.owner,
        Some(0),
        "kept while ducked behind its plane: {:?}",
        w.feet
    );
}

#[test]
fn a_portal_cannot_take_a_player_held_by_a_nearer_one() {
    let c = room();
    let near = wall_portal();
    let mut far = wall_portal();
    far.origin[1] = 70.0;
    let mut w = Walker::at([470.0, 30.0, 0.0]);
    w.step(&Input::default(), DT, &c, Some([near, far]));
    assert_eq!(
        w.owner,
        Some(0),
        "both boxes touch, the nearer portal keeps the player"
    );
}

const HULL: ([f32; 3], [f32; 3]) = ([-16.0, -16.0, 0.0], [16.0, 16.0, 20.0]);

fn floor_space() -> Space {
    Space::new(&room(), Some(&[floor_portal(), wall_portal()]))
}

fn solid_at(space: &Space, at: [f32; 3]) -> bool {
    space.trace(Some(0), at, at, HULL.0, HULL.1).start_solid
}

#[test]
fn deeper_than_the_tube_the_wall_is_cut_wider() {
    let space = floor_space();
    assert!(
        !solid_at(&space, [0.0, 16.15, -30.0]),
        "32.15 fits the 32.2 cut"
    );
    assert!(solid_at(&space, [0.0, 16.25, -30.0]), "32.25 does not");
}

#[test]
fn the_carved_wall_starts_a_hundredth_behind_the_plane() {
    let space = floor_space();
    assert!(!solid_at(&space, [0.0, 60.0, -0.005]));
    assert!(solid_at(&space, [0.0, 60.0, -0.02]));
}

#[test]
fn a_hull_starting_in_the_tube_is_not_held_by_it() {
    let space = floor_space();
    let at = [0.0, 16.15, -0.5];
    let up = space.trace(Some(0), at, [at[0], at[1], 10.0], HULL.0, HULL.1);
    assert!(!up.start_solid && up.fraction == 1.0, "{up:?}");
}

/// Two portals in open space, each with a block where the other's lands when carried through.
fn far_pair() -> (Chamber, [Portal; 2]) {
    let entry = Portal::new([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], (32.0, 54.0));
    let exit = Portal::new(
        [1000.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        (32.0, 54.0),
    );
    let c = Chamber {
        blocks: vec![
            block([-170.0, -100.0, -20.0], [-130.0, -60.0, 20.0]),
            block([1130.0, 60.0, -20.0], [1170.0, 100.0, 20.0]),
        ],
        ..Default::default()
    };
    (c, [entry, exit])
}

#[test]
fn the_far_side_only_collides_through_the_hole() {
    let (c, pair) = far_pair();
    let space = Space::new(&c, Some(&pair));
    let half = ([-16.0; 3], [16.0; 3]);
    let at = [-150.0, -80.0, 0.0];
    let tr = space.trace(Some(0), at, at, half.0, half.1);
    assert!(
        !tr.start_solid,
        "beside the hole the carried block is not traced: {tr:?}"
    );
}

#[test]
fn a_tie_between_the_real_and_carried_world_keeps_the_real_hit() {
    let (_, pair) = far_pair();
    // the real block meets the hull's side and the carried one its top, both halfway along
    let c = Chamber {
        blocks: vec![
            block([-170.0, -100.0, -20.0], [-130.0, -61.0, 80.0]),
            block([1130.0, 20.0, 31.0], [1170.0, 40.0, 60.0]),
        ],
        ..Default::default()
    };
    let space = Space::new(&c, Some(&pair));
    let half = ([-16.0; 3], [16.0; 3]);
    let tr = space.trace(
        Some(0),
        [-150.0, -30.0, 0.0],
        [-150.0, -60.0, 30.0],
        half.0,
        half.1,
    );
    assert!((tr.fraction - 0.5).abs() < 0.01, "{tr:?}");
    assert_eq!(
        tr.normal,
        [0.0, 1.0, 0.0],
        "the real block's face, not the carried top: {tr:?}"
    );
}

#[test]
fn a_player_in_solid_behind_the_plane_beside_the_hole_is_taken_and_freed() {
    let c = room();
    let exit = Portal::new(
        [0.0, 300.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
        (32.0, 54.0),
    );
    let pair = [wall_portal(), exit];
    // centre 5 into the wall, 89.5 above the portal's centre: inside its box, outside its hole
    let mut w = Walker::at([505.0, 0.0, 54.0 + 89.5 - 36.0]);
    assert!(real_solid(&w, &c));
    idle(&mut w, &c, pair, 40);
    assert_eq!(w.owner, Some(0), "the portal took the stuck player");
    assert!(!real_solid(&w, &c), "freed: {:?}", w.feet);
}

#[test]
fn a_steep_look_into_a_floor_portal_with_a_ceiling_exit_sets_pitch_reorientation() {
    let mut c = room();
    c.blocks
        .push(block([-200.0, 200.0, 300.0], [200.0, 600.0, 364.0]));
    let ceiling = Portal::new(
        [0.0, 400.0, 300.0],
        [0.0, 0.0, -1.0],
        [1.0, 0.0, 0.0],
        (32.0, 54.0),
    );
    for (pitch, expected) in [(80.0, true), (0.0, false)] {
        let mut w = Walker::at([0.0, 0.0, 100.0]);
        w.pitch = pitch;
        let crossed = idle(&mut w, &c, [floor_portal(), ceiling], 100);
        assert_eq!(crossed, Some(0));
        assert_eq!(w.pitch_reorientation, expected, "pitch {pitch}");
    }
}

#[test]
fn the_roll_recovery_pitches_while_the_flag_is_set() {
    let rolled = |flag: bool| {
        let mut w = Walker::at([0.0; 3]);
        w.roll = 90.0;
        w.pitch_reorientation = flag;
        w.fix_view(0.1, false);
        w.roll
    };
    assert!(rolled(false) < 90.0, "without the flag the roll eases back");
    assert_ne!(
        rolled(true),
        rolled(false),
        "with it the view pitches about its right axis instead"
    );
}

#[test]
fn a_hull_that_lands_behind_the_exit_plane_right_after_crossing_is_sent_back() {
    let mut c = room();
    c.blocks
        .push(block([-200.0, 200.0, 300.0], [200.0, 600.0, 364.0]));
    let ceiling = Portal::new(
        [0.0, 400.0, 300.0],
        [0.0, 0.0, -1.0],
        [1.0, 0.0, 0.0],
        (32.0, 54.0),
    );
    let pair = [floor_portal(), ceiling];
    let mut w = Walker::at([0.0, 0.0, 30.0]);
    assert_eq!(idle(&mut w, &c, pair, 100), Some(0));
    // the exit's box must already know the player, so leaving it behind the plane sends it back
    w.feet = [0.0, 400.0, 310.0];
    w.velocity = [0.0; 3];
    assert_eq!(w.step(&Input::default(), DT, &c, Some(pair)), Some(1));
}

#[test]
fn an_owned_object_meets_every_portal_piece_in_the_surface_behind_that_portal() {
    let pair = [floor_portal(), wall_portal()];
    let space = Space::new(&room(), Some(&pair));
    let pieces = space.object_solids(Some(0), true);
    let linked = |id: usize| (id >> 20) & 15 == 3;
    assert!(pieces.iter().any(|p| linked(p.0)), "no far side pieces");
    for (id, surface, _) in &pieces {
        let want = if linked(*id) { 1 } else { 0 };
        assert_eq!(*surface, want, "piece {id:#x}");
    }
    let mut raised = pair;
    raised[0].origin[2] = 100.0;
    let space = Space::new(&room(), Some(&raised));
    for (id, surface, _) in space.object_solids(Some(0), false) {
        assert_eq!(
            surface, 0,
            "piece {id:#x} should take the floor 100 units behind"
        );
    }
    let mut lost = pair;
    lost[0].origin[2] = 900.0;
    let space = Space::new(&room(), Some(&lost));
    for (id, surface, _) in space.object_solids(Some(0), false) {
        assert_eq!(
            surface,
            usize::MAX,
            "piece {id:#x} with nothing behind the portal"
        );
    }
}
