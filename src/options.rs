// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::settings::Settings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Improvement {
    Airwalk,
    GunLag,
    BodyShadows,
    Feet,
    Gait,
    EyeGate,
}

impl Improvement {
    pub const ALL: [Self; 6] = [
        Self::Airwalk,
        Self::GunLag,
        Self::BodyShadows,
        Self::Feet,
        Self::Gait,
        Self::EyeGate,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Airwalk => "Hold jump pose during airwalk",
            Self::GunLag => "Continuous gun sway through portals",
            Self::BodyShadows => "Player body shadows",
            Self::Feet => "Fit feet to the floor",
            Self::Gait => "Match walk cycle to footsteps",
            Self::EyeGate => "Symmetric portal camera crossing",
        }
    }

    fn original(self, settings: &mut Settings) -> &mut bool {
        match self {
            Self::Airwalk => &mut settings.original_airwalk,
            Self::GunLag => &mut settings.original_gun_lag,
            Self::BodyShadows => &mut settings.original_body_shadows,
            Self::Feet => &mut settings.original_feet,
            Self::Gait => &mut settings.original_gait,
            Self::EyeGate => &mut settings.original_eye_gate,
        }
    }

    pub fn enabled(self, settings: &Settings) -> bool {
        !*self.original(&mut settings.clone())
    }

    pub fn toggle(self, settings: &mut Settings) {
        let original = self.original(settings);
        *original = !*original;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Closed,
    Pause,
    Options,
    Quit,
}

#[derive(Default)]
pub struct Options {
    pub page: Page,
    pub draft: Option<Settings>,
}

impl Options {
    // Translated from 0x4edb30 (disassembly, engine.so)
    pub fn escape(&mut self) {
        match self.page {
            Page::Closed => {
                self.page = if self.draft.is_some() {
                    Page::Options
                } else {
                    Page::Pause
                }
            }
            Page::Pause | Page::Options => self.page = Page::Closed,
            Page::Quit => self.page = Page::Pause,
        }
    }

    pub fn open(&mut self, settings: &Settings) {
        self.draft = Some(settings.clone());
        self.page = Page::Options;
    }

    pub fn cancel(&mut self) {
        self.draft = None;
        self.page = Page::Pause;
    }

    pub fn restore_original(&mut self) {
        if let Some(draft) = &mut self.draft {
            for improvement in Improvement::ALL {
                *improvement.original(draft) = true;
            }
        }
    }

    pub fn apply(&mut self, settings: &mut Settings, path: &std::path::Path) -> Result<(), String> {
        if let Some(draft) = &self.draft {
            draft.save(path)?;
            *settings = draft.clone();
        }
        Ok(())
    }
}
