use super::random::{RandomTable, SimdRandom};

pub type Vec3 = [f32; 3];

/// The attributes an initializer or operator reads as they were when the particle was made.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Initial {
    pub tint: Vec3,
    pub alpha: f32,
    pub radius: f32,
}

/// One particle's attributes (Source's PARTICLE_ATTRIBUTE_* 0 to 16).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Particle {
    pub xyz: Vec3,
    pub life: f32,
    pub prev: Vec3,
    pub radius: f32,
    pub rotation: f32,
    pub rotation_speed: f32,
    pub tint: Vec3,
    pub alpha: f32,
    pub creation: f32,
    pub sequence: f32,
    pub trail: f32,
    pub id: u32,
    pub yaw: f32,
    pub sequence1: f32,
    pub alpha2: f32,
    pub initial: Initial,
}

impl Particle {
    /// A scalar attribute by its index, PARTICLE_ID read as its integer value
    pub fn scalar(&self, a: u32) -> f32 {
        match a {
            0 => self.xyz[0],
            1 => self.life,
            2 => self.prev[0],
            3 => self.radius,
            4 => self.rotation,
            5 => self.rotation_speed,
            6 => self.tint[0],
            7 => self.alpha,
            8 => self.creation,
            9 => self.sequence,
            10 => self.trail,
            11 => self.id as f32,
            12 => self.yaw,
            13 => self.sequence1,
            16 => self.alpha2,
            _ => 0.0,
        }
    }

    pub fn set_scalar(&mut self, a: u32, v: f32) {
        match a {
            0 => self.xyz[0] = v,
            1 => self.life = v,
            2 => self.prev[0] = v,
            3 => self.radius = v,
            4 => self.rotation = v,
            5 => self.rotation_speed = v,
            6 => self.tint[0] = v,
            7 => self.alpha = v,
            8 => self.creation = v,
            9 => self.sequence = v,
            10 => self.trail = v,
            11 => self.id = v as u32,
            12 => self.yaw = v,
            13 => self.sequence1 = v,
            16 => self.alpha2 = v,
            _ => {}
        }
    }

    /// A vector attribute (position, previous position or tint).
    pub fn vector(&mut self, a: u32) -> Option<&mut Vec3> {
        match a {
            0 => Some(&mut self.xyz),
            2 => Some(&mut self.prev),
            6 => Some(&mut self.tint),
            _ => None,
        }
    }
}

/// A control point: where the system's effect is anchored and how it is turned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlPoint {
    pub pos: Vec3,
    pub prev: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
    pub right: Vec3,
}

impl Default for ControlPoint {
    fn default() -> Self {
        ControlPoint {
            pos: [0.0; 3],
            prev: [0.0; 3],
            // the collection constructor's defaults (0x67d870)
            forward: [0.0, 1.0, 0.0],
            up: [0.0, 0.0, 1.0],
            right: [1.0, 0.0, 0.0],
        }
    }
}

/// The state operators work on (CParticleCollection's fields they touch).
pub struct Collection {
    pub particles: Vec<Particle>,
    pub cur_time: f32,
    pub dt: f32,
    pub prev_dt: f32,
    pub control_points: [ControlPoint; 64],
    pub highest_cp: usize,
    pub seed: u32,
    /// the random offset, 17 more for every operator that has run this step
    pub op_offset: u32,
    pub kills: Vec<usize>,
    pub table: RandomTable,
    /// the process-wide SIMD stream, lent to whichever collection is simulating
    pub simd: SimdRandom,
    /// Movement Lock's two SIMD draws for the block of four being worked on
    pub lock_fade: Option<([f32; 4], [f32; 4])>,
    /// m_nRandomQueryCount: sequential table draws so far
    pub query_count: u32,
    /// m_nUniqueParticleId
    pub unique_id: u32,
    /// what a new particle holds before its initializers run: the definition's constants
    pub template: Particle,
    /// m_nMaxAllowedParticles
    pub max_allowed: usize,
}

