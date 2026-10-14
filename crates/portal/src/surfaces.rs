// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::collections::HashMap;

use crate::keyvalues::{self, Kv, Value};
use crate::vpk::SearchPath;
use crate::Result;

const MANIFEST: &str = "scripts/surfaceproperties_manifest.txt";
const DEFAULT: &str = "default";

/// What a surface plays underfoot.
#[derive(Debug, Clone, PartialEq)]
pub struct Surface {
    pub step_left: String,
    pub step_right: String,
    /// the gamematerial character
    pub material: u8,
    pub friction: f32,
    pub jump_factor: f32,
    pub max_speed_factor: f32,
    /// vphysics parses a missing value as 0.001
    pub elasticity: f32,
    pub impact_hard: String,
    pub impact_soft: String,
    pub scrape_rough: String,
    pub scrape_smooth: String,
    pub hardness: f32,
    pub roughness: f32,
    pub scrape_rough_threshold: f32,
    pub impact_hard_threshold: f32,
    pub hard_min_velocity: f32,
}

impl Default for Surface {
    fn default() -> Self {
        Self {
            step_left: String::new(),
            step_right: String::new(),
            material: 0,
            friction: 1.0,
            jump_factor: 1.0,
            max_speed_factor: 1.0,
            elasticity: 0.001,
            impact_hard: String::new(),
            impact_soft: String::new(),
            scrape_rough: String::new(),
            scrape_smooth: String::new(),
            hardness: 0.0,
            roughness: 0.0,
            scrape_rough_threshold: 0.0,
            impact_hard_threshold: 0.0,
            hard_min_velocity: 0.0,
        }
    }
}

pub struct Surfaces(HashMap<String, Surface>);

impl Surfaces {
    pub fn read(search: &SearchPath) -> Result<Surfaces> {
        let parse = |path: &str| -> Result<Vec<Kv>> {
            keyvalues::parse(&String::from_utf8_lossy(&search.read(path)?))
        };
        let manifest = parse(MANIFEST)?;
        let files = manifest
            .iter()
            .flat_map(|m| m.all("file").map(String::from).collect::<Vec<_>>())
            .map(|f| parse(&f))
            .collect::<Result<Vec<_>>>()?;
        Ok(Surfaces::from_files(&files))
    }

    /// Definitions extend the existing surface or default, with base applied in token order.
    pub fn from_files(files: &[Vec<Kv>]) -> Surfaces {
        let mut out: HashMap<String, Surface> = HashMap::new();
        for entry in files.iter().flatten() {
            let Value::Block(properties) = &entry.value else {
                continue;
            };
            let name = entry.key.to_ascii_lowercase();
            let mut s = out
                .get(&name)
                .or_else(|| out.get(DEFAULT))
                .cloned()
                .unwrap_or_default();
            for property in properties {
                let Value::Text(value) = &property.value else {
                    continue;
                };
                match property.key.to_ascii_lowercase().as_str() {
                    "base" => {
                        if let Some(base) = out.get(&value.to_ascii_lowercase()) {
                            s = base.clone();
                        }
                    }
                    "stepleft" => s.step_left = value.clone(),
                    "stepright" => s.step_right = value.clone(),
                    "gamematerial" => {
                        if let Some(v) = value.bytes().next() {
                            s.material = v;
                        }
                    }
                    "friction" => s.friction = value.parse().unwrap_or(0.0),
                    "jumpfactor" => s.jump_factor = value.parse().unwrap_or(0.0),
                    "maxspeedfactor" => s.max_speed_factor = value.parse().unwrap_or(0.0),
                    "elasticity" => s.elasticity = value.parse().unwrap_or(0.0),
                    "impacthard" => s.impact_hard = value.clone(),
                    "impactsoft" => s.impact_soft = value.clone(),
                    "scraperough" => s.scrape_rough = value.clone(),
                    "scrapesmooth" => s.scrape_smooth = value.clone(),
                    "audiohardnessfactor" => s.hardness = value.parse().unwrap_or(0.0),
                    "audioroughnessfactor" => s.roughness = value.parse().unwrap_or(0.0),
                    "scraperoughthreshold" => {
                        s.scrape_rough_threshold = value.parse().unwrap_or(0.0)
                    }
                    "impacthardthreshold" => s.impact_hard_threshold = value.parse().unwrap_or(0.0),
                    "audiohardminvelocity" => s.hard_min_velocity = value.parse().unwrap_or(0.0),
                    _ => {}
                }
            }
            out.insert(name, s);
        }
        Surfaces(out)
    }

    pub fn get(&self, name: &str) -> &Surface {
        static NONE: Surface = Surface {
            step_left: String::new(),
            step_right: String::new(),
            material: 0,
            friction: 1.0,
            jump_factor: 1.0,
            max_speed_factor: 1.0,
            elasticity: 0.001,
            impact_hard: String::new(),
            impact_soft: String::new(),
            scrape_rough: String::new(),
            scrape_smooth: String::new(),
            hardness: 0.0,
            roughness: 0.0,
            scrape_rough_threshold: 0.0,
            impact_hard_threshold: 0.0,
            hard_min_velocity: 0.0,
        };
        let map = &self.0;
        map.get(&name.to_ascii_lowercase())
            .or_else(|| map.get(DEFAULT))
            .unwrap_or(&NONE)
    }
}
