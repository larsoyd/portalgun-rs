// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::keyvalues::{Kv, Value};

/// texture data names in the order of Icon
const NAMES: [&str; 5] = [
    "portal_crosshair_left_invalid",
    "portal_crosshair_right_invalid",
    "portal_crosshair_left_valid",
    "portal_crosshair_right_valid",
    "portal_crosshair_last_placed",
];
/// screen lines per hud scale step without the gamepad ui
const SCALE_LINES: i32 = 1600;
const BRACKET_DY: f32 = 0.17;
const LEFT_DX: f32 = 0.64;
const RIGHT_DX: f32 = -0.35;
const RING_LEFT_DX: f32 = 1.85;
const RING_RIGHT_DX: f32 = 0.75;
const FADE_UP: f32 = 600.0;
const FADE_DOWN: f32 = 300.0;
const PEAK: f32 = 255.0;
const RING_FLOOR: f32 = 128.0;
const BRACKET_ALPHA: u8 = 150;
const PLACABLE: f32 = 0.5;
const BLUE: [u8; 3] = [64, 160, 255];
const ORANGE: [u8; 3] = [255, 160, 32];
const HOLDING: [u8; 4] = [242, 202, 167, 255];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    LeftInvalid,
    RightInvalid,
    LeftValid,
    RightValid,
    LastPlaced,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite {
    pub icon: Icon,
    /// x, y, width, height in screen pixels
    pub rect: [i32; 4],
    pub colour: [u8; 4],
}

/// The last placed rings' alpha and direction, left side first.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rings {
    pub alpha: [f32; 2],
    pub up: [bool; 2],
}

/// Translated from 0x551dc0 (decompiled, client.so)
impl Default for Rings {
    fn default() -> Self {
        Rings {
            alpha: [80.0; 2],
            up: [true; 2],
        }
    }
}

impl Rings {
    /// Translated from 0x551500 to 0x551a70 (disassembled, client.so)
    fn think(&mut self, last_fired: u8, dt: f32) {
        let i = usize::from(last_fired == 2);
        if self.up[i] {
            self.alpha[i] += dt * FADE_UP;
            if self.alpha[i] > PEAK {
                self.up[i] = false;
                self.alpha[i] = 2.0 * PEAK - self.alpha[i];
            }
        } else {
            self.alpha[i] = RING_FLOOR.max(self.alpha[i] - dt * FADE_DOWN);
        }
        let o = 1 - i;
        let v = self.alpha[o] - dt * FADE_DOWN;
        self.alpha[o] = if 0.0 > v { 0.0 } else { v };
        self.up[o] = true;
    }
}

/// What the client gets from the gun, blue first.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GunState {
    pub can_fire: [bool; 2],
    /// the server's test shot result, refreshed every tenth of a second
    pub placable: [f32; 2],
    pub last_fired: u8,
    pub holding: bool,
}

/// Translated from 0x5510ac (disassembled, client.so)
pub fn scale(screen_h: i32) -> f32 {
    (screen_h / SCALE_LINES + 1) as f32
}

fn with_alpha(rgb: [u8; 3], a: u8) -> [u8; 4] {
    [rgb[0], rgb[1], rgb[2], a]
}

