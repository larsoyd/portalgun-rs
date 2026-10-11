//! A particle system definition and its running instance: Init (0x684d10) and Simulate (0x680830).

use super::collection::{strength, Collection, ControlPoint, Fade, Particle, Vec3};
use super::emitters::Emitter;
use super::forces::{Constraint, Force, Movement, World};
use super::initializers::{initialize, Initializer, Query, CREATION};
use super::operators::{fade, Operator};
use super::random::{RandomTable, SimdRandom};
use super::renderers::Renderer;
use crate::pcf::{Child, Function, SystemDef};

const ONE_255: f32 = 1.0 / 255.0;
/// Simulate ignores steps under this (f64 1e-22 at 0x82e9c0)
const MIN_DT: f64 = 1e-22;
/// the step a first frame and a new collection assume (0x3d4ccccd)
const FIRST_DT: f32 = 0.05;

#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    pub name: String,
    pub material: String,
    pub max_particles: usize,
    pub initial_particles: usize,
    pub template: Particle,
    pub max_time_step: f32,
    pub max_sim: f32,
    pub min_sim: f32,
    pub min_frames: i32,
    pub operators: Vec<(Operator, Fade)>,
    pub initializers: Vec<Initializer>,
    pub emitters: Vec<(Emitter, Fade)>,
    pub forces: Vec<(Force, Fade)>,
    pub constraints: Vec<(Constraint, Fade)>,
    pub renderers: Vec<Renderer>,
    pub children: Vec<Child>,
}

fn ported<T>(
    system: &str,
    kind: &str,
    functions: &[Function],
    make: impl Fn(&Function) -> Option<T>,
) -> Result<Vec<(T, Fade)>, String> {
    functions
        .iter()
        .map(|f| {
            make(f).map(|t| (t, fade(f))).ok_or_else(|| {
                format!("system={system} kind={kind} function={} not ported", f.name)
            })
        })
        .collect()
}

impl Definition {
    /// Every function ported or the whole system gets refused, naming the first one missing.
    pub fn new(d: &SystemDef) -> Result<Definition, String> {
        let template = Particle {
            life: 1.0,
            radius: d.radius,
            rotation: d.rotation,
            rotation_speed: d.rotation_speed,
            // blue takes the green byte, as Init (0x684d10) fills the constant tint
            tint: [d.color[0], d.color[1], d.color[1]].map(|b| b as f32 * ONE_255),
            alpha: d.color[3] as f32 * ONE_255,
            sequence: d.sequence as f32,
            trail: 0.1,
            sequence1: d.sequence1 as f32,
            alpha2: 1.0,
            ..Particle::default()
        };
        let name = d.name.as_str();
        Ok(Definition {
            name: d.name.clone(),
            material: d.material.clone(),
            max_particles: d.max_particles.clamp(1, 5000),
            initial_particles: d.initial_particles,
            template,
            max_time_step: d.max_time_step,
            max_sim: d.max_sim,
            min_sim: d.min_sim,
            min_frames: d.min_frames,
            operators: ported(name, "operator", &d.operators, Operator::from_function)?,
            initializers: ported(
                name,
                "initializer",
                &d.initializers,
                Initializer::from_function,
            )?
            .into_iter()
            .map(|(i, _)| i)
            .collect(),
            emitters: ported(name, "emitter", &d.emitters, Emitter::from_function)?,
            forces: ported(name, "force", &d.forces, Force::from_function)?,
            constraints: ported(
                name,
                "constraint",
                &d.constraints,
                Constraint::from_function,
            )?,
            renderers: ported(name, "renderer", &d.renderers, Renderer::from_function)?
                .into_iter()
                .map(|(r, _)| r)
                .collect(),
            children: d.children.clone(),
        })
    }
}

/// A running system and its children, all sharing one random table.
pub struct System {
    pub def: Definition,
    pub c: Collection,
    operators: Vec<(Operator, Fade)>,
    initializers: Vec<Initializer>,
    emitters: Vec<(Emitter, Fade)>,
    first_frame: bool,
    prev_initialized: bool,
    drawn_frames: i32,
    pub children: Vec<System>,
}

