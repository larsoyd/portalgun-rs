// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use portal::anim::{blend, cycle_rate, sample, BonePose};
use portal::mdl::Mdl;

/// The sequence playing and the one it fades from.
#[derive(Default)]
pub struct GunAnim {
    pub seq: usize,
    pub started: f32,
    from: Option<(usize, f32, f32)>,
}

impl GunAnim {
    /// Starts the activity's sequence, each of the gun's activities has exactly one.
    pub fn play(&mut self, mdl: &Mdl, activity: &str, now: f32) -> bool {
        let Some(seq) = mdl.sequence_for(activity) else {
            return false;
        };
        self.from = Some((self.seq, self.started, now));
        self.seq = seq;
        self.started = now;
        true
    }

    /// Seconds until a one-shot ends, loops never do.
    pub fn left(&self, mdl: &Mdl, now: f32) -> f32 {
        let s = &mdl.sequences[self.seq];
        if s.looping {
            return 0.0;
        }
        (1.0 / cycle_rate(&mdl.anims[s.anim]) - (now - self.started)).max(0.0)
    }

    pub fn pose(&self, mdl: &Mdl, now: f32) -> portal::Result<Vec<BonePose>> {
        let pose = pose_at(mdl, self.seq, now - self.started)?;
        let Some((seq, started, at)) = self.from else {
            return Ok(pose);
        };
        let fade = mdl.sequences[self.seq].fade_in;
        let w = if fade > 0.0 { (now - at) / fade } else { 1.0 };
        if w >= 1.0 {
            return Ok(pose);
        }
        let old = pose_at(mdl, seq, now - started)?;
        Ok(mix(&old, &pose, w.max(0.0)))
    }
}

/// The frame a sequence shows after some seconds.
pub fn frame_at(mdl: &Mdl, seq: usize, t: f32) -> f32 {
    let s = &mdl.sequences[seq];
    let anim = &mdl.anims[s.anim];
    let cycle = t.max(0.0) * cycle_rate(anim);
    let cycle = if s.looping {
        cycle.fract()
    } else {
        cycle.min(1.0)
    };
    cycle * (anim.frames - 1) as f32
}

fn pose_at(mdl: &Mdl, seq: usize, t: f32) -> portal::Result<Vec<BonePose>> {
    sample(mdl, mdl.sequences[seq].anim, frame_at(mdl, seq, t))
}

fn mix(a: &[BonePose], b: &[BonePose], w: f32) -> Vec<BonePose> {
    a.iter()
        .zip(b)
        .map(|(a, b)| BonePose {
            pos: [0, 1, 2].map(|i| a.pos[i] + (b.pos[i] - a.pos[i]) * w),
            quat: blend(a.quat, b.quat, w),
        })
        .collect()
}
