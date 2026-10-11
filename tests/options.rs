// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portalgun::options::{Improvement, Options, Page};
use portalgun::settings::Settings;

#[test]
fn escape_hides_options_without_applying_or_discarding_the_draft() {
    let settings = Settings::parse("").unwrap();
    let mut menu = Options::default();
    menu.escape();
    assert_eq!(menu.page, Page::Pause);
    menu.open(&settings);
    Improvement::Feet.toggle(menu.draft.as_mut().unwrap());
    assert!(settings.original_feet);
    menu.escape();
    assert_eq!(menu.page, Page::Closed);
    assert!(!menu.draft.as_ref().unwrap().original_feet);
    menu.escape();
    assert_eq!(menu.page, Page::Options);
    assert!(!menu.draft.as_ref().unwrap().original_feet);
    menu.cancel();
    assert_eq!(menu.page, Page::Pause);
    assert!(menu.draft.is_none());
    menu.escape();
    assert_eq!(menu.page, Page::Closed);
}

#[test]
fn improvement_checkboxes_are_off_and_independent() {
    let mut settings = Settings::parse("").unwrap();
    for improvement in Improvement::ALL {
        assert!(!improvement.enabled(&settings));
        improvement.toggle(&mut settings);
        for other in Improvement::ALL {
            assert_eq!(other.enabled(&settings), other == improvement);
        }
        improvement.toggle(&mut settings);
    }
}

#[test]
fn apply_saves_all_choices_and_cancel_only_discards_unapplied_changes() {
    let path = std::env::temp_dir().join(format!("portal-options-{}.toml", std::process::id()));
    let mut settings = Settings::parse(
        "fov = 100\nsensitivity = 1.25\nfullscreen = false\nmsaa = 8\nvsync = true",
    )
    .unwrap();
    let mut menu = Options::default();
    menu.open(&settings);
    for improvement in Improvement::ALL {
        improvement.toggle(menu.draft.as_mut().unwrap());
    }
    menu.apply(&mut settings, &path).unwrap();
    assert_eq!(menu.page, Page::Options);
    assert_eq!(
        Settings::parse(&std::fs::read_to_string(&path).unwrap()).unwrap(),
        settings
    );
    for improvement in Improvement::ALL {
        assert!(improvement.enabled(&settings));
    }
    menu.restore_original();
    assert_eq!(menu.draft.as_ref().unwrap().fov, 100.0);
    for improvement in Improvement::ALL {
        assert!(!improvement.enabled(menu.draft.as_ref().unwrap()));
    }
    menu.cancel();
    assert!(Improvement::Feet.enabled(&settings));
    menu.open(&settings);
    menu.restore_original();
    menu.apply(&mut settings, &path).unwrap();
    for improvement in Improvement::ALL {
        assert!(!improvement.enabled(&settings));
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn failed_save_keeps_active_settings_and_the_draft() {
    let mut settings = Settings::parse("").unwrap();
    let before = settings.clone();
    let mut menu = Options::default();
    menu.open(&settings);
    Improvement::GunLag.toggle(menu.draft.as_mut().unwrap());
    let path = std::env::temp_dir().join(format!(
        "portal-missing-{}/settings.toml",
        std::process::id()
    ));
    assert!(menu.apply(&mut settings, &path).is_err());
    assert_eq!(settings, before);
    assert_eq!(menu.page, Page::Options);
    assert!(Improvement::GunLag.enabled(menu.draft.as_ref().unwrap()));
}
