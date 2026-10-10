use std::io::Cursor;
use std::path::Path;

use bevy::prelude::*;
use bevy::ecs::system::NonSendMarker;
use bevy::window::PrimaryWindow;
use bevy::winit::WINIT_WINDOWS;
use image::{ImageFormat, RgbaImage};
use portal::bmp::Bmp;

/// the engine's own window icon, relative to the install
const ICON: &str = "portal/resource/game-icon.bmp";
pub const APP_ID: &str = "portalgun";

pub fn read(install: &Path) -> Result<Bmp, String> {
    let path = install.join(ICON);
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Bmp::parse(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn desktop_entry(icon: &Path, exec: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName={APP_ID}\nIcon={}\nExec={}\nStartupWMClass={APP_ID}\nNoDisplay=true\n",
        icon.display(),
        exec.display()
    )
}

/// Wayland takes a window's icon from the desktop entry named after its app id.
pub fn write_desktop_entry(data_home: &Path, bmp: &Bmp, exec: &Path) -> Result<bool, String> {
    let png_path = data_home.join(format!("icons/hicolor/48x48/apps/{APP_ID}.png"));
    let entry_path = data_home.join(format!("applications/{APP_ID}.desktop"));
    let image = RgbaImage::from_raw(bmp.width, bmp.height, bmp.rgba.clone())
        .ok_or("icon pixels do not fill its size")?;
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .map_err(|e| format!("png: {e}"))?;
    let entry = desktop_entry(&png_path, exec);
    let mut changed = false;
    for (path, bytes) in [(&png_path, png.as_slice()), (&entry_path, entry.as_bytes())] {
        if std::fs::read(path).is_ok_and(|old| old == bytes) {
            continue;
        }
        let write = |p: &Path| {
            std::fs::create_dir_all(p.parent().unwrap_or(p))?;
            std::fs::write(p, bytes)
        };
        write(path).map_err(|e| format!("{}: {e}", path.display()))?;
        changed = true;
    }
    Ok(changed)
}

#[derive(Resource)]
pub struct GameIcon(pub Bmp);

/// X11 and Windows take the icon from the window itself, once winit has made it.
pub fn set_window_icon(
    icon: Res<GameIcon>,
    window: Single<Entity, With<PrimaryWindow>>,
    mut done: Local<bool>,
    _main_thread: NonSendMarker,
) {
    if *done {
        return;
    }
    WINIT_WINDOWS.with_borrow(|windows| {
        let Some(w) = windows.get_window(*window) else {
            return;
        };
        match winit::window::Icon::from_rgba(icon.0.rgba.clone(), icon.0.width, icon.0.height) {
            Ok(i) => {
                w.set_window_icon(Some(i));
                info!("[icon] window size={}x{}", icon.0.width, icon.0.height);
            }
            Err(e) => warn!("[icon] refused reason={e}"),
        }
        *done = true;
    });
}
