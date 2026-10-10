//! emit_continuously and emit_instantaneously from client.so.

use super::collection::Collection;
use crate::pcf::Function;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Continuous {
    pub duration: f32,
    pub start: f32,
    pub rate: f32,
    pub scale_cp: f32,
    pub use_parent: bool,
    accumulated: f32,
    emitted: i32,
    base: f32,
    stopped: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Instant {
    pub count: i32,
    pub minimum: i32,
    pub start: f32,
    pub per_frame: i32,
    pub scale_cp: i32,
    pub field: usize,
    remaining: i32,
    total: i32,
    base: f32,
    scaled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Emitter {
    Continuous(Continuous),
    Instant(Instant),
}

impl Emitter {
    /// Parameters after InitParams (0x661220 and 0x661080).
    pub fn from_function(f: &Function) -> Option<Emitter> {
        Some(match f.name.to_ascii_lowercase().as_str() {
            "emit_continuously" => Emitter::Continuous(Continuous {
                duration: f.float_or("emission_duration", 0.0).max(0.0),
                start: f.float_or("emission_start_time", 0.0),
                rate: f.float_or("emission_rate", 100.0).max(0.0),
                scale_cp: f.float_or("scale emission to used control points", 0.0),
                use_parent: f.bool_or("use parent particles for emission scaling", false),
                ..Continuous::default()
            }),
            "emit_instantaneously" => {
                let mut count = f.int_or("num_to_emit", 100);
                let mut minimum = f.int_or("num_to_emit_minimum", -1);
                if minimum >= 0 && count < minimum {
                    std::mem::swap(&mut count, &mut minimum);
                }
                let per_frame = f.int_or("maximum emission per frame", -1);
                let scale_cp = f.int_or("emission count scale control point", -1);
                if scale_cp > 63 {
                    return None;
                }
                Emitter::Instant(Instant {
                    count,
                    minimum,
                    start: f.float_or("emission_start_time", 0.0),
                    per_frame: if per_frame < 0 { i32::MAX } else { per_frame },
                    scale_cp,
                    field: f
                        .int_or("emission count scale control point field", 0)
                        .clamp(0, 2) as usize,
                    ..Instant::default()
                })
            }
            _ => return None,
        })
    }

    /// InitializeContextData: the instantaneous count is drawn here when it has a minimum.
    pub fn init_context(&mut self, c: &mut Collection) {
        match self {
            Emitter::Continuous(e) => {
                e.accumulated = 0.0;
                e.emitted = 0;
                e.base = 0.0;
                e.stopped = false;
            }
            Emitter::Instant(e) => {
                let n = if e.minimum < 0 {
                    e.count
                } else {
                    c.random_int(e.minimum, e.count)
                };
                e.remaining = n;
                e.total = n;
                e.base = 0.0;
                e.scaled = false;
            }
        }
    }

    /// Emit: appends new particles with their creation times, true when it made any.
    pub fn emit(
        &mut self,
        c: &mut Collection,
        strength: f32,
        parent_active: Option<usize>,
    ) -> bool {
        match self {
            Emitter::Continuous(e) => e.emit(c, strength, parent_active),
            Emitter::Instant(e) => e.emit(c),
        }
    }

    /// MayCreateMoreParticles.
    pub fn may_emit(&self, c: &Collection) -> bool {
        match self {
            Emitter::Continuous(e) => {
                !e.stopped
                    && e.rate > 0.0
                    && (e.duration == 0.0 || c.cur_time - c.dt <= e.duration + e.start + e.base)
            }
            Emitter::Instant(e) => e.remaining > 0,
        }
    }

    pub fn stop(&mut self) {
        match self {
            Emitter::Continuous(e) => e.stopped = true,
            Emitter::Instant(e) => e.remaining = 0,
        }
    }
}

fn spawn(c: &mut Collection, n: usize, mut time: impl FnMut() -> f32) {
    for _ in 0..n {
        let mut p = c.template;
        p.creation = time();
        c.particles.push(p);
    }
}

fn room(c: &Collection) -> i32 {
    c.max_allowed as i32 - c.particles.len() as i32
}

impl Continuous {
    /// 0x660710: the count due by now, spread evenly over the step.
    fn emit(&mut self, c: &mut Collection, strength: f32, parent_active: Option<usize>) -> bool {
        let hc = c.highest_cp as f32;
        let mut rate = strength * self.rate;
        if hc * self.scale_cp != 0.0 && !self.use_parent {
            rate = rate * hc * self.scale_cp;
        } else if self.use_parent {
            rate *= parent_active.map_or(self.scale_cp, |n| self.scale_cp * n as f32);
        }
        if rate == 0.0 || self.stopped || self.rate <= 0.0 {
            return false;
        }
        let cur = c.cur_time;
        let start = self.start + self.base;
        let (t0, t1) = if self.duration == 0.0 {
            if cur < start {
                return false;
            }
            (cur - c.dt, cur)
        } else {
            let end = self.duration + start;
            let prev = cur - c.dt;
            if end < prev || cur < start {
                return false;
            }
            (start.max(prev), end.min(cur))
        };
        let span = t1 - t0;
        self.accumulated += rate * span;
        let n = (self.accumulated.floor() - self.emitted as f32) as i32;
        self.emitted += n;
        let n = n.min(room(c));
        if n <= 0 {
            return false;
        }
        let step = span / n as f32;
        let mut t = t0;
        spawn(c, n as usize, || {
            t = (t + step).min(t1);
            t
        });
        true
    }
}

impl Instant {
    /// 0x660560: the whole count at the start time, at most so many a frame.
    fn emit(&mut self, c: &mut Collection) -> bool {
        let mut rem = self.remaining;
        if rem <= 0 {
            return false;
        }
        let at = self.start + self.base;
        let cur = c.cur_time;
        if at > cur || self.total == 0 {
            return false;
        }
        if self.scale_cp >= 0 && !self.scaled {
            let cp = self.scale_cp as usize;
            let v = if cur < at || at < cur - c.prev_dt {
                c.control_points[cp].prev
            } else {
                c.cp_at(cp, at)
            };
            self.total = (self.total as f32 * v[self.field]) as i32;
            self.scaled = true;
            rem = (self.remaining as f32 * v[self.field]) as i32;
        }
        let rem = rem.max(0);
        let now = rem.min(self.per_frame);
        let n = room(c).min(self.total).min(now);
        self.remaining = rem - now;
        if n <= 0 {
            return false;
        }
        spawn(c, n as usize, || at);
        true
    }
}
