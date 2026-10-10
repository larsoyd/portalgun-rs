use std::path::Path;

use portal::install::root;
use portalgun::icon::{desktop_entry, read, write_desktop_entry};

fn install() -> std::path::PathBuf {
    root(None, None, Path::new(&std::env::var("HOME").unwrap()))
}

#[test]
fn the_icon_comes_from_the_installs_resource_folder() {
    let bmp = read(&install()).unwrap();
    assert_eq!((bmp.width, bmp.height), (48, 48));
    let e = read(Path::new("/nonexistent")).unwrap_err();
    assert!(e.contains("portal/resource/game-icon.bmp"), "{e}");
}

#[test]
fn the_desktop_entry_ties_the_app_id_to_the_icon() {
    let text = desktop_entry(Path::new("/d/portalgun.png"), Path::new("/x/portalgun"));
    for line in [
        "[Desktop Entry]",
        "Type=Application",
        "Name=portalgun",
        "Icon=/d/portalgun.png",
        "Exec=/x/portalgun",
        "StartupWMClass=portalgun",
        "NoDisplay=true",
    ] {
        assert!(text.lines().any(|l| l == line), "missing {line} in\n{text}");
    }
}

#[test]
fn the_entry_is_written_once_and_left_alone_when_unchanged() {
    let home = std::env::temp_dir().join(format!("portalgun-icon-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let bmp = read(&install()).unwrap();
    let exe = Path::new("/x/portalgun");
    assert!(write_desktop_entry(&home, &bmp, exe).unwrap());
    let png = home.join("icons/hicolor/48x48/apps/portalgun.png");
    let entry = home.join("applications/portalgun.desktop");
    assert!(std::fs::read(&png).unwrap().starts_with(b"\x89PNG"));
    let text = std::fs::read_to_string(&entry).unwrap();
    assert!(text.contains(&format!("Icon={}", png.display())), "{text}");
    assert!(!write_desktop_entry(&home, &bmp, exe).unwrap());
    assert!(write_desktop_entry(&home, &bmp, Path::new("/y/portalgun")).unwrap());
    std::fs::remove_dir_all(&home).unwrap();
}
