// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::prelude::*;
use portal::carry::{
    add, carry_pitch, dot, hold_distance, pickup_eligible, sub, throw_impulse, Carry, RigidCube,
};
use portal::pair::{through_dir, through_point, Portal, Vec3 as SourceVec};
use portal::teleport::{entered, in_front, nearer, touching};
use portal::viewbob::angle_vectors;

use crate::faces::Portals;
use crate::player::{Controls, MovementResult, Player};
use crate::scene::Level;
use crate::simulator::Space;
use crate::walker::Walker;

#[derive(Resource)]
pub struct Objects {
    pub cube: RigidCube,
    pub visual_revision: u64,
    pub carry: Option<Carry>,
    pair: Option<[Portal; 2]>,
    safe_position: SourceVec,
    safe_rotation: portal::carry::Basis,
    pub blocked_until: f32,
}

impl Default for Objects {
    fn default() -> Self {
        let position = [-220.0, -80.0, 20.1];
        Self {
            cube: RigidCube::at(position),
            visual_revision: 0,
            carry: None,
            pair: None,
            safe_position: position,
            safe_rotation: portal::carry::IDENTITY,
            blocked_until: 0.0,
        }
    }
}

impl Objects {
    pub fn holding(&self) -> bool {
        self.carry.is_some()
    }

    pub fn push(&mut self, player: &Walker, forward: f32, side: f32) {
        if self.holding() || player.ground == Some(usize::MAX - 1) {
            return;
        }
        let mut direction = add(
            player.forward().map(|v| v * forward),
            player.right().map(|v| v * side),
        );
        direction[2] = 0.0;
        let length = dot(direction, direction).sqrt();
        if length == 0.0 {
            return;
        }
        direction = direction.map(|v| v / length);
        let mut expanded = self.cube.clone();
        expanded.half_size += 16.0;
        if ray_cube(player.centre(), direction, &expanded, 3.0).is_some() {
            let speed = dot(self.cube.velocity, direction);
            if speed < 150.0 {
                self.cube.velocity = add(
                    self.cube.velocity,
                    direction.map(|v| v * (150.0 - speed) * 80.0 / (80.0 + self.cube.mass)),
                );
            }
        }
    }

    pub fn release(&mut self, now: f32) {
        self.carry = None;
        self.blocked_until = now + 0.5;
        info!(
            "[object] release position={:?} velocity={:?}",
            self.cube.position, self.cube.velocity
        );
    }

    pub fn use_object(&mut self, player: &Walker, space: &Space, now: f32) -> bool {
        if self.holding() {
            self.release(now);
            return true;
        }
        if player.ground == Some(usize::MAX - 1)
            || now < self.blocked_until
            || !pickup_eligible(self.cube.mass, [self.cube.half_size * 2.0; 3], true)
        {
            return false;
        }
        let Some(portal) = pickup_ray(player, &self.cube, space) else {
            return false;
        };
        let rotation = match (portal, space.pair) {
            (Some(i), Some(pair)) => self
                .cube
                .rotation
                .map(|v| through_dir(&pair[1 - i], &pair[i], v)),
            _ => self.cube.rotation,
        };
        self.carry = Some(Carry::with_angles(rotation, player.angles(), portal));
        self.pair = space.pair;
        info!(
            "[object] pickup position={:?} portal={portal:?}",
            self.cube.position
        );
        true
    }