/// Translated from 0x550fe0 (disassembled, client.so). The brackets and rings, in draw order.
pub fn paint(
    sw: i32,
    sh: i32,
    sizes: &[(i32, i32); 5],
    gun: &GunState,
    rings: &mut Rings,
    dt: f32,
) -> Vec<Sprite> {
    let s = scale(sh);
    let x = (sw / 2) as f32;
    let top = ((sh as f32 - sizes[Icon::LeftValid as usize].1 as f32 * s) * 0.5) as i32;
    let y = top as f32;
    let (mut c, mut r) = (
        [
            with_alpha(BLUE, BRACKET_ALPHA),
            with_alpha(ORANGE, BRACKET_ALPHA),
        ],
        [with_alpha(BLUE, 0), with_alpha(ORANGE, 0)],
    );
    let mut p = gun.placable.map(|v| v > PLACABLE);
    match gun.can_fire {
        [false, _] => {
            (c[0], p[0]) = (c[1], p[1]);
        }
        [true, false] => {
            (c[1], p[1]) = (c[0], p[0]);
        }
        [true, true] => {
            rings.think(gun.last_fired, dt);
            for (ring, a) in r.iter_mut().zip(rings.alpha) {
                ring[3] = match gun.last_fired {
                    0 => 0,
                    _ => a as i32 as u8,
                };
            }
        }
    }
    if gun.holding {
        c = [HOLDING; 2];
        p = [false; 2];
    }
    let rv_h = sizes[Icon::RightValid as usize].1 as f32;
    let left = [Icon::LeftInvalid, Icon::LeftValid][usize::from(p[0])];
    let right = [Icon::RightInvalid, Icon::RightValid][usize::from(p[1])];
    let size = |icon: Icon| {
        let (w, h) = sizes[icon as usize];
        (w as f32, (w as f32 * s) as i32, (h as f32 * s) as i32)
    };
    let (lw, lsw, lsh) = size(left);
    let (rw, rsw, rsh) = size(right);
    let (gw, gsw, gsh) = size(Icon::LastPlaced);
    vec![
        Sprite {
            icon: left,
            rect: [
                (x - lw * s * LEFT_DX) as i32,
                (y - rv_h * BRACKET_DY * s) as i32,
                lsw,
                lsh,
            ],
            colour: c[0],
        },
        Sprite {
            icon: right,
            rect: [
                (rw * s * RIGHT_DX + x) as i32,
                (rv_h * s * BRACKET_DY + y) as i32,
                rsw,
                rsh,
            ],
            colour: c[1],
        },
        Sprite {
            icon: Icon::LastPlaced,
            rect: [(x - gw * s * RING_LEFT_DX) as i32, top, gsw, gsh],
            colour: r[0],
        },
        Sprite {
            icon: Icon::LastPlaced,
            rect: [(gw * s * RING_RIGHT_DX + x) as i32, top, gsw, gsh],
            colour: r[1],
        },
    ]
}

/// Translated from 0x4b8540 (decompiled, client.so). u0, v0, u1, v1 of a sheet rect.
pub fn icon_uv(rect: [i32; 4], sheet: (u32, u32)) -> [f32; 4] {
    let [x, y, w, h] = rect.map(|v| v as f32);
    let (sw, sh) = (sheet.0 as f32, sheet.1 as f32);
    [
        (x + 0.5) / sw,
        (y + 0.5) / sh,
        (x + w - 0.5) / sw,
        (y + h - 0.5) / sh,
    ]
}

/// The bracket sheet and each icon's rect, from a texture data script.
pub fn icons(root: &[Kv]) -> Result<(String, [[i32; 4]; 5]), String> {
    let data = root
        .iter()
        .find_map(|k| k.block("TextureData"))
        .ok_or("no TextureData block")?;
    let mut file = String::new();
    let mut rects = [[0; 4]; 5];
    for (i, name) in NAMES.iter().enumerate() {
        let icon = data.block(name).ok_or_else(|| format!("no {name}"))?;
        let num = |k: &str| {
            icon.get(k)
                .and_then(|v| v.trim().parse().ok())
                .ok_or_else(|| format!("{name}: bad {k}"))
        };
        rects[i] = [num("x")?, num("y")?, num("width")?, num("height")?];
        let f = icon.get("file").ok_or_else(|| format!("{name}: no file"))?;
        if !file.is_empty() && file != f {
            return Err(format!("{name}: sheet {f} differs from {file}"));
        }
        file = f.to_string();
    }
    Ok((file, rects))
}

/// The Crosshairs font's tall for a screen height. The first of each conditional pair is the
/// one for a pc that is not a Mac or a Deck.
pub fn font_tall(root: &[Kv], screen_h: i32) -> Result<f32, String> {
    let fonts = root
        .iter()
        .find_map(|k| k.block("Fonts"))
        .and_then(|f| f.block("Crosshairs"))
        .ok_or("no Crosshairs font")?;
    let Value::Block(entries) = &fonts.value else {
        return Err("Crosshairs is not a block".into());
    };
    entries
        .iter()
        .find_map(|e| {
            let range: Vec<i32> = e
                .get("yres")?
                .split_whitespace()
                .filter_map(|v| v.parse().ok())
                .collect();
            let fits = range.len() == 2 && range[0] <= screen_h && screen_h <= range[1];
            fits.then(|| e.get("tall")?.trim().parse().ok())?
        })
        .ok_or_else(|| format!("no Crosshairs font for yres {screen_h}"))
}

/// Translated from 0x4da230 (decompiled, client.so). A font icon's cell centred on the screen.
pub fn reticle_at(sw: i32, sh: i32, w: i32, h: i32) -> [i32; 2] {
    let (cx, cy) = (sw as f32 * 0.5, sh as f32 * 0.5);
    [(0.5 + cx) as i32 - w / 2, (cy + 0.5) as i32 - h / 2]
}
