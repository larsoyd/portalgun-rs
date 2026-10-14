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
    refused: std::collections::HashSet<String>,
    seed: crate::random::Xorshift,
    scale: f32,
}

impl Sounds {
    pub fn read(search: &SearchPath) -> portal::Result<Sounds> {
        let bank = Bank::read(search)?;
        info!(
            "[sound] bank scripts={} missing_scripts={}",
            bank.scripts,
            bank.missing.len()
        );
        Ok(Sounds {
            bank,
            waves: HashMap::new(),
            refused: default(),
            seed: crate::random::Xorshift::default(),
            scale: 1.0,
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
        self.start(
            commands,
            files,
            audio,
            name,
            volume,
            pitch,
            PlaybackSettings::DESPAWN,
        )
    }

    /// Plays at the script's own volume times a factor, as physics impacts do.
    pub fn play_scaled(
        &mut self,
        commands: &mut Commands,
        files: &Files,
        audio: &mut Assets<AudioSource>,
        name: &str,
        factor: f32,
    ) -> Option<Entity> {
        self.scale = factor;
        let id = self.play(commands, files, audio, name);
        self.scale = 1.0;
        id
    }

    pub fn entry(&self, name: &str) -> Option<&portal::soundscript::Entry> {
        self.bank.get(name).ok()
    }

    /// A sound that runs until despawned, like the controller patches the game fades.
    pub fn looped(
        &mut self,
        commands: &mut Commands,
        files: &Files,
        audio: &mut Assets<AudioSource>,
        name: &str,
        volume: f32,
    ) -> Option<Entity> {
        self.start(
            commands,
            files,
            audio,
            name,
            Some(volume),
            None,
            PlaybackSettings::LOOP,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn start(
        &mut self,
        commands: &mut Commands,
        files: &Files,
        audio: &mut Assets<AudioSource>,
        name: &str,
        volume: Option<f32>,
        pitch: Option<f32>,
        mode: PlaybackSettings,
    ) -> Option<Entity> {
        let draws = [self.draw(), self.draw(), self.draw()];
        let entry = match self.bank.get(name) {
            Ok(e) => e,
            Err(e) => {
                if self.refused.insert(name.to_owned()) {
                    warn!("[sound] refused name={name} reason={e}");
                }
                return None;
            }
        };
        let mut pick = entry.pick(draws);
        pick.volume = volume.unwrap_or(pick.volume) * self.scale;
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
                    if self.refused.insert(path.clone()) {
                        warn!("[sound] refused name={name} wave={path} reason={e}");
                    }
                    return None;
                }
            },
        };
        self.refused.remove(name);
        self.refused.remove(path);
        info!(
            "[sound] queued name={name} wave={path} volume={:.3} pitch={:.1}",
            pick.volume, pick.pitch
        );
        let settings = mode
            .with_volume(Volume::Linear(pick.volume))
            .with_speed(pick.pitch / 100.0);
        Some(commands.spawn((AudioPlayer(wave), settings)).id())
    }

    fn draw(&mut self) -> f32 {
        self.seed.unit()
    }
}
