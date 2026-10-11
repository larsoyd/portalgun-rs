// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portalgun::settings::Settings;

#[test]
fn an_empty_file_gives_portals_own_defaults() {
    let s = Settings::parse("").unwrap();
    assert_eq!((s.sensitivity, s.fov, s.msaa), (3.0, 75.0, 4));
    assert!(s.fullscreen);
    assert!(!s.vsync);
}

#[test]
fn display_settings_allow_windowed_vsync_and_reject_invalid_booleans() {
    let s = Settings::parse("fullscreen = false\nvsync = true").unwrap();
    assert!(!s.fullscreen);
    assert!(s.vsync);
    assert!(Settings::parse("fullscreen = maybe").is_err());
    assert!(Settings::parse("vsync = 180").is_err());
}

#[test]
fn keys_override_and_comments_are_skipped() {
    let s = Settings::parse("# mine\nsensitivity = 1.5\nfov = 90\nmsaa = 1\n").unwrap();
    assert_eq!((s.sensitivity, s.fov, s.msaa), (1.5, 90.0, 1));
}

#[test]
fn fov_is_held_to_the_range_the_game_allows() {
    assert_eq!(Settings::parse("fov = 74.9").unwrap().fov, 75.0);
    assert_eq!(Settings::parse("fov = 120").unwrap().fov, 120.0);
    assert_eq!(Settings::parse("fov = 130").unwrap().fov, 120.0);
}

#[test]
fn unknown_keys_and_bad_values_are_refused_with_the_line() {
    let e = Settings::parse("fov = 90\nzoom = 2").unwrap_err();
    assert!(e.contains("line 2") && e.contains("zoom"), "{e}");
    let e = Settings::parse("sensitivity = fast").unwrap_err();
    assert!(e.contains("line 1") && e.contains("fast"), "{e}");
    let e = Settings::parse("msaa = 3").unwrap_err();
    assert!(e.contains("msaa"), "{e}");
}

#[test]
fn the_horizontal_fov_is_kept_at_four_by_three_so_wide_screens_see_more() {
    let s = Settings::parse("").unwrap();
    assert!(
        (s.vertical_fov().to_degrees() - 59.84).abs() < 0.01,
        "{}",
        s.vertical_fov().to_degrees()
    );
    let wide = Settings::parse("fov = 90").unwrap();
    assert!(
        (wide.vertical_fov().to_degrees() - 73.74).abs() < 0.01,
        "{}",
        wide.vertical_fov().to_degrees()
    );
}

#[test]
fn the_original_airwalk_pose_is_opt_in() {
    assert!(!Settings::parse("").unwrap().original_airwalk);
    assert!(
        Settings::parse("original_airwalk = true")
            .unwrap()
            .original_airwalk
    );
    assert!(Settings::parse("original_airwalk = 1").is_err());
}