    pub fn advance(
        &mut self,
        dt: f32,
        player: &Walker,
        chamber: &crate::chamber::Chamber,
        pair: Option<[Portal; 2]>,
        crossed: Option<usize>,
        now: f32,
    ) {
        let mut space = Space::new(chamber, pair.as_ref());
        if self.pair != pair {
            if self.carry.as_ref().is_some_and(|c| c.portal.is_some()) {
                self.release(now);
            }
            self.cube.owner = None;
            let tr = space.trace_oriented(
                None,
                self.cube.position,
                self.cube.position,
                self.cube.rotation,
                self.cube.half_size,
            );
            if tr.start_solid {
                self.cube.position = self.safe_position;
                self.cube.rotation = self.safe_rotation;
                self.visual_revision += 1;
                self.cube.velocity = [0.0; 3];
                self.cube.angular_velocity = [0.0; 3];
                if self.holding() {
                    self.release(now);
                }
                info!(
                    "[object] recover reason=portal_changed position={:?}",
                    self.cube.position
                );
            }
            self.pair = pair;
        }
        if let (Some(carry), Some(i)) = (&mut self.carry, crossed) {
            carry.player_crossed(i);
            let basis = match (carry.portal, pair) {
                (Some(p), Some(pair)) => self
                    .cube
                    .rotation
                    .map(|v| through_dir(&pair[1 - p], &pair[p], v)),
                _ => self.cube.rotation,
            };
            carry.relative_rotation = portal::carry::relative_rotation(basis, player.angles());
        }
        if !self.holding() {
            space.add_solid(crate::collision::box_planes(
                add(player.feet, [-16.0, -16.0, 0.0]),
                add(player.feet, [16.0, 16.0, player.height()]),
            ));
        }
        let slices = (dt / (1.0 / 120.0)).ceil().clamp(1.0, 16.0) as usize;
        let step = dt / slices as f32;
        let prior_rotation = self.cube.rotation;
        for _ in 0..slices {
            self.own(&space);
            let target = self
                .carry
                .as_ref()
                .and_then(|carry| hold_target(carry, &self.cube, player, &space));
            if let Some((position, rotation)) = target {
                let delta = sub(position, self.cube.position);
                let distance = dot(delta, delta).sqrt();
                let desired = delta.map(|v| v / 0.05);
                let speed = dot(desired, desired).sqrt();
                self.cube.velocity = desired.map(|v| v * (1000.0 / speed.max(1000.0)));
                let previous = self.cube.rotation;
                self.cube.rotation = rotation;
                if space
                    .trace_oriented(
                        self.cube.owner,
                        self.cube.position,
                        self.cube.position,
                        rotation,
                        self.cube.half_size,
                    )
                    .start_solid
                {
                    self.cube.rotation = previous;
                }
                self.cube.angular_velocity =
                    portal::carry::rotation_velocity(prior_rotation, self.cube.rotation, dt);
                let looks = self.carry.as_ref().and_then(|c| c.portal).is_none_or(|i| {
                    pair.is_some_and(|p| {
                        portal_ray(&p[i], player.eye(), player.forward(), 1024.0).is_some()
                    })
                });
                if self
                    .carry
                    .as_mut()
                    .is_some_and(|c| c.update_error(distance, step, looks))
                {
                    self.release(now);
                }
            } else if self.holding() {
                self.release(now);
            }
            if !self.holding() {
                self.cube.velocity[2] -= 600.0 * step;
                self.cube.velocity = self.cube.velocity.map(|v| v * (-0.1 * step).exp());
                let old = self.cube.rotation;
                self.cube.rotate(step);
                if space
                    .trace_oriented(
                        self.cube.owner,
                        self.cube.position,
                        self.cube.position,
                        self.cube.rotation,
                        self.cube.half_size,
                    )
                    .start_solid
                {
                    self.cube.rotation = old;
                    self.cube.angular_velocity = self.cube.angular_velocity.map(|v| v * 0.5);
                }
            }
            let mut remaining = step;
            for _ in 0..4 {
                let goal = add(
                    self.cube.position,
                    self.cube.velocity.map(|v| v * remaining),
                );
                let tr = space.trace_oriented(
                    self.cube.owner,
                    self.cube.position,
                    goal,
                    self.cube.rotation,
                    self.cube.half_size,
                );
                if tr.start_solid {
                    self.cube.velocity = [0.0; 3];
                    break;
                }
                self.cube.position = tr.end;
                if tr.fraction >= 1.0 {
                    break;
                }
                self.cube.contact(tr.normal, 0.25, 0.8);
                let normal_speed = dot(self.cube.velocity, tr.normal);
                if normal_speed.abs() < 600.0 * step {
                    self.cube.velocity =
                        sub(self.cube.velocity, tr.normal.map(|v| v * normal_speed));
                }
                remaining *= 1.0 - tr.fraction;
            }
            if let (Some(i), Some(pair)) = (self.cube.owner, pair) {
                if entered(&pair[i], self.cube.position, self.cube.half()) {
                    self.cube.teleport(&pair[i], &pair[1 - i], 1 - i);
                    self.visual_revision += 1;
                    if let Some(carry) = &mut self.carry {
                        carry.object_crossed(i);
                    }
                    info!(
                        "[object] cross from={i} position={:?} velocity={:?}",
                        self.cube.position, self.cube.velocity
                    );
                }
            }
            let clear = space.trace_oriented(
                None,
                self.cube.position,
                self.cube.position,
                self.cube.rotation,
                self.cube.half_size,
            );
            if !clear.start_solid {
                self.safe_position = self.cube.position;
                self.safe_rotation = self.cube.rotation;
            }
        }
    }

