// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::placement::Surface;
use portalgun::chamber::{Block, Chamber};
use portalgun::walker::{Input, Walker};

fn compare(csv: &str) {
    let rows: Vec<Vec<f32>> = csv
        .lines()
        .skip(1)
        .map(|line| line.split(',').map(|v| v.parse().unwrap()).collect())
        .collect();
    let initial = &rows[0];
    let floor = initial[11] - 0.03125;
    let chamber = Chamber {
        blocks: vec![Block::cuboid(
            [-10000.0, -10000.0, floor - 16.0],
            [10000.0, 10000.0, floor],
            "",
            Surface::Portalable,
            "default",
        )],
    };
    let mut walker = Walker::at([initial[9], initial[10], initial[11]]);
    walker.step(&Input::default(), 0.015, &chamber, None);
    for row in &rows[1..] {
        let buttons = row[1] as u32;
        let input = Input {
            jump: buttons & 2 != 0,
            crouch: buttons & 4 != 0,
            forward: if buttons & 8 != 0 {
                1.0
            } else if buttons & 16 != 0 {
                -1.0
            } else {
                0.0
            },
            ..Default::default()
        };
        walker.step(&input, 0.015, &chamber, None);
        for i in 0..3 {
            assert!(
                (walker.feet[i] - row[9 + i]).abs() < 0.003,
                "tick {} axis{i}: position {} expected {}",
                row[0],
                walker.feet[i],
                row[9 + i]
            );
            assert!(
                (walker.velocity[i] - row[12 + i]).abs() < 0.003,
                "tick {} axis{i}: velocity {} expected {}",
                row[0],
                walker.velocity[i],
                row[12 + i]
            );
        }
        assert_eq!(walker.duck.ducked, row[4] != 0.0, "tick {} ducked", row[0]);
        assert_eq!(
            walker.duck.ducking,
            row[5] != 0.0,
            "tick {} transition",
            row[0]
        );
        assert_eq!(
            walker.duck.in_duck_jump,
            row[6] != 0.0,
            "tick {} duck jump",
            row[0]
        );
        assert_eq!(
            walker.grounded,
            (row[15] as u32) & 1 != 0,
            "tick {} ground",
            row[0]
        );
    }
}

#[test]
fn standing_jump_matches_recorded_original_positions_velocity_and_states() {
    compare(include_str!("fixtures/standing-jump.csv"));
}
#[test]
fn acceleration_matches_recorded_original_each_tick() {
    compare(include_str!("fixtures/acceleration.csv"));
}
#[test]
fn crouch_jump_and_release_match_recorded_original_each_tick() {
    compare(include_str!("fixtures/crouch-jump.csv"));
}
#[test]
fn duck_walking_braking_and_unducking_match_recorded_original_each_tick() {
    compare(include_str!("fixtures/duck-walk.csv"));
}
