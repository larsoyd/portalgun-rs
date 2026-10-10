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
    seed: u32,
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
            seed: 0x9e37_79b9,
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
        let draws = [self.draw(), self.draw(), self.draw()];
        let entry = match self.bank.get(name) {
            Ok(e) => e,
            Err(e) => {
                warn!("[sound] refused name={name} reason={e}");
                return None;
            }
        };
        let pick = entry.pick(draws);
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

    // xorshift, the engine's own random stream is not traced
    fn draw(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1 << 24) as f32
    }
}
