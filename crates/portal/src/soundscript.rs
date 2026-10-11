// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::collections::HashMap;

use crate::error::malformed;
use crate::keyvalues::{self, Kv};
use crate::vpk::SearchPath;
use crate::{Error, Result};

/// Source's sound prefix characters (stream, spatial, distance variant and so on).
const SOUND_CHARS: &[char] = &[
    '*', '?', '!', '#', '>', '<', '^', '@', ')', '}', '$', '&', '~', '`', '+', '%',
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pitch {
    Fixed(f32),
    Range(f32, f32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub waves: Vec<String>,
    /// low and high, Source picks one between them each time
    pub volume: (f32, f32),
    pub pitch: Pitch,
    /// as written, the dB table it names lives in the engine
    pub soundlevel: Option<String>,
}

/// Sound script entries by lower-case name, waves as paths inside the game's files.
pub fn parse(text: &str) -> Result<HashMap<String, Entry>> {
    keyvalues::parse(text)?
        .iter()
        .map(|kv| Ok((kv.key.to_ascii_lowercase(), entry(kv)?)))
        .collect()
}

fn entry(kv: &Kv) -> Result<Entry> {
    let mut waves: Vec<&str> = kv.all("wave").collect();
    if let Some(r) = kv.block("rndwave") {
        waves.extend(r.all("wave"));
    }
    if waves.is_empty() {
        return Err(malformed("sound script", format!("{} has no wave", kv.key)));
    }
    let number = |v: &str| {
        v.trim()
            .parse::<f32>()
            .map_err(|_| malformed("sound script", format!("{}: {v:?}", kv.key)))
    };
    let volume = match kv.get("volume") {
        None | Some("VOL_NORM") => (1.0, 1.0),
        Some(v) => match v.split_once(',') {
            Some((lo, hi)) => (number(lo)?, number(hi)?),
            None => (number(v)?, number(v)?),
        },
    };
    let pitch = match kv.get("pitch") {
        None | Some("PITCH_NORM") => Pitch::Fixed(100.0),
        Some("PITCH_LOW") => Pitch::Fixed(95.0),
        Some("PITCH_HIGH") => Pitch::Fixed(120.0),
        Some(v) => match v.split_once(',') {
            Some((lo, hi)) => Pitch::Range(number(lo)?, number(hi)?),
            None => Pitch::Fixed(number(v)?),
        },
    };
    Ok(Entry {
        waves: waves
            .iter()
            .map(|w| {
                format!(
                    "sound/{}",
                    w.trim_start_matches(SOUND_CHARS).replace('\\', "/")
                )
            })
            .collect(),
        volume,
        pitch,
        soundlevel: kv.get("soundlevel").map(|v| v.trim().to_string()),
    })
}

const MANIFEST: &str = "scripts/game_sounds_manifest.txt";

/// One play of an entry, from three uniform draws in 0..1.
pub struct Pick {
    pub wave: usize,
    pub volume: f32,
    pub pitch: f32,
}

impl Entry {
    pub fn pick(&self, r: [f32; 3]) -> Pick {
        let between = |(lo, hi): (f32, f32), t: f32| lo + (hi - lo) * t;
        let wave = ((r[0] * self.waves.len() as f32) as usize).min(self.waves.len() - 1);
        let pitch = match self.pitch {
            Pitch::Fixed(p) => p,
            Pitch::Range(lo, hi) => between((lo, hi), r[2]),
        };
        Pick {
            wave,
            volume: between(self.volume, r[1]),
            pitch,
        }
    }
}

/// Every sound the manifest's scripts declare, as far as the pinned files hold them.
pub struct Bank {
    entries: HashMap<String, Entry>,
    ambiguous: Vec<String>,
    pub scripts: usize,
    pub missing: Vec<String>,
}

impl Bank {
    pub fn read(search: &SearchPath) -> Result<Bank> {
        let text = |path: &str| {
            search
                .read(path)
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        };
        let manifest = keyvalues::parse(&text(MANIFEST)?)?;
        let files = manifest
            .first()
            .map(|m| m.all("precache_file").collect::<Vec<_>>());
        let mut bank = Bank {
            entries: HashMap::new(),
            ambiguous: Vec::new(),
            scripts: 0,
            missing: Vec::new(),
        };
        for file in files.unwrap_or_default() {
            let Ok(script) = text(file) else {
                bank.missing.push(file.to_string());
                continue;
            };
            bank.scripts += 1;
            for (name, entry) in parse(&script)? {
                match bank.entries.get(&name) {
                    Some(seen) if *seen != entry => bank.ambiguous.push(name),
                    _ => drop(bank.entries.insert(name, entry)),
                }
            }
        }
        Ok(bank)
    }

    /// Two scripts disagreeing on a name is refused, since the engine's merge rule is not traced.
    pub fn get(&self, name: &str) -> Result<&Entry> {
        let key = name.to_ascii_lowercase();
        if self.ambiguous.contains(&key) {
            return Err(Error::Refused(format!(
                "ambiguous sound {key}, two scripts disagree"
            )));
        }
        self.entries
            .get(&key)
            .ok_or_else(|| Error::Refused(format!("unknown sound {key}")))
    }
}
