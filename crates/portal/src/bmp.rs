// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::error::malformed;
use crate::{Error, Result};

const BITFIELDS: u32 = 3;

#[derive(Debug)]
pub struct Bmp {
    pub width: u32,
    pub height: u32,
    /// top row first
    pub rgba: Vec<u8>,
}

impl Bmp {
    /// The 32-bit bitfield layout the game's icon uses, nothing else.
    pub fn parse(bytes: &[u8]) -> Result<Bmp> {
        let u32_at = |at: usize| -> Result<u32> {
            bytes
                .get(at..at + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or_else(|| malformed("BMP", format!("ends before {at:#x}")))
        };
        if !bytes.starts_with(b"BM") {
            return Err(malformed("BMP", "no BM signature"));
        }
        let (data, width, height) = (u32_at(10)? as usize, u32_at(18)?, u32_at(22)? as i32);
        let bpp = u32_at(28)? & 0xffff;
        let compression = u32_at(30)?;
        if bpp != 32 || compression != BITFIELDS {
            return Err(Error::Unsupported(format!(
                "BMP with {bpp} bits and compression {compression}"
            )));
        }
        let masks = [u32_at(54)?, u32_at(58)?, u32_at(62)?, u32_at(66)?];
        if masks.contains(&0) {
            return Err(malformed(
                "BMP",
                format!("empty channel mask in {masks:x?}"),
            ));
        }
        let rows = height.unsigned_abs() as usize;
        let mut rgba = Vec::with_capacity(width as usize * rows * 4);
        for y in 0..rows {
            let row = if height > 0 { rows - 1 - y } else { y };
            for x in 0..width as usize {
                let px = u32_at(data + (row * width as usize + x) * 4)?;
                rgba.extend(masks.map(|m| ((px & m) >> m.trailing_zeros()) as u8));
            }
        }
        Ok(Bmp {
            width,
            height: rows as u32,
            rgba,
        })
    }
}