impl Collection {
    /// R_k of an operator: the table entry for this particle, seed and operator offset.
    pub fn op_random(&self, p: &Particle, k: u32) -> f32 {
        self.table.get(
            p.id.wrapping_add(self.seed)
                .wrapping_add(self.op_offset)
                .wrapping_add(k),
        )
    }

    /// RandomFloat's draw: the next table entry after the seed.
    pub fn random(&mut self) -> f32 {
        let r = self.table.get(self.query_count.wrapping_add(self.seed));
        self.query_count = self.query_count.wrapping_add(1);
        r
    }

    pub fn random_float(&mut self, min: f32, max: f32) -> f32 {
        (max - min) * self.random() + min
    }

    pub fn random_float_exp(&mut self, min: f32, max: f32, exponent: f32) -> f32 {
        (max - min) * self.random().powf(exponent) + min
    }

    pub fn random_int(&mut self, min: i32, max: i32) -> i32 {
        ((max + 1 - min) as f32 * self.random()) as i32 + min
    }

    /// RandomVector: one draw counted, the seed added twice.
    pub fn random_vector(&mut self, min: Vec3, max: Vec3) -> Vec3 {
        let base = self.query_count.wrapping_add(self.seed);
        self.query_count = self.query_count.wrapping_add(1);
        [0, 1, 2].map(|k| {
            let r = self
                .table
                .get(base.wrapping_add(self.seed).wrapping_add(k as u32));
            (max[k] - min[k]) * r + min[k]
        })
    }

    /// GetControlPointAtTime: between the previous and current position by time (0x67f5e0).
    pub fn cp_at(&self, cp: usize, t: f32) -> Vec3 {
        let p = &self.control_points[cp];
        if self.dt == 0.0 {
            return p.pos;
        }
        let f = (((t + self.dt) - self.cur_time) / self.dt).max(0.0);
        [0, 1, 2].map(|k| (p.pos[k] - p.prev[k]) * f + p.prev[k])
    }

    pub fn kill(&mut self, index: usize) {
        self.kills.push(index);
    }

    /// ApplyKillList (0x680770): the ascending kill list fills each hole from the front with
    /// the last particle still alive, dead ones at the tail dropped first.
    pub fn apply_kills(&mut self) {
        let list = std::mem::take(&mut self.kills);
        let mut active = self.particles.len();
        let mut n = list.len();
        while n > 0 && list[n - 1] >= active {
            n -= 1;
        }
        let mut front = 0;
        while n > front {
            let k = list[front];
            front += 1;
            let mut last = active - 1;
            while n > front && list[n - 1] == last {
                n -= 1;
                last -= 1;
            }
            active = last;
            if last != k {
                self.particles[k] = self.particles[last];
            }
        }
        self.particles.truncate(active);
    }
}

/// The share of an operator's effect at the system's current time from its fade window
/// (0x67e800 with 0x67c2b0), or None when it does not run.
pub fn strength(fade: &Fade, cur_time: f32) -> Option<f32> {
    let mut t = cur_time;
    if fade.oscillate > 0.0 {
        t = ((t as f64) * (1.0 / fade.oscillate as f64)).rem_euclid(1.0) as f32;
    }
    let Fade {
        start_in: a,
        end_in: b,
        start_out: c,
        end_out: d,
        ..
    } = *fade;
    if a > t || (d > 0.0 && t > d) {
        return None;
    }
    let capped = |x: f32| if x > 1.0 { 1.0 } else { x };
    let s = if b > a {
        let c = c.max(b);
        let s = if b > t {
            capped((t - a) / (b - a))
        } else {
            1.0
        };
        if d > c && t > c {
            capped((t - d) / (c - d)).min(s)
        } else {
            s
        }
    } else if c > a {
        if d > c && t > c {
            capped((t - d) / (c - d))
        } else {
            1.0
        }
    } else if d > a && t > a {
        capped((t - d) / (a - d))
    } else {
        1.0
    };
    (s > 0.0).then_some(s)
}

/// Every operator's fade window: "operator start fadein" and the rest.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Fade {
    pub start_in: f32,
    pub end_in: f32,
    pub start_out: f32,
    pub end_out: f32,
    pub oscillate: f32,
}
