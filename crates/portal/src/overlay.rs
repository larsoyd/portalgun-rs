// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::carry::{cross, dot};
use crate::error::malformed;
use crate::pair::Vec3;
use crate::vmf::{vec3, Entity};
use crate::{Error, Result};

const OVERLAYS: usize = 45;
const WATER: usize = 50;
const FADES: usize = 60;
const RECORD: usize = 0x160;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Overlay {
    pub faces: Vec<u32>,
    pub order: u8,
    pub u: [f32; 2],
    pub v: [f32; 2],
    pub points: [[f32; 2]; 4],
    pub origin: Vec3,
    /// U, V and the normal
    pub basis: [Vec3; 3],
    /// squared fade start and end distances
    pub fade: [f32; 2],
}

impl Overlay {
    /// What vbsp writes for an info_overlay, read off the first chamber's accessors and their records
    pub fn from_keys(e: &Entity) -> Result<Overlay> {
        let key = |k: &str| {
            e.get(k)
                .ok_or_else(|| Error::Refused(format!("overlay without {k}")))
        };
        let number = |k: &str| -> Result<f32> {
            key(k)?
                .trim()
                .parse()
                .map_err(|_| malformed("overlay", format!("{k} unreadable")))
        };
        let mut points = [[0.0; 2]; 4];
        for (i, p) in points.iter_mut().enumerate() {
            let [x, y, _] = vec3(key(&format!("uv{i}"))?)?;
            *p = [x, y];
        }
        let (u, normal) = (vec3(key("BasisU")?)?, vec3(key("BasisNormal")?)?);
        let v = cross(normal, u);
        if dot(v, vec3(key("BasisV")?)?) < 0.99 {
            return Err(Error::Refused(
                "overlay BasisV is not BasisNormal cross BasisU, the flipped case is not ported"
                    .into(),
            ));
        }
        if e.get("fademaxdist").is_some_and(|d| d.trim() != "0") {
            return Err(Error::Refused("overlay fademaxdist is not ported".into()));
        }
        let faces = key("sides")?
            .split_whitespace()
            .map(|s| {
                s.parse()
                    .map_err(|_| malformed("overlay", format!("side {s}")))
            })
            .collect::<Result<_>>()?;
        Ok(Overlay {
            faces,
            order: e.get("RenderOrder").map_or(Ok(0), |o| {
                o.trim()
                    .parse()
                    .map_err(|_| malformed("overlay", "RenderOrder"))
            })?,
            u: [number("StartU")?, number("EndU")?],
            v: [number("StartV")?, number("EndV")?],
            points,
            origin: vec3(key("BasisOrigin")?)?,
            basis: [u, v, normal],
            fade: [number("fademindist")?, 0.0],
        })
    }
}

/// Translated from 0x45fe30 and 0x45b6e0 (decompiled, engine.so), the overlay records of a map
pub fn lump(bytes: &[u8]) -> Result<Vec<Overlay>> {
    let int = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .ok_or_else(|| malformed("bsp", format!("truncated at {at}")))
    };
    let float = |at: usize| int(at).map(f32::from_bits);
    let vector = |at: usize| -> Result<Vec3> { Ok([float(at)?, float(at + 4)?, float(at + 8)?]) };
    let span = |lump: usize| -> Result<(usize, usize)> {
        Ok((int(8 + lump * 16)? as usize, int(12 + lump * 16)? as usize))
    };
    let (start, length) = span(OVERLAYS)?;
    if length % RECORD != 0 {
        return Err(malformed("bsp", format!("overlay lump size {length}")));
    }
    if span(WATER)?.1 != 0 {
        return Err(Error::Refused("water overlays are not ported".into()));
    }
    let (fades, fade_length) = span(FADES)?;
    (0..length / RECORD)
        .map(|i| {
            let at = start + i * RECORD;
            let packed = int(at + 4)? >> 16;
            let mut points = [[0.0; 2]; 4];
            let mut z = [0.0; 4];
            for k in 0..4 {
                let p = vector(at + 0x118 + 12 * k)?;
                points[k] = [p[0], p[1]];
                z[k] = p[2];
            }
            let normal = vector(at + 0x154)?;
            let u = [z[0], z[1], z[2]];
            let v = cross(normal, u).map(|c| if z[3] == 1.0 { -c } else { c });
            Ok(Overlay {
                faces: (0..(packed & 0x3fff) as usize)
                    .map(|f| int(at + 8 + 4 * f))
                    .collect::<Result<_>>()?,
                order: (packed >> 14) as u8,
                u: [float(at + 0x108)?, float(at + 0x10c)?],
                v: [float(at + 0x110)?, float(at + 0x114)?],
                points,
                origin: vector(at + 0x148)?,
                basis: [u, v, normal],
                fade: if fade_length == 0 {
                    [-1.0, 0.0]
                } else {
                    [float(fades + 8 * i)?, float(fades + 8 * i + 4)?]
                },
            })
        })
        .collect()
}