impl System {
    /// Init: a system that starts after its delay, children made with theirs added on.
    pub fn new(
        defs: &[Definition],
        index: usize,
        delay: f32,
        seed: u32,
        table: &RandomTable,
    ) -> Self {
        let def = &defs[index];
        let mut c = Collection {
            particles: Vec::new(),
            cur_time: -delay,
            dt: 0.0,
            prev_dt: FIRST_DT,
            control_points: [ControlPoint::default(); 64],
            highest_cp: 0,
            seed,
            op_offset: 0,
            kills: Vec::new(),
            table: table.clone(),
            simd: SimdRandom::default(),
            lock_fade: None,
            query_count: 0,
            unique_id: 0,
            template: def.template,
            max_allowed: def.max_particles,
        };
        let mut emitters = def.emitters.clone();
        for (e, _) in &mut emitters {
            e.init_context(&mut c);
        }
        let children = def
            .children
            .iter()
            .enumerate()
            .map(|(k, child)| {
                let seed = seed.wrapping_add(0x81 * (k as u32 + 1));
                System::new(defs, child.system, child.delay + delay, seed, table)
            })
            .collect();
        System {
            def: def.clone(),
            c,
            operators: def.operators.clone(),
            initializers: def.initializers.clone(),
            emitters,
            first_frame: true,
            prev_initialized: false,
            drawn_frames: 0,
            children,
        }
    }

    /// SetControlPoint (0x533af0), passed down to every child.
    pub fn set_control_point(&mut self, n: usize, pos: Vec3) {
        self.c.highest_cp = self.c.highest_cp.max(n);
        self.c.control_points[n].pos = pos;
        for child in &mut self.children {
            child.set_control_point(n, pos);
        }
    }

    /// SetControlPointOrientation (0x533bc0), passed down the same way.
    pub fn set_control_point_orientation(
        &mut self,
        n: usize,
        forward: Vec3,
        right: Vec3,
        up: Vec3,
    ) {
        let p = &mut self.c.control_points[n];
        (p.forward, p.right, p.up) = (forward, right, up);
        for child in &mut self.children {
            child.set_control_point_orientation(n, forward, right, up);
        }
    }

    fn update_prev_control_points(&mut self, dt: f32) {
        self.c.prev_dt = dt;
        for p in &mut self.c.control_points[..=self.c.highest_cp] {
            p.prev = p.pos;
        }
        self.prev_initialized = true;
    }

    /// Simulate: sub-steps of the definition's time step, then the children with the whole step.
    pub fn simulate(
        &mut self,
        dt: f32,
        simd: &mut SimdRandom,
        world: &dyn World,
        query: &dyn Query,
    ) {
        self.simulate_under(dt, simd, world, query, None);
    }

    fn simulate_under(
        &mut self,
        dt: f32,
        simd: &mut SimdRandom,
        world: &dyn World,
        query: &dyn Query,
        parent_active: Option<usize>,
    ) {
        if dt < 0.0 {
            return;
        }
        if self.c.cur_time < 0.0 {
            if dt as f64 >= MIN_DT {
                self.c.cur_time += dt;
                self.update_prev_control_points(dt);
            }
            return;
        }
        std::mem::swap(&mut self.c.simd, simd);
        if self.first_frame {
            self.simulate_first_frame(query);
            self.first_frame = false;
        }
        if dt as f64 >= MIN_DT {
            self.steps(dt, world, query, parent_active);
        }
        std::mem::swap(&mut self.c.simd, simd);
        if dt as f64 >= MIN_DT {
            let active = Some(self.c.particles.len());
            for child in &mut self.children {
                child.simulate_under(dt, simd, world, query, active);
            }
            self.update_prev_control_points(dt);
        }
    }

    fn steps(
        &mut self,
        dt: f32,
        world: &dyn World,
        query: &dyn Query,
        parent_active: Option<usize>,
    ) {
        let def = &self.def;
        let (max_step, cap) = if def.max_time_step <= 0.0 {
            (0.1, 1.0)
        } else {
            (def.max_time_step, def.max_time_step * 10.0)
        };
        let mut rem = dt;
        if def.max_sim != 0.0 && self.drawn_frames <= def.min_frames {
            if def.max_sim < dt + self.c.cur_time {
                let left = def.max_sim - self.c.cur_time;
                rem = if def.min_sim > left {
                    def.min_sim
                } else {
                    left
                };
            }
            self.drawn_frames += 1;
        }
        rem = rem.min(cap);
        while rem > 0.0 {
            self.c.op_offset = 0;
            let step = rem.min(max_step);
            rem -= step;
            self.c.dt = step;
            self.c.cur_time += step;
            self.emit(query, parent_active);
            self.c.op_offset = 0;
            if !self.c.particles.is_empty() {
                self.operate(world);
            }
        }
    }

