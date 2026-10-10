use std::collections::HashMap;

use crate::keyvalues::{self, Kv, Value};
use crate::vpk::SearchPath;
use crate::Result;

const MANIFEST: &str = "scripts/surfaceproperties_manifest.txt";
const DEFAULT: &str = "default";

/// What a surface plays underfoot.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Surface {
    pub step_left: String,
    pub step_right: String,
    /// the gamematerial character
    pub material: u8,
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

    /// Each entry starts as a copy of its base, else of the default.
    pub fn from_files(files: &[Vec<Kv>]) -> Surfaces {
        let mut out: HashMap<String, Surface> = HashMap::new();
        for entry in files.iter().flatten() {
            let Value::Block(_) = entry.value else {
                continue;
            };
            let name = entry.key.to_ascii_lowercase();
            let base = entry.get("base").map(str::to_ascii_lowercase);
            let mut s = out
                .get(base.as_deref().unwrap_or(DEFAULT))
                .cloned()
                .unwrap_or_default();
            if let Some(v) = entry.get("stepleft") {
                s.step_left = v.to_string();
            }
            if let Some(v) = entry.get("stepright") {
                s.step_right = v.to_string();
            }
            if let Some(v) = entry.get("gamematerial").and_then(|v| v.bytes().next()) {
                s.material = v;
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
        };
        let map = &self.0;
        map.get(&name.to_ascii_lowercase())
            .or_else(|| map.get(DEFAULT))
            .unwrap_or(&NONE)
    }
}
