//! Sprite sheets from a VTF's sheet resource, sampled as the client's CSheet (0x6883f0) builds them.

use crate::error::malformed;
use crate::Result;

pub type Rect = [f32; 4];

/// One of the 1024 samples across a sequence: the frame before, the frame after and how far between.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub a: Rect,
    pub b: Rect,
    pub blend: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct Sequence {
    clamp: bool,
    span: f32,
    samples: Vec<Sample>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    sequences: Vec<Sequence>,
}

const SLOTS: usize = 64;
const SAMPLES: usize = 1024;

struct Cursor<'a>(&'a [u8], usize);

impl Cursor<'_> {
    fn word(&mut self) -> Result<[u8; 4]> {
        let w = self
            .0
            .get(self.1..self.1 + 4)
            .ok_or_else(|| malformed("sheet", format!("cut short at byte {}", self.1)))?;
        self.1 += 4;
        Ok(w.try_into().unwrap())
    }

    fn int(&mut self) -> Result<i32> {
        self.word().map(i32::from_le_bytes)
    }

    fn float(&mut self) -> Result<f32> {
        self.word().map(f32::from_le_bytes)
    }
}

/// GetInterpolationData: the knots around a position, wrapping past the ends or holding them.
fn interpolate(knots: &[f32], at: f32, wrap: bool) -> (usize, usize, f32) {
    let n = knots.len();
    let range = SAMPLES as f32;
    let idx = knots.iter().position(|k| *k >= at).unwrap_or(n);
    let (k1, k2, gap, offset) = if idx == 0 || idx == n {
        if !wrap {
            let k = if idx == 0 { 0 } else { n - 1 };
            return (k, k, 1.0);
        }
        let gap = knots[0] + (range - knots[n - 1]);
        let offset = if idx == 0 {
            at + (range - knots[n - 1])
        } else {
            at - knots[n - 1]
        };
        (n - 1, 0, gap, offset)
    } else {
        (
            idx - 1,
            idx,
            knots[idx] - knots[idx - 1],
            at - knots[idx - 1],
        )
    };
    (k1, k2, offset / gap)
}

impl Sheet {
    pub fn parse(bytes: &[u8]) -> Result<Sheet> {
        let mut c = Cursor(bytes, 0);
        let rects = if c.int()? == 0 { 1 } else { 4 };
        let count = c.int()?;
        let mut slots: Vec<Option<Sequence>> = vec![None; SLOTS];
        let mut first = None;
        for _ in 0..count {
            let seq = c.int()?;
            let slot = usize::try_from(seq)
                .ok()
                .filter(|s| *s < SLOTS)
                .ok_or_else(|| malformed("sheet", format!("sequence number {seq} too high")))?;
            let clamp = c.int()? != 0;
            let n = c.int()?;
            let total = c.float()? as i32;
            let mut frames = Vec::new();
            let mut knots = Vec::new();
            let mut at = 0.0;
            for _ in 0..n.max(0) {
                let duration = c.float()?;
                let rect = [c.float()?, c.float()?, c.float()?, c.float()?];
                for _ in 1..rects {
                    (0..4).try_for_each(|_| c.float().map(drop))?;
                }
                knots.push(SAMPLES as f32 / total as f32 * at);
                at += duration;
                frames.push(rect);
            }
            if frames.is_empty() {
                return Err(malformed("sheet", format!("sequence {seq} has no frames")));
            }
            let samples = if frames.len() == 1 {
                vec![Sample {
                    a: frames[0],
                    b: frames[0],
                    blend: 1.0,
                }]
            } else {
                (0..SAMPLES)
                    .map(|i| {
                        let (k1, k2, blend) = interpolate(&knots, i as f32, !clamp);
                        Sample {
                            a: frames[k1],
                            b: frames[k2],
                            blend,
                        }
                    })
                    .collect()
            };
            first.get_or_insert(slot);
            slots[slot] = Some(Sequence {
                clamp,
                span: at,
                samples,
            });
        }
        let first = first.ok_or_else(|| malformed("sheet", "no sequences"))?;
        let fill = slots[first].clone();
        Ok(Sheet {
            sequences: slots
                .into_iter()
                .map(|s| s.or_else(|| fill.clone()).unwrap())
                .collect(),
        })
    }

    fn seq(&self, seq: usize) -> &Sequence {
        &self.sequences[seq.min(SLOTS - 1)]
    }

    /// The sequence's total frame time (frameSpan).
    pub fn span(&self, seq: usize) -> f32 {
        self.seq(seq).span
    }

    pub fn clamps(&self, seq: usize) -> bool {
        self.seq(seq).clamp
    }

    pub fn sample(&self, seq: usize, index: usize) -> Sample {
        let s = &self.seq(seq).samples;
        s[index.min(s.len() - 1)]
    }
}
