// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::collections::HashMap;

use bevy::audio::Volume;
use bevy::prelude::*;
use portal::soundscript::Bank;
use portal::vpk::SearchPath;

#[derive(Resource, Deref)]
pub struct Files(pub SearchPath);

#[derive(Resource)]
pub struct Sounds {
    bank: Bank,
    waves: HashMap<String, Handle<AudioSource>>,
    seed: crate::random::Xorshift,
}

impl Sounds {
    pub fn read(search: &SearchPath) -> portal::Result<Sounds> {
        let bank = Bank::read(search)?;
        println!(
            "[sound] bank scripts={} missing_scripts={}",
            bank.scripts,
            bank.missing.len()
        );
        Ok(Sounds {
            bank,
            waves: HashMap::new(),
            seed: crate::random::Xorshift::default(),
        })
    }

    pub fn decoded(&self) -> usize {
        self.waves.len()
    }

    pub fn play(
        &mut self,
        commands: &mut Commands,
        files: &Files,
        audio: &mut Assets<AudioSource>,
        name: &str,
    ) -> Option<Entity> {
        self.play_as(commands, files, audio, name, None, None)
    }

    /// Plays with the volume or pitch the caller sets in place of the script's, as the game's
    /// footsteps and landings do.
    pub fn play_as(
        &mut self,
        commands: &mut Commands,
        files: &Files,
        audio: &mut Assets<AudioSource>,
        name: &str,
        volume: Option<f32>,
        pitch: Option<f32>,
    ) -> Option<Entity> {
        let draws = [self.draw(), self.draw(), self.draw()];
        let entry = match self.bank.get(name) {
            Ok(e) => e,
            Err(e) => {
                warn!("[sound] refused name={name} reason={e}");
                return None;
            }
        };
        let mut pick = entry.pick(draws);
        pick.volume = volume.unwrap_or(pick.volume);
        pick.pitch = pitch.unwrap_or(pick.pitch);
        let path = &entry.waves[pick.wave];
        let wave = match self.waves.get(path) {
            Some(h) => h.clone(),
            None => match files.read(path) {
                Ok(bytes) => {
                    let h = audio.add(AudioSource {
                        bytes: bytes.into(),
                    });
                    self.waves.insert(path.clone(), h.clone());
                    h
                }
                Err(e) => {
                    warn!("[sound] refused name={name} wave={path} reason={e}");
                    return None;
                }
            },
        };
        info!(
            "[sound] played name={name} wave={path} volume={:.3} pitch={:.1}",
            pick.volume, pick.pitch
        );
        let settings = PlaybackSettings::DESPAWN
            .with_volume(Volume::Linear(pick.volume))
            .with_speed(pick.pitch / 100.0);
        Some(commands.spawn((AudioPlayer(wave), settings)).id())
    }

    fn draw(&mut self) -> f32 {
        self.seed.unit()
    }
}
