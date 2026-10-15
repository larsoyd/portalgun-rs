// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::PathBuf;

use portalgun::args::Args;

fn parse(line: &str) -> Result<Args, String> {
    Args::parse(line.split_whitespace().map(String::from))
}

#[test]
fn nothing_given_means_no_script_no_shots_no_quit() {
    let a = parse("").unwrap();
    assert_eq!(a, Args::default());
    assert!(
        a.install.is_none() && a.script.is_none() && a.shots.is_empty() && a.quit_after.is_none()
    );
}

#[test]
fn every_flag_lands_where_it_should() {
    let a = parse(
        "--install /p --script s.txt --shot-at 1.5 --shot-at 3 --shot-dir out --quit-after 4",
    )
    .unwrap();
    assert_eq!(a.install.as_deref(), Some("/p"));
    assert_eq!(a.script, Some(PathBuf::from("s.txt")));
    assert_eq!(a.shots, vec![1.5, 3.0]);
    assert_eq!(a.shot_dir, PathBuf::from("out"));
    assert_eq!(a.quit_after, Some(4.0));
}

#[test]
fn shots_default_to_the_current_directory() {
    assert_eq!(parse("--shot-at 2").unwrap().shot_dir, PathBuf::from("."));
}

#[test]
fn bad_input_is_refused_with_the_flag_named() {
    for (line, needle) in [
        ("--shot-at soon", "--shot-at"),
        ("--quit-after", "--quit-after"),
        ("--quit-after -1", "--quit-after"),
        ("--fly", "--fly"),
        ("--shot-at 5 --quit-after 4", "after --quit-after"),
        ("--shot-at 1 --shot-at 5 --quit-after 4", "--shot-at 5"),
    ] {
        let e = parse(line).unwrap_err();
        assert!(e.contains(needle), "{line}: {e}");
    }
}

#[test]
fn a_shot_at_zero_is_fine_but_not_at_the_quit_itself() {
    assert_eq!(parse("--shot-at 0").unwrap().shots, vec![0.0]);
    assert!(parse("--shot-at 3.9 --quit-after 4").is_ok());
    assert!(parse("--shot-at 4 --quit-after 4").is_err());
    assert!(parse("--shot-at inf").is_err());
}

#[test]
fn settings_default_to_a_file_beside_where_it_runs() {
    assert_eq!(parse("").unwrap().settings, PathBuf::from("settings.toml"));
    assert_eq!(
        parse("--settings mine.toml").unwrap().settings,
        PathBuf::from("mine.toml")
    );
}

#[test]
fn the_chamber_defaults_to_the_sandbox_and_takes_a_name() {
    assert_eq!(parse("").unwrap().chamber, "sandbox");
    assert_eq!(parse("--chamber entities").unwrap().chamber, "entities");
}