    fn own(&mut self, space: &Space) {
        let Some(pair) = space.pair else {
            self.cube.owner = None;
            return;
        };
        let center = self.cube.position;
        let half = self.cube.half();
        if self
            .cube
            .owner
            .is_some_and(|i| !touching(&pair[i], center, half))
        {
            self.cube.owner = None;
        }
        for i in 0..2 {
            if touching(&pair[i], center, half)
                && in_front(&pair[i], center)
                && self
                    .cube
                    .owner
                    .is_none_or(|old| nearer(center, &pair[i], &pair[old]))
            {
                self.cube.owner = Some(i);
            }
        }
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectsUpdated;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectInteraction;

pub struct ObjectsPlugin;
impl Plugin for ObjectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Objects>()
            .add_systems(
                FixedUpdate,
                push.before(crate::player::simulate)
                    .run_if(crate::menu::playing),
            )
            .add_systems(
                Update,
                interact
                    .in_set(ObjectInteraction)
                    .before(crate::firing::fire)
                    .run_if(crate::menu::playing),
            )
            .add_systems(
                FixedUpdate,
                simulate
                    .in_set(ObjectsUpdated)
                    .after(crate::player::simulate)
                    .run_if(crate::menu::playing),
            );
    }
}

fn push(mut objects: ResMut<Objects>, player: Res<Player>, controls: Res<Controls>) {
    objects.push(&player, controls.forward, controls.side);
}

fn interact(
    mut objects: ResMut<Objects>,
    mut controls: ResMut<Controls>,
    player: Res<Player>,
    level: Res<Level>,
    portals: Res<Portals>,
    time: Res<Time>,
    mut random: Local<crate::random::Xorshift>,
) {
    let now = time.elapsed_secs();
    if objects.holding() && (controls.fire1 || controls.fire2) {
        let throw = controls.fire1;
        controls.fire1 = false;
        controls.fire2 = false;
        let mut forward = player.forward();
        if let (Some(i), Some(pair)) = (
            objects.carry.as_ref().and_then(|c| c.portal),
            portals.linked(),
        ) {
            forward = through_dir(&pair[i], &pair[1 - i], forward);
        }
        objects.release(now);
        if throw {
            let impulse = throw_impulse(objects.cube.mass) / objects.cube.mass;
            objects.cube.velocity = add(objects.cube.velocity, forward.map(|v| v * impulse));
            let torque = portal::carry::throw_torque(
                objects.cube.mass,
                [0, 1, 2].map(|_| random.range(-10.0, 10.0)),
            );
            let inv_inertia = 1.5 / (objects.cube.mass * objects.cube.half_size.powi(2));
            objects.cube.angular_velocity = add(
                objects.cube.angular_velocity,
                torque.map(|v| (v * inv_inertia).to_degrees()),
            );
        }
    }
    if std::mem::take(&mut controls.use_object) {
        let space = Space::new(&level, portals.linked().as_ref());
        objects.use_object(&player, &space, now);
    }
}

fn simulate(
    mut objects: ResMut<Objects>,
    mut player: ResMut<Player>,
    level: Res<Level>,
    portals: Res<Portals>,
    result: Res<MovementResult>,
    time: Res<Time<Fixed>>,
) {
    objects.advance(
        time.delta_secs(),
        &player,
        &level,
        portals.linked(),
        result.0,
        time.elapsed_secs(),
    );
    if objects.holding() {
        player.pitch_reorientation = false;
    }
}

fn hold_target(
    carry: &Carry,
    cube: &RigidCube,
    player: &Walker,
    space: &Space,
) -> Option<(SourceVec, portal::carry::Basis)> {
    let mut angles = player.angles();
    angles.pitch = carry_pitch(angles.pitch);
    let forward = angle_vectors(angles).0;
    let support_direction = match (carry.portal, space.pair) {
        (Some(i), Some(pair)) => through_dir(&pair[i], &pair[1 - i], forward),
        _ => forward,
    };
    let radius = cube.support(support_direction) + (16.0_f32 * 16.0 * 2.0).sqrt();
    let maximum = 24.0 + 2.0 * radius;
    let end = add(player.eye(), forward.map(|v| v * maximum));
    let owner = carry.portal.or(player.owner);
    let tr = space.trace(owner, player.eye(), end, [0.0; 3], [0.0; 3]);
    carry.target_angles(
        player.eye(),
        forward,
        hold_distance(radius, tr.fraction),
        player.angles(),
        space.pair.as_ref(),
    )
}

