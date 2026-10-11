// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

/// fov_desired's default and range in the client
const FOV_DEFAULT: f32 = 75.0;
const FOV_RANGE: (f32, f32) = (75.0, 120.0);

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub sensitivity: f32,
    pub fov: f32,
    pub msaa: u32,
    pub fullscreen: bool,
    pub vsync: bool,
    /// the bind pose Portal shows while rising out of a portal, since the model has no airwalk clip
    pub original_airwalk: bool,
    pub original_gun_lag: bool,
    pub original_body_shadows: bool,
    pub original_feet: bool,
    pub original_gait: bool,
    pub original_eye_gate: bool,
}

impl Settings {
    pub fn parse(text: &str) -> Result<Settings, String> {
        let mut s = Settings {
            sensitivity: 3.0,
            fov: FOV_DEFAULT,
            msaa: 4,
            fullscreen: true,
            vsync: false,
            original_airwalk: true,
            original_gun_lag: true,
            original_body_shadows: true,
            original_feet: true,
            original_gait: true,
            original_eye_gate: true,
        };
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let at = || format!("line {}: {line}", n + 1);
            let (key, value) = line.split_once('=').ok_or_else(at)?;
            let number = || value.trim().parse::<f32>().map_err(|_| at());
            let boolean = || value.trim().parse::<bool>().map_err(|_| at());
            match key.trim() {
                "sensitivity" => s.sensitivity = number()?,
                "fov" => s.fov = number()?.clamp(FOV_RANGE.0, FOV_RANGE.1),
                "fullscreen" => s.fullscreen = boolean()?,
                "vsync" => s.vsync = boolean()?,
                "original_airwalk" => s.original_airwalk = boolean()?,
                "original_gun_lag" => s.original_gun_lag = boolean()?,
                "original_body_shadows" => s.original_body_shadows = boolean()?,
                "original_feet" => s.original_feet = boolean()?,
                "original_gait" => s.original_gait = boolean()?,
                "original_eye_gate" => s.original_eye_gate = boolean()?,
                "msaa" => match value.trim() {
                    "1" | "2" | "4" | "8" => s.msaa = number()? as u32,
                    _ => return Err(format!("{}, msaa takes 1, 2, 4 or 8", at())),
                },
                _ => return Err(format!("{}, unknown key", at())),
            }
        }
        Ok(s)
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        use std::io::Write;
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let mut name = path.as_os_str().to_os_string();
        name.push(format!(
            ".{}.{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let temporary = std::path::PathBuf::from(name);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let save = || -> std::io::Result<()> {
            writeln!(
                file,
                "sensitivity = {}\nfov = {}\nmsaa = {}\nfullscreen = {}\nvsync = {}",
                self.sensitivity, self.fov, self.msaa, self.fullscreen, self.vsync
            )?;
            for (key, value) in [
                ("original_airwalk", self.original_airwalk),
                ("original_gun_lag", self.original_gun_lag),
                ("original_body_shadows", self.original_body_shadows),
                ("original_feet", self.original_feet),
                ("original_gait", self.original_gait),
                ("original_eye_gate", self.original_eye_gate),
            ] {
                writeln!(file, "{key} = {value}")?;
            }
            file.sync_all()?;
            std::fs::rename(&temporary, path)
        }();
        if save.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        save.map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn vertical_fov(&self) -> f32 {
        horizontal_to_vertical(self.fov)
    }
}

/// Source fovs are horizontal on a 4:3 screen, so wider screens keep the height and see more.
// Translated from 0x6069a0 and its caller at 0x607741 (disassembly, client.so)
pub fn horizontal_to_vertical(degrees: f32) -> f32 {
    2.0 * ((degrees.to_radians() / 2.0).tan() * 0.75).atan()
}
