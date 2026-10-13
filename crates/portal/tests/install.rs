// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::{Path, PathBuf};

use portal::install::{manifest, root, Install};
use portal::Error;

fn steam() -> PathBuf {
    root(None, None, Path::new(&std::env::var("HOME").unwrap()))
}

fn copy_of_install(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("portal-install-{}-{name}", std::process::id()));
    for &(file, _) in manifest(&steam()) {
        let to = dir.join(file);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(steam().join(file), &to).unwrap();
    }
    dir
}

#[test]
fn root_takes_the_argument_then_the_variable_then_steam() {
    let home = Path::new("/h");
    let steam = Path::new("/h/.local/share/Steam/steamapps/common/Portal");
    assert_eq!(root(Some("/a"), Some("/e"), home), Path::new("/a"));
    assert_eq!(root(None, Some("/e"), home), Path::new("/e"));
    assert_eq!(root(None, None, home), steam);
}

#[test]
fn opens_the_users_install() {
    let install =
        Install::open(&steam()).unwrap_or_else(|e| panic!("install at {:?}: {e}", steam()));
    for (path, pak) in [
        ("models/weapons/v_portalgun.mdl", "portal_pak"),
        ("shaders/fxc/accumbuff4sample_ps20.vcs", "hl2_misc"),
        (
            "materials/models/weapons/v_stunstick/v_stunstick_diffuse.vtf",
            "hl2_textures",
        ),
    ] {
        assert!(install.search.contains(path), "{path} from {pak} not found");
    }
    assert_eq!(
        install.client.len(),
        std::fs::metadata(steam().join(manifest(&steam())[4].0))
            .unwrap()
            .len() as usize
    );
}

#[test]
fn one_flipped_byte_in_any_pinned_file_is_refused() {
    for (i, (file, _)) in manifest(&steam()).iter().enumerate() {
        let dir = copy_of_install(&format!("flip{i}"));
        let path = dir.join(file);
        let mut bytes = std::fs::read(&path).unwrap();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 1;
        std::fs::write(&path, bytes).unwrap();
        match Install::open(&dir) {
            Err(Error::Refused(reason)) => assert!(reason.contains(file), "{file}: {reason}"),
            other => panic!("{file}: expected a refusal, got {:?}", other.err()),
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn an_untouched_copy_opens() {
    let dir = copy_of_install("clean");
    Install::open(&dir).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_missing_pinned_file_is_refused_by_name() {
    let dir = copy_of_install("missing");
    std::fs::remove_file(dir.join("hl2/hl2_misc_dir.vpk")).unwrap();
    match Install::open(&dir) {
        Err(Error::Refused(reason)) => assert!(reason.contains("hl2/hl2_misc_dir.vpk"), "{reason}"),
        other => panic!("expected a refusal, got {:?}", other.err()),
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_search_path_reaches_step_sounds_and_loose_hl2_scripts() {
    let install =
        Install::open(&steam()).unwrap_or_else(|e| panic!("install at {:?}: {e}", steam()));
    for path in [
        "sound/player/footsteps/concrete1.wav",
        "scripts/game_sounds_physics.txt",
        "scripts/surfaceproperties.txt",
    ] {
        assert!(install.search.contains(path), "{path} not found");
    }
    assert!(!install.search.contains("scripts/no_such_file.txt"));
}
