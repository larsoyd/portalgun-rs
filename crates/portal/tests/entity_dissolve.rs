// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::entity::dissolve::colour;

#[test]
fn a_dissolve_tints_from_white_to_black_over_its_first_second() {
    assert_eq!(colour(-0.01), [255, 255, 255, 255]);
    assert_eq!(colour(0.25), [191, 191, 191, 255]);
    assert_eq!(colour(0.5), [127, 127, 127, 255]);
    assert_eq!(colour(0.99), [2, 2, 2, 255]);
    assert_eq!(colour(1.0), [0, 0, 0, 255]);
    assert_eq!(colour(1.5), [0, 0, 0, 255]);
}

#[test]
fn the_tint_is_black_at_the_exact_start_time() {
    assert_eq!(colour(0.0), [0, 0, 0, 255]);
}

#[test]
fn the_model_fades_out_in_its_last_tenth_of_a_second() {
    assert_eq!(colour(1.9)[3], 255);
    assert_eq!(colour(1.925)[3], 191);
    assert_eq!(colour(1.975)[3], 63);
    assert_eq!(colour(1.99)[3], 25);
    assert_eq!(colour(2.01), [0, 0, 0, 0]);
    assert_eq!(colour(2.5), [0, 0, 0, 0]);
}
