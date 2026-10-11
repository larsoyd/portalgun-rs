use bevy::prelude::*;
use portal::anim::{blend, cycle_rate, sample, BonePose};
use portal::mdl::Mdl;
use portal::vpk::SearchPath;

use crate::visual::VisualPlayer;

#[derive(Resource)]
pub struct PlayerClips {
    pub mdl: Mdl,
    idle: usize,
    crouch: usize,
    run: usize,
    crouch_walk: usize,
    jump: usize,
    fire: [usize; 2],
}

impl PlayerClips {
    pub fn read(files: &SearchPath) -> Result<Self, String> {
        let mdl = Mdl::parse(
            files
                .read("models/player/chell_animations.mdl")
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        for name in [
            "root",
            "hips",
            "spine_mid",
            "chest",
            "neck",
            "head",
            "weapon_bone",
        ]
        .into_iter()
        .map(str::to_owned)
        .chain(["L", "R"].into_iter().flat_map(|side| {
            [
                "thigh",
                "knee",
                "ankle",
                "toe",
                "bicep",
                "elbow",
                "wrist",
                "index_base",
                "mid_base",
                "mid_mid",
                "mid_end",
                "thumb_base",
                "thumb_mid",
                "thumb_end",
            ]
            .map(move |name| format!("{name}_{side}"))
        })) {
            if !mdl.bones.iter().any(|b| b.name == name) {
                return Err(format!("missing player bone {name}"));
            }
        }
        let seq = |name| {
            mdl.sequence_named(name)
                .ok_or_else(|| format!("missing player sequence {name}"))
        };
        Ok(Self {
            idle: seq("idle_PORTALGUN")?,
            crouch: seq("idleCROUCH_PORTALGUN")?,
            run: seq("Run_PORTALGUN")?,
            crouch_walk: seq("Crouch_walk_PORTALGUN")?,
            jump: seq("portalgun_standing_jump")?,
            fire: [seq("stand_fire_PORTALGUN")?, seq("crouch_fire_PORTALGUN")?],
            mdl,
        })
    }

    pub fn bone(&self, name: &str) -> usize {
        self.mdl
            .bones
            .iter()
            .position(|b| b.name == name)
            .expect("validated player bone")
    }

    fn weights(&self, seq: usize, params: [f32; 2]) -> Vec<(usize, f32)> {
        let s = &self.mdl.sequences[seq];
        let at = [0, 1].map(|i| {
            if s.grid[i] == 1 {
                return (0, 0.0);
            }
            let fraction = (params[i] - s.param_start[i]) / (s.param_end[i] - s.param_start[i]);
            let v = fraction.clamp(0.0, 1.0) * (s.grid[i] - 1) as f32;
            let index = (v.floor() as usize).min(s.grid[i] - 2);
            (index, v - index as f32)
        });
        let mut out = Vec::new();
        for y in 0..=usize::from(s.grid[1] > 1) {
            for x in 0..=usize::from(s.grid[0] > 1) {
                let wx = if x == 0 { 1.0 - at[0].1 } else { at[0].1 };
                let wy = if y == 0 { 1.0 - at[1].1 } else { at[1].1 };
                let w = wx * wy;
                if w > 0.0 {
                    out.push((s.blends[(at[1].0 + y) * s.grid[0] + at[0].0 + x], w));
                }
            }
        }
        out
    }

    fn rate(&self, seq: usize, params: [f32; 2]) -> f32 {
        self.weights(seq, params)
            .iter()
            .map(|&(a, w)| cycle_rate(&self.mdl.anims[a]) * w)
            .sum()
    }

    fn ground_speed(&self, seq: usize, params: [f32; 2]) -> f32 {
        let movement = self
            .weights(seq, params)
            .iter()
            .fold(Vec3::ZERO, |v, &(a, w)| {
                v + Vec3::from(self.mdl.anims[a].movement) * w
            });
        movement.length() * self.rate(seq, params)
    }

    pub fn sequence(
        &self,
        seq: usize,
        cycle: f32,
        params: [f32; 2],
    ) -> Result<Vec<BonePose>, String> {
        let mut result = Vec::new();
        let mut total = 0.0;
        for (anim, weight) in self.weights(seq, params) {
            let frame = cycle.clamp(0.0, 1.0) * (self.mdl.anims[anim].frames - 1) as f32;
            let pose = sample(&self.mdl, anim, frame).map_err(|e| e.to_string())?;
            total += weight;
            if result.is_empty() {
                result = pose;
            } else {
                mix(&mut result, &pose, weight / total);
            }
        }
        Ok(result)
    }

    fn layers(&self, seq: usize, pose: &mut [BonePose], aim: [f32; 2]) -> Result<(), String> {
        for &layer in &self.mdl.sequences[seq].layers {
            let delta = self.sequence(layer, 0.0, aim)?;
            add(pose, &delta, &self.mdl.sequences[layer].weights);
        }
        Ok(())
    }
}

#[derive(Resource, Default)]
pub struct BodyAnimation {
    sequence: Option<usize>,
    cycle: f32,
    from: Vec<BonePose>,
    shown: Vec<BonePose>,
    transition: f32,
    height_from: f32,
    eye_height: f32,
    shot: Option<f32>,
    feet_yaw: Option<f32>,
    goal_yaw: f32,
    revision: u64,
}

pub struct AnimatedBody {
    pub bones: Vec<Mat4>,
    pub yaw: f32,
    pub origin: Vec3,
}

impl BodyAnimation {
    pub fn fire(&mut self, now: f32) {
        self.shot = Some(now);
    }

