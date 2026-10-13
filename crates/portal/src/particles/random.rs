// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::error::malformed;
use crate::Result;

/// Where Portal's client.so keeps its 4096 per-particle random floats, and their hash.
pub const TABLE_OFFSET: usize = 0xa50fc0;
const TABLE_LEN: usize = 4096;
const TABLE_SHA256: [u8; 32] = [
    0x35, 0x58, 0x16, 0xca, 0x0d, 0x97, 0xac, 0x2d, 0x71, 0xab, 0x5e, 0x71, 0x0a, 0xd7, 0xb5, 0x08,
    0xa8, 0x7c, 0x56, 0x4a, 0x6f, 0x87, 0x72, 0xaf, 0x46, 0x0a, 0x6e, 0xa3, 0xee, 0x59, 0x87, 0xc1,
];
/// SeedRandSIMD's seed from the particle manager's start-up (raw 0x6731f0).
const SIMD_SEED: u32 = 12_345_678;
const SIMD_ENTRIES: usize = 55;

pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend(((data.len() as u64) * 8).to_be_bytes());
    for chunk in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[4 * i..4 * i + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for (a, b) in h.iter_mut().zip(v) {
            *a = a.wrapping_add(b);
        }
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// The per-particle random floats every scalar random draw and operator random reads.
#[derive(Clone)]
pub struct RandomTable(std::sync::Arc<[f32]>);

impl RandomTable {
    /// Read out of Portal's own client.so. Any other build gets refused.
    pub fn from_client(bytes: &[u8]) -> Result<Self> {
        let offset = if bytes.starts_with(b"MZ") {
            0x4b4fe0
        } else {
            TABLE_OFFSET
        };
        let raw = bytes.get(offset..offset + 4 * TABLE_LEN).ok_or_else(|| {
            malformed(
                "particle random table",
                format!("{} bytes, too short for the random table", bytes.len()),
            )
        })?;
        if sha256(raw) != TABLE_SHA256 {
            return Err(malformed(
                "particle random table",
                "the random table's hash is not Portal's",
            ));
        }
        Ok(RandomTable(
            raw.as_chunks::<4>()
                .0
                .iter()
                .map(|&c| f32::from_le_bytes(c))
                .collect::<Vec<_>>()
                .into(),
        ))
    }

    pub fn from_values(values: Vec<f32>) -> Self {
        RandomTable(values.into())
    }

    pub fn get(&self, index: u32) -> f32 {
        self.0[(index & 0xfff) as usize]
    }
}

/// RandSIMD's additive lagged-Fibonacci stream, the first of its 32 contexts as one thread uses it.
pub struct SimdRandom {
    entries: [[f32; 4]; SIMD_ENTRIES],
    j: usize,
    k: usize,
}

impl Default for SimdRandom {
    fn default() -> Self {
        Self::new()
    }
}

impl SimdRandom {
    pub fn new() -> Self {
        let mut x = SIMD_SEED;
        let mut entries = [[0.0; 4]; SIMD_ENTRIES];
        for entry in entries.iter_mut() {
            for lane in entry.iter_mut() {
                *lane = ((x >> 16) as f64 * (1.0 / 65536.0)) as f32;
                x = x.wrapping_add(1).wrapping_mul(0xbb40_e62d);
            }
        }
        SimdRandom {
            entries,
            j: 23,
            k: 54,
        }
    }

    pub fn draw(&mut self) -> [f32; 4] {
        let mut r = [0.0; 4];
        for (l, out) in r.iter_mut().enumerate() {
            let sum = self.entries[self.j][l] + self.entries[self.k][l];
            *out = if sum >= 1.0 { sum - 1.0 } else { sum };
        }
        self.entries[self.k] = r;
        self.j = if self.j == 0 {
            SIMD_ENTRIES - 1
        } else {
            self.j - 1
        };
        self.k = if self.k == 0 {
            SIMD_ENTRIES - 1
        } else {
            self.k - 1
        };
        r
    }
}

/// Pow_FixedPoint_Exponent_SIMD: the exponent in whole quarters, built from square roots and
/// squarings. The reciprocal for negative exponents is exact here where Source estimates it.
pub fn pow_fixed(x: f32, exponent: f32) -> f32 {
    let n = (exponent as f64 * 4.0) as i32;
    let mut q = n.unsigned_abs();
    let mut r = 1.0f32;
    if q & 3 != 0 {
        let s = x.sqrt();
        if q & 1 != 0 {
            r = s.sqrt();
        }
        if q & 2 != 0 {
            r *= s;
        }
    }
    q >>= 2;
    let mut p = x;
    while q != 0 {
        if q & 1 != 0 {
            r *= p;
        }
        q >>= 1;
        p *= p;
    }
    if n < 0 {
        1.0 / if r == 0.0 { f32::EPSILON } else { r }
    } else {
        r
    }
}
