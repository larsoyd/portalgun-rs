// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::process::Command;

#[test]
fn startup_refusals_and_effective_settings_are_saved_to_the_log() {
    let dir = std::env::temp_dir().join(format!("portalgun-log-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let settings = dir.join("settings.toml");
    let log = dir.join("session.log");
    std::fs::write(&settings, "sensitivity = 1.25\nfullscreen = false").unwrap();
    let output = Command::new("timeout")
        .args(["20", env!("CARGO_BIN_EXE_portalgun"), "--log-file"])
        .arg(&log)
        .arg("--settings")
        .arg(&settings)
        .arg("--install")
        .arg(dir.join("missing-install"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let text = std::fs::read_to_string(&log).expect("startup failure must leave a diagnostic log");
    assert!(
        text.contains("[startup] version=")
            && text.contains("sensitivity: 1.25")
            && text.contains("[startup] refused:"),
        "{text}"
    );
    assert!(!text.contains("\u{1b}["), "log contains terminal escapes");
    std::fs::write(&settings, "fov = NaN").unwrap();
    let output = Command::new("timeout")
        .args(["20", env!("CARGO_BIN_EXE_portalgun"), "--log-file"])
        .arg(&log)
        .arg("--settings")
        .arg(&settings)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(
        text.contains("fov = NaN") && text.contains("[startup] refused:"),
        "{text}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn repeated_faults_report_once_and_can_report_again_after_recovery() {
    let mut faults = portalgun::diagnostics::Faults::default();
    assert!(faults.raised("bounds", true));
    for _ in 0..1000 {
        assert!(!faults.raised("bounds", true));
    }
    assert!(faults.raised("camera", true));
    assert!(!faults.raised("bounds", false));
    assert!(!faults.raised("camera", true));
    assert!(faults.raised("bounds", true));
}

#[test]
fn long_logs_rotate_and_keep_the_previous_chunk() {
    use bevy::log::tracing_subscriber::fmt::MakeWriter;
    use std::io::Write;
    let path = std::env::temp_dir().join(format!("portalgun-rotation-{}.log", std::process::id()));
    let previous = std::path::PathBuf::from(format!("{}.previous", path.display()));
    let file = portalgun::diagnostics::LogFile::open(&path).unwrap();
    let block = vec![b'x'; 1024 * 1024];
    for _ in 0..9 {
        file.make_writer().write_all(&block).unwrap();
    }
    file.make_writer().write_all(b"last event\n").unwrap();
    file.flush().unwrap();
    assert_eq!(std::fs::metadata(&previous).unwrap().len(), 8 * 1024 * 1024);
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .ends_with("last event\n"));
    drop(file);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(previous).unwrap();
}

#[test]
fn movement_diagnostics_name_nonfinite_fields_without_changing_the_player() {
    use portalgun::diagnostics::invalid_player;
    let mut player = portalgun::walker::Walker::at([0.0; 3]);
    assert_eq!(invalid_player(&player), None);
    player.velocity[1] = f32::INFINITY;
    assert_eq!(invalid_player(&player), Some("velocity"));
    assert!(player.velocity[1].is_infinite());
    player.velocity[1] = 0.0;
    player.feet[2] = f32::NAN;
    assert_eq!(invalid_player(&player), Some("feet"));
    player.feet[2] = 0.0;
    player.duck.eye = f32::NAN;
    assert_eq!(invalid_player(&player), Some("view"));
}