    fn emit(&mut self, query: &dyn Query, parent_active: Option<usize>) {
        let mut prev = self.c.particles.len();
        for (e, f) in &mut self.emitters {
            if let Some(s) = strength(f, self.c.cur_time) {
                if e.emit(&mut self.c, s, parent_active) && self.c.particles.len() != prev {
                    let n = self.c.particles.len() - prev;
                    initialize_new(
                        &mut self.c,
                        &mut self.initializers,
                        prev,
                        n,
                        CREATION,
                        query,
                    );
                }
            }
            prev = self.c.particles.len();
        }
    }

    fn operate(&mut self, world: &dyn World) {
        for (op, f) in &mut self.operators {
            let Some(s) = strength(f, self.c.cur_time) else {
                continue;
            };
            if let Operator::Movement {
                gravity,
                drag,
                passes,
            } = *op
            {
                let now = self.c.cur_time;
                let forces: Vec<(Force, f32)> = (self.def.forces.iter())
                    .filter_map(|(force, f)| strength(f, now).map(|s| (force.clone(), s)))
                    .collect();
                let constraints: Vec<(Constraint, bool)> = (self.def.constraints.iter())
                    .map(|(con, f)| (con.clone(), strength(f, now).is_some()))
                    .collect();
                let m = Movement {
                    gravity,
                    drag,
                    passes,
                };
                m.integrate(&mut self.c, &forces);
                m.constrain(&mut self.c, &constraints, world);
            } else {
                op.operate(&mut self.c, s);
            }
            if !self.c.kills.is_empty() {
                self.c.apply_kills();
                if self.c.particles.is_empty() {
                    break;
                }
            }
            self.c.op_offset += 17;
        }
    }

    /// SimulateFirstFrame (0x67f250): the definition's initial particles at the current time.
    fn simulate_first_frame(&mut self, query: &dyn Query) {
        self.c.dt = 0.0;
        self.drawn_frames = 0;
        if !self.prev_initialized {
            self.update_prev_control_points(FIRST_DT);
        }
        let n = self.def.initial_particles.min(self.c.max_allowed);
        if n > 0 {
            let mut p = self.c.template;
            p.creation = self.c.cur_time;
            self.c.particles = vec![p; n];
            initialize_new(&mut self.c, &mut self.initializers, 0, n, 0, query);
        }
    }

    /// StopEmission, children included. Removing the particles too clears them at once.
    pub fn stop_emission(&mut self, remove_all: bool) {
        for (e, _) in &mut self.emitters {
            e.stop();
        }
        if remove_all {
            self.c.particles.clear();
        }
        for child in &mut self.children {
            child.stop_emission(remove_all);
        }
    }

    /// IsFinished (0x6801c0).
    pub fn is_finished(&self) -> bool {
        !self.first_frame
            && self.c.particles.is_empty()
            && !self.emitters.iter().any(|(e, _)| e.may_emit(&self.c))
            && self.children.iter().all(|c| c.is_finished())
    }
}

/// InitializeNewParticles, then the ids InitParticleAttributes gives and the initial copies.
fn initialize_new(
    c: &mut Collection,
    inits: &mut [Initializer],
    first: usize,
    n: usize,
    initted: u32,
    query: &dyn Query,
) {
    initialize(c, inits, first, n, initted, query);
    for i in first..first + n {
        let id = c.unique_id.wrapping_add(c.seed) & 0xfff;
        c.unique_id = c.unique_id.wrapping_add(1);
        let p = &mut c.particles[i];
        p.id = id;
        p.initial.tint = p.tint;
        p.initial.alpha = p.alpha;
        p.initial.radius = p.radius;
    }
}
