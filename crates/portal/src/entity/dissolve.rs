// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

// dissolver spawn stores these for type 0, server.so 0x4f7670
const FADE_IN_START: f32 = 0.0;
const FADE_IN_LENGTH: f32 = 1.0;
const FADE_OUT_MODEL_START: f32 = 1.9;
const FADE_OUT_MODEL_LENGTH: f32 = f32::from_bits(0x3dcc_ccd0);
const FADE_OUT_START: f32 = 2.0;

/// Translated from 0x3931b0 (decompiled, client.so), a dissolving model colour dt seconds in
pub fn colour(dt: f32) -> [u8; 4] {
    let fade = if dt > FADE_OUT_START {
        0.0
    } else if dt < FADE_IN_START {
        1.0
    } else if dt > FADE_IN_START && dt < FADE_IN_START + FADE_IN_LENGTH {
        1.0 - (dt - FADE_IN_START) / FADE_IN_LENGTH
    } else {
        0.0
    };
    let rgb = (255.0 * fade) as u8;
    let out = dt - FADE_OUT_MODEL_START;
    let alpha = if dt <= FADE_OUT_MODEL_START {
        255
    } else if out <= FADE_OUT_MODEL_LENGTH {
        (255.0 * (1.0 - out / FADE_OUT_MODEL_LENGTH)) as u8
    } else {
        0
    };
    [rgb, rgb, rgb, alpha]
}
