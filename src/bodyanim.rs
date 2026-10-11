// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::prelude::*;
use portal::anim::{blend, cycle_rate, sample, BonePose};
use portal::mdl::Mdl;
use portal::vpk::SearchPath;

use crate::visual::VisualPlayer;
use crate::walker::Walker;

#[derive(Resource)]
pub struct PlayerClips {
    pub mdl: Mdl,
    idle: usize,
    crouch: usize,
    run: usize,
    crouch_walk: usize,
    jump: usize,
    fire: [usize; 2],
    contacts: Vec<[f32; 2]>,
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
        let mut clips = Self {
            idle: seq("idle_PORTALGUN")?,
            crouch: seq("idleCROUCH_PORTALGUN")?,
            run: seq("Run_PORTALGUN")?,
            crouch_walk: seq("Crouch_walk_PORTALGUN")?,
            jump: seq("portalgun_standing_jump")?,
            fire: [seq("stand_fire_PORTALGUN")?, seq("crouch_fire_PORTALGUN")?],
            contacts: vec![[0.0, 0.5]; mdl.anims.len()],
            mdl,
        };
        for seq in [clips.run, clips.crouch_walk] {
            for &anim in &clips.mdl.sequences[seq].blends {
                let frames = clips.mdl.anims[anim].frames - 1;
                let toes = [clips.bone("toe_L"), clips.bone("toe_R")];
                let heights = (0..frames)
                    .map(|frame| {
                        let pose =
                            sample(&clips.mdl, anim, frame as f32).map_err(|e| e.to_string())?;
                        let bones = global_bones(&clips.mdl, &pose);
                        Ok(toes.map(|i| bones[i].w_axis.z))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                for side in 0..2 {
                    let peak = (0..frames)
                        .max_by(|&a, &b| heights[a][side].total_cmp(&heights[b][side]))
                        .unwrap();
                    let low = heights
                        .iter()
                        .map(|h| h[side])
                        .fold(f32::INFINITY, f32::min);
                    if heights[peak][side] - low < 0.5 {
                        continue;
                    }
                    let contact = (1..=frames)
                        .map(|n| (peak + n) % frames)
                        .find(|&n| heights[n][side] <= 3.0_f32.max(low + 0.2))
                        .unwrap();
                    clips.contacts[anim][side] = contact as f32 / frames as f32;
                }
            }
        }
        Ok(clips)
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
        self.sample_sequence(seq, cycle, params, false)
    }

    fn sample_sequence(
        &self,
        seq: usize,
        cycle: f32,
        params: [f32; 2],
        gait: bool,
    ) -> Result<Vec<BonePose>, String> {
        let mut result = Vec::new();
        let mut total = 0.0;
        for (anim, weight) in self.weights(seq, params) {
            let cycle = if gait {
                let [left, right] = self.contacts[anim];
                let span = (right - left).rem_euclid(1.0);
                let phase = cycle.rem_euclid(1.0);
                if phase < 0.5 {
                    (left + span * phase * 2.0).fract()
                } else {
                    (right + (1.0 - span) * (phase - 0.5) * 2.0).fract()
                }
            } else {
                cycle
            };
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

#[derive(Clone, Copy, Default)]
pub struct Stance {
    pub grounded: bool,
    pub ducked: bool,
    pub ducking: bool,
    /// jump events raised so far
    pub jumps: u64,
}

impl Stance {
    pub fn new(grounded: bool, ducked: bool) -> Self {
        Self {
            grounded,
            ducked,
            ..default()
        }
    }

    pub fn of(player: &Walker) -> Self {
        Self {
            grounded: player.grounded,
            ducked: player.duck.ducked,
            ducking: player.duck.ducking,
            jumps: player.jumps,
        }
    }
}

#[derive(Resource, Default)]
pub struct BodyAnimation {
    pub original_airwalk: bool,
    pub original_feet: bool,
    pub original_gait: bool,
    sequence: Option<usize>,
    cycle: f32,
    from: Vec<BonePose>,
    shown: Vec<BonePose>,
    transition: f32,
    pub footfall: Option<Footfall>,
    shot: Option<f32>,
    feet_yaw: Option<f32>,
    goal_yaw: f32,
    revision: u64,
    jumps: u64,
    jumping: bool,
    first_jump_frame: bool,
    jump_start: f32,
    airwalk: bool,
    restart: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Activity {
    Idle,
    CrouchIdle,
    Run,
    CrouchWalk,
    JumpStart,
    Airwalk,
}

/// the sequence an activity the model lacks falls back to
const FALLBACK: usize = 0;
const STUDIO_DELTA: i32 = 0x4;

#[derive(Clone, Copy)]
pub struct Footfall {
    pub left: bool,
    pub at: f32,
    pub interval: f32,
}

#[derive(Resource, Default)]
pub struct Gait(pub Option<Footfall>);

pub struct AnimatedBody {
    pub bones: Vec<Mat4>,
    pub yaw: f32,
    pub origin: Vec3,
    pub planted: Option<usize>,
    /// the original has no foot IK, DONTUSEIK set at 0x53f873 (disassembly, client.so)
    pub foot_ik: bool,
    pub sequence: usize,
    pub gesture: Option<usize>,
}

impl BodyAnimation {
    pub fn fire(&mut self, now: f32) {
        self.shot = Some(now);
    }

    // Translated from 0x282dc0 (decompiled, client.so)
    fn jump_activity(&mut self, rise: f32, grounded: bool, now: f32) -> Option<Activity> {
        let mut activity = Activity::Idle;
        if rise > 300.0 || self.airwalk {
            if !grounded {
                self.airwalk = true;
                return Some(Activity::Airwalk);
            }
            self.airwalk = false;
            self.restart = true;
        } else {
            if !self.jumping {
                return None;
            }
            if self.first_jump_frame {
                self.first_jump_frame = false;
                self.restart = true;
            } else if now - self.jump_start > 0.2 && grounded {
                self.jumping = false;
                self.restart = true;
            }
            if self.jumping {
                activity = Activity::JumpStart;
            }
        }
        (self.jumping || self.airwalk).then_some(activity)
    }

    fn main_sequence(&self, clips: &PlayerClips, activity: Activity) -> usize {
        match activity {
            Activity::Idle => clips.idle,
            Activity::CrouchIdle => clips.crouch,
            Activity::Run => clips.run,
            Activity::CrouchWalk => clips.crouch_walk,
            Activity::JumpStart => clips.jump,
            Activity::Airwalk if self.original_airwalk => FALLBACK,
            Activity::Airwalk => clips.jump,
        }
    }

    pub fn pose(
        &mut self,
        clips: &PlayerClips,
        visual: &VisualPlayer,
        stance: Stance,
        now: f32,
        dt: f32,
    ) -> Result<AnimatedBody, String> {
        let grounded = stance.grounded;
        let speed = visual.pose.velocity.truncate().length();
        let eye_yaw = visual.angles.yaw;
        let relocated = self.revision != visual.revision;
        if self.feet_yaw.is_none() || relocated {
            self.feet_yaw = Some(eye_yaw);
            self.goal_yaw = eye_yaw;
            self.revision = visual.revision;
        }
        // Translated from 0x256460 (decompiled, client.so), the bound is the float after 1 at 0x7f7878
        if visual.pose.velocity.length_squared() > 1.0 + f32::EPSILON {
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
        if self.sequence.is_none() {
            self.jumps = stance.jumps;
        }
        if self.jumps != stance.jumps {
            // event 6 in 0x250a30 (decompiled, client.so)
            self.jumps = stance.jumps;
            self.jumping = true;
            self.first_jump_frame = true;
            self.jump_start = now;
            self.restart = true;
        }
        // Translated from 0x250530, 0x2830c0 and 0x283020 (decompiled, client.so)
        let activity = self
            .jump_activity(visual.pose.velocity.z, grounded, now)
            .unwrap_or(match (stance.ducked || stance.ducking, speed > 0.5) {
                (true, _) if speed < 0.5 => Activity::CrouchIdle,
                (true, _) => Activity::CrouchWalk,
                (false, true) => Activity::Run,
                (false, false) => Activity::Idle,
            });
        let seq = self.main_sequence(clips, activity);
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
        if changed {
            self.from = self.shown.clone();
            self.transition = 0.0;
            self.cycle = 0.0;
            self.sequence = Some(seq);
        } else if self.restart {
            self.cycle = 0.0;
        }
        self.restart = false;
        let elapsed = if changed { 0.0 } else { dt };
        self.cycle += elapsed * clips.rate(seq, params);
        self.cycle = if clips.mdl.sequences[seq].looping {
            self.cycle.fract()
        } else {
            self.cycle.min(1.0)
        };
        let gait = self
            .footfall
            .filter(|_| !self.original_gait && (seq == clips.run || seq == clips.crouch_walk));
        let cycle = gait.map_or(self.cycle, |step| {
            let progress = ((now - step.at) / step.interval).clamp(0.0, 1.0);
            (if step.left { 0.0 } else { 0.5 }) + progress * 0.5
        });
        let mut pose = if clips.mdl.sequences[seq].flags & STUDIO_DELTA != 0 {
            let mut bind: Vec<_> = clips
                .mdl
                .bones
                .iter()
                .map(|b| BonePose {
                    pos: b.pos,
                    quat: b.quat,
                })
                .collect();
            let delta = clips.sequence(seq, cycle, [0.0; 2])?;
            add(&mut bind, &delta, &clips.mdl.sequences[seq].weights);
            bind
        } else {
            clips.sample_sequence(seq, cycle, params, gait.is_some())?
        };
        clips.layers(seq, &mut pose, aim)?;
        self.transition += elapsed;
        let fade = clips.mdl.sequences[seq].fade_in;
        if !self.from.is_empty() && self.transition < fade {
            let fraction = self.transition / fade;
            let mut previous = self.from.clone();
            mix(&mut previous, &pose, fraction);
            pose = previous;
        }
        self.shown = pose.clone();
        let mut gesture = None;
        if let Some(shot) = self.shot {
            // FL_DUCKING in 0x283140 (decompiled, client.so) is set and cleared with m_bDucked
            let fire = clips.fire[usize::from(stance.ducked)];
            let cycle = (now - shot) * clips.rate(fire, [0.0; 2]);
            if cycle < 1.0 {
                gesture = Some(fire);
                let delta = clips.sequence(fire, cycle.max(0.0), [0.0; 2])?;
                add(&mut pose, &delta, &clips.mdl.sequences[fire].weights);
            } else {
                self.shot = None;
            }
        }
        let bones = global_bones(&clips.mdl, &pose);
        // Translated from 0x340080, 0x362790 and 0x361d00 (decompiled, client.so)
        let origin = visual.pose.centre - Vec3::Z * visual.pose.half.z;
        let planted = self
            .footfall
            .filter(|step| grounded && now - step.at < step.interval * 0.15)
            .map(|step| usize::from(!step.left));
        Ok(AnimatedBody {
            bones,
            yaw,
            origin,
            planted,
            foot_ik: !self.original_feet,
            sequence: seq,
            gesture,
        })
    }
}

fn global_bones(mdl: &Mdl, pose: &[BonePose]) -> Vec<Mat4> {
    let mut bones = Vec::with_capacity(pose.len());
    for (i, p) in pose.iter().enumerate() {
        let local = Mat4::from_rotation_translation(Quat::from_array(p.quat), Vec3::from(p.pos));
        bones.push(
            mdl.bones[i]
                .parent
                .map_or(local, |parent| bones[parent] * local),
        );
    }
    bones
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
