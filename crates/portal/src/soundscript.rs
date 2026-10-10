use std::collections::HashMap;

use crate::error::malformed;
use crate::keyvalues::{self, Kv};
use crate::Result;

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
    })
}
