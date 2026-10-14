// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::Path;

use crate::error::malformed;
use crate::particles::random::sha256;
use crate::vmf::{output, Entity};
use crate::{Error, Result};

/// The original maps whose entities we cite, by sha256
pub const MAPS: [(&str, &str); 2] = [
    (
        "testchmb_a_00",
        "9e1674866df01e6383a654eb4da7c770f9e788786e3bc5d412aece4f2432747e",
    ),
    (
        "testchmb_a_01",
        "a3a898490bd8b7622eeed1993fc8d18b483c4aa3e9c8bdd4656c59bfb29eb8e4",
    ),
];

const VERSION: i32 = 20;

pub fn open(root: &Path, name: &str) -> Result<Vec<Entity>> {
    let Some(&(_, want)) = MAPS.iter().find(|(map, _)| *map == name) else {
        return Err(Error::Refused(format!("no pinned hash for map {name}")));
    };
    let path = root.join(format!("portal/maps/{name}.bsp"));
    let bytes =
        std::fs::read(&path).map_err(|e| Error::Refused(format!("{}: {e}", path.display())))?;
    let got: String = sha256(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    if got != want {
        return Err(Error::Refused(format!(
            "{name}.bsp sha256={got} expected={want}"
        )));
    }
    entities(&bytes)
}

/// Lump 0, the entity text vbsp writes from the map's entities.
pub fn entities(bytes: &[u8]) -> Result<Vec<Entity>> {
    let int = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
            .ok_or_else(|| malformed("bsp", format!("truncated at {at}")))
    };
    if bytes.get(..4) != Some(b"VBSP") {
        return Err(malformed("bsp", "no VBSP magic"));
    }
    let version = int(4)?;
    if version != VERSION {
        return Err(malformed(
            "bsp",
            format!("version {version}, want {VERSION}"),
        ));
    }
    let (offset, length) = (int(8)? as usize, int(12)? as usize);
    let text = bytes
        .get(offset..offset + length)
        .ok_or_else(|| malformed("bsp", format!("entity lump {offset}+{length} past the end")))?;
    String::from_utf8_lossy(text)
        .split('{')
        .skip(1)
        .map(entity)
        .collect()
}

fn entity(block: &str) -> Result<Entity> {
    let body = block.split('}').next().unwrap_or_default();
    let quoted: Vec<&str> = body.split('"').skip(1).step_by(2).collect();
    let keys: Vec<(String, String)> = quoted
        .as_chunks::<2>()
        .0
        .iter()
        .map(|[k, v]| (k.to_string(), v.to_string()))
        .collect();
    let classname = keys
        .iter()
        .find(|(k, _)| k == "classname")
        .map(|(_, v)| v.clone())
        .ok_or_else(|| malformed("bsp", format!("entity without classname: {body:?}")))?;
    let outputs = keys
        .iter()
        .filter(|(k, _)| k.starts_with("On"))
        .map(|(k, v)| output(k, v))
        .collect::<Result<_>>()?;
    Ok(Entity {
        classname,
        keys,
        outputs,
        solids: Vec::new(),
    })
}