// Translated from 0x2e52a0 and 0x2e64b0 (decompiled, server.so)
fn pickup_ray(player: &Walker, cube: &RigidCube, space: &Space) -> Option<Option<usize>> {
    if select_frame(
        player.eye(),
        player.forward(),
        player.up(),
        cube,
        space,
        None,
        [player.centre()[2], player.centre()[2] + player.height()],
    ) {
        return Some(None);
    }
    let pair = space.pair.as_ref()?;
    let (i, distance) = (0..2)
        .filter_map(|i| {
            portal_ray(&pair[i], player.eye(), player.forward(), 1024.0).map(|d| (i, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))?;
    let entrance_point = add(player.eye(), player.forward().map(|v| v * distance));
    let entrance = space.trace(None, player.eye(), entrance_point, [0.0; 3], [0.0; 3]);
    let entrance_length = distance;
    if (1.0 - entrance.fraction) * entrance_length > 0.04 {
        return None;
    }
    let eye = through_point(&pair[i], &pair[1 - i], player.eye());
    let forward = through_dir(&pair[i], &pair[1 - i], player.forward());
    let up = through_dir(&pair[i], &pair[1 - i], player.up());
    select_frame(
        eye,
        forward,
        up,
        cube,
        space,
        Some(1 - i),
        [player.centre()[2], player.centre()[2] + player.height()],
    )
    .then_some(Some(i))
}

fn select_frame(
    eye: SourceVec,
    forward: SourceVec,
    up: SourceVec,
    cube: &RigidCube,
    space: &Space,
    owner: Option<usize>,
    vertical: [f32; 2],
) -> bool {
    let cast = |dir: SourceVec, length: f32, hull: f32| {
        let end = add(eye, dir.map(|v| v * length));
        let wall = space.trace(owner, eye, end, [-hull; 3], [hull; 3]);
        let planes: Vec<_> = cube
            .rotation
            .into_iter()
            .flat_map(|axis| {
                [-1.0, 1.0].map(|sign| {
                    let normal = axis.map(|v| v * sign);
                    crate::collision::Plane {
                        normal,
                        distance: dot(normal, cube.position) + cube.half_size,
                    }
                })
            })
            .collect();
        let hit = crate::collision::trace_brushes(eye, end, [-hull; 3], [hull; 3], &[(0, planes)]);
        let z = hit.end[2] - hit.end[2].clamp(vertical[0], vertical[1]);
        let delta = sub(hit.end, eye);
        let distance = (delta[0] * delta[0] + delta[1] * delta[1] + z * z).sqrt();
        hit.block.is_some() && distance < 80.0 && hit.fraction <= wall.fraction + 0.0001
    };
    if cast(forward, 1024.0, 0.0) {
        return true;
    }
    for tangent in [
        1.0,
        0.57735026,
        0.36397022,
        0.2679492,
        0.17632698,
        -0.17632698,
        -0.2679492,
    ] {
        let dir = sub(forward, up.map(|v| v * tangent));
        let length = dot(dir, dir).sqrt();
        if cast(dir.map(|v| v / length), 72.0, 16.0) {
            return true;
        }
    }
    let relative = sub(eye, cube.position);
    let nearest = cube.rotation.iter().fold(cube.position, |point, axis| {
        add(
            point,
            axis.map(|v| v * dot(relative, *axis).clamp(-cube.half_size, cube.half_size)),
        )
    });
    let delta = sub(nearest, eye);
    let length = dot(delta, delta).sqrt();
    if length > 80.0 || length == 0.0 || ((dot(delta, forward) / length) as f64) < 0.8 {
        return false;
    }
    let trace = space.trace(owner, eye, nearest, [0.0; 3], [0.0; 3]);
    trace.fraction >= 0.9999
}

fn portal_ray(portal: &Portal, eye: SourceVec, dir: SourceVec, max: f32) -> Option<f32> {
    let denominator = dot(dir, portal.forward);
    if denominator >= 0.0 {
        return None;
    }
    let distance = -portal.local(eye)[0] / denominator;
    (distance >= 0.0
        && distance <= max
        && portal.inside(portal.local(add(eye, dir.map(|v| v * distance)))))
    .then_some(distance)
}

pub fn ray_cube(eye: SourceVec, dir: SourceVec, cube: &RigidCube, max: f32) -> Option<f32> {
    let relative = sub(eye, cube.position);
    let mut near = 0.0_f32;
    let mut far = max;
    for axis in cube.rotation {
        let origin = dot(relative, axis);
        let direction = dot(dir, axis);
        if direction.abs() < 1e-6 {
            if origin.abs() > cube.half_size {
                return None;
            }
        } else {
            let a = (-cube.half_size - origin) / direction;
            let b = (cube.half_size - origin) / direction;
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return None;
            }
        }
    }
    Some(near)
}
