// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

/// xorshift, the engine's own random stream is not traced
#[derive(Debug, Clone, Copy)]
pub struct Xorshift(u32);

impl Default for Xorshift {
    fn default() -> Self {
        Self(0x9e37_79b9)
    }
}

impl Xorshift {
    pub fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1 << 24) as f32
    }

    pub fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }
}