    pub fn pose(
        &mut self,
        clips: &PlayerClips,
        visual: &VisualPlayer,
        grounded: bool,
        crouching: bool,
        now: f32,
        dt: f32,
    ) -> Result<AnimatedBody, String> {
        let speed = visual.pose.velocity.truncate().length();
        let eye_yaw = visual.angles.yaw;
        if self.feet_yaw.is_none() || self.revision != visual.revision {
            self.feet_yaw = Some(eye_yaw);
            self.goal_yaw = eye_yaw;
            self.revision = visual.revision;
        }
        // Translated from 0x256460 (decompiled, client.so).
        if visual.pose.velocity.length_squared() > 1.0 {
            self.goal_yaw = eye_yaw;
        } else {
            let difference = angle(self.goal_yaw - eye_yaw);
            if difference.abs() > 45.0 {
                self.goal_yaw = angle(self.goal_yaw - difference.signum() * 45.0);
            }
        }
        let current = self.feet_yaw.unwrap();
        let difference = angle(self.goal_yaw - current);
        let step = (difference.abs() / 60.0).clamp(0.01, 1.0) * 720.0 * dt;
        let yaw = angle(current + difference.clamp(-step, step));
        self.feet_yaw = Some(yaw);
        let aim = [-angle(eye_yaw - yaw), -visual.angles.pitch];
        let seq = if !grounded {
            clips.jump
        } else if crouching {
            if speed < 0.5 {
                clips.crouch
            } else {
                clips.crouch_walk
            }
        } else if speed > 0.5 {
            clips.run
        } else {
            clips.idle
        };
        let direction = visual.pose.velocity.y.atan2(visual.pose.velocity.x) - eye_yaw.to_radians();
        let (sin, cos) = direction.sin_cos();
        let largest = sin.abs().max(cos.abs()).max(f32::EPSILON);
        let full = [-sin / largest, cos / largest];
        let ground_speed = clips.ground_speed(seq, full);
        let amount = if ground_speed > 0.0 {
            (speed / ground_speed).min(1.0)
        } else {
            0.0
        };
        let params = [full[0] * amount, full[1] * amount];
        let changed = self.sequence != Some(seq);
        let eye_height = if seq == clips.crouch || seq == clips.crouch_walk {
            28.0
        } else {
            crate::walker::EYE
        };
        if changed {
            if self.shown.is_empty() {
                self.eye_height = eye_height;
            }
            self.height_from = self.eye_height;
            self.from = self.shown.clone();
            self.transition = 0.0;
            self.cycle = 0.0;
            self.sequence = Some(seq);
        }
        let rate = if speed > 0.5 && ground_speed > 0.001 {
            (speed / ground_speed).clamp(0.01, 10.0)
        } else {
            1.0
        };
        let elapsed = if changed { 0.0 } else { dt };
        self.cycle += elapsed * clips.rate(seq, params) * rate;
        self.cycle = if clips.mdl.sequences[seq].looping {
            self.cycle.fract()
        } else {
            self.cycle.min(1.0)
        };
        let mut pose = clips.sequence(seq, self.cycle, params)?;
        clips.layers(seq, &mut pose, aim)?;
        self.transition += elapsed;
        let fade = clips.mdl.sequences[seq].fade_in;
        self.eye_height = eye_height;
        if !self.from.is_empty() && self.transition < fade {
            let fraction = self.transition / fade;
            let mut previous = self.from.clone();
            mix(&mut previous, &pose, fraction);
            pose = previous;
            self.eye_height = self.height_from + (eye_height - self.height_from) * fraction;
        }
        self.shown = pose.clone();
        if let Some(shot) = self.shot {
            let fire = clips.fire[usize::from(crouching)];
            let cycle = (now - shot) * clips.rate(fire, [0.0; 2]);
            if cycle < 1.0 {
                let delta = clips.sequence(fire, cycle.max(0.0), [0.0; 2])?;
                add(&mut pose, &delta, &clips.mdl.sequences[fire].weights);
            } else {
                self.shot = None;
            }
        }
        let mut bones = Vec::with_capacity(pose.len());
        for (i, p) in pose.iter().enumerate() {
            let local =
                Mat4::from_rotation_translation(Quat::from_array(p.quat), Vec3::from(p.pos));
            bones.push(
                clips.mdl.bones[i]
                    .parent
                    .map_or(local, |parent| bones[parent] * local),
            );
        }
        // Blend stance height with the pose so collision hull changes cannot pop the rig.
        let origin = visual.pose.eye - Vec3::Z * self.eye_height;
        Ok(AnimatedBody { bones, yaw, origin })
    }
}

fn angle(v: f32) -> f32 {
    (v + 180.0).rem_euclid(360.0) - 180.0
}

fn mix(a: &mut [BonePose], b: &[BonePose], weight: f32) {
    for (a, b) in a.iter_mut().zip(b) {
        a.pos = Vec3::from(a.pos).lerp(b.pos.into(), weight).to_array();
        a.quat = blend(a.quat, b.quat, weight);
    }
}

fn add(pose: &mut [BonePose], delta: &[BonePose], weights: &[f32]) {
    for ((p, d), &weight) in pose.iter_mut().zip(delta).zip(weights) {
        p.pos = (Vec3::from(p.pos) + Vec3::from(d.pos) * weight).to_array();
        let q = Quat::IDENTITY.slerp(Quat::from_array(d.quat), weight);
        p.quat = (Quat::from_array(p.quat) * q).normalize().to_array();
    }
}
