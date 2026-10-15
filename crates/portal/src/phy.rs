// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::error::malformed;
use crate::keyvalues;
use crate::pair::Vec3;
use crate::Result;

const HEADER: usize = 16;
const SURFACE_HEADER: usize = 28;

#[derive(Debug, Clone, PartialEq)]
pub struct Solid {
    pub mass_center: Vec3,
    /// rotation inertia for unit mass, in metres squared
    pub unit_inertia: [f32; 3],
    /// upper limit radius in metres
    pub radius: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Params {
    pub mass: f32,
    pub surfaceprop: String,
    pub damping: f32,
    pub rotdamping: f32,
    pub inertia: f32,
    pub drag: f32,
    pub volume: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Phy {
    pub solids: Vec<Solid>,
    pub params: Vec<Params>,
}

fn f32_at(b: &[u8], at: usize) -> Result<f32> {
    let bytes = b
        .get(at..at + 4)
        .ok_or_else(|| malformed("phy", format!("float at {at} past the end ({})", b.len())))?;
    Ok(f32::from_le_bytes(bytes.try_into().unwrap()))
}

fn i32_at(b: &[u8], at: usize) -> Result<i32> {
    Ok(f32_at(b, at)?.to_bits() as i32)
}

/// header, per solid a size, VPHY header and IVPS compact surface, then the solid keyvalues
pub fn parse(b: &[u8]) -> Result<Phy> {
    if i32_at(b, 0)? != HEADER as i32 {
        return Err(malformed("phy", "header size is not 16"));
    }
    let count = i32_at(b, 8)?;
    let mut at = HEADER;
    let mut solids = Vec::new();
    for i in 0..count {
        let size = i32_at(b, at)? as usize;
        if b.get(at + 4..at + 8) != Some(b"VPHY".as_slice()) {
            return Err(malformed("phy", format!("solid {i} has no VPHY marker")));
        }
        let surface = at + 4 + SURFACE_HEADER;
        let read3 = |o: usize| -> Result<Vec3> {
            Ok([f32_at(b, o)?, f32_at(b, o + 4)?, f32_at(b, o + 8)?])
        };
        solids.push(Solid {
            mass_center: read3(surface)?,
            unit_inertia: read3(surface + 0xc)?,
            radius: f32_at(b, surface + 0x18)?,
        });
        at += 4 + size;
    }
    let text = b
        .get(at..)
        .ok_or_else(|| malformed("phy", "no keyvalue text"))?;
    let text = String::from_utf8_lossy(text);
    let params = keyvalues::parse(text.trim_end_matches('\0'))?
        .iter()
        .filter(|kv| kv.key.eq_ignore_ascii_case("solid"))
        .map(|kv| {
            let num = |k: &str| kv.get(k).and_then(|v| v.trim().parse().ok()).unwrap_or(0.0);
            Params {
                mass: num("mass"),
                surfaceprop: kv.get("surfaceprop").unwrap_or_default().to_string(),
                damping: num("damping"),
                rotdamping: num("rotdamping"),
                inertia: num("inertia"),
                drag: num("drag"),
                volume: num("volume"),
            }
        })
        .collect();
    Ok(Phy { solids, params })
}
