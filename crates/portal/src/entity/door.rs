// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use super::io::Handle;
use super::{Env, Kind, World};
use crate::pair::Vec3;

const START_OPEN: u32 = 0x1;
const NO_AUTO_RETURN: u32 = 0x20;
const LOCKED: u32 = 0x800;
const SILENT: u32 = 0x1000;
const DEFAULT_SPEED: f32 = 100.0;
const EDGE: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Top,
    Bottom,
    GoingUp,
    GoingDown,
}

#[derive(Debug, Clone)]
pub(super) struct Door {
    state: State,
    pos1: Vec3,
    pos2: Vec3,
    pub(super) origin: Vec3,
    velocity: Vec3,
    final_dest: Vec3,
    speed: f32,
    wait: f32,
    locked: bool,
    local_time: f32,
    move_done: f32,
    blocker: Option<Handle>,
    arrived: String,
    closed: String,
}

/// Translated from 0xa3aef0 (decompiled, server.so)
fn angle_vector(pitch: f32, yaw: f32) -> Vec3 {
    let (p, y) = (
        pitch * (std::f32::consts::PI / 180.0),
        yaw * (std::f32::consts::PI / 180.0),
    );
    [y.cos() * p.cos(), p.cos() * y.sin(), -p.sin()]
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scaled(a: Vec3, k: f32) -> Vec3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

impl Door {
    /// Translated from 0x4e5310 (decompiled, server.so)
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        origin: Vec3,
        movedir: [f32; 2],
        size: Vec3,
        lip: f32,
        speed: f32,
        wait: f32,
        spawnflags: u32,
        sounds: (&str, &str),
    ) -> Door {
        let dir = angle_vector(movedir[0], movedir[1]);
        let reach = (0..3)
            .map(|i| (dir[i] * (size[i] - EDGE)).abs())
            .sum::<f32>()
            - lip;
        let pos2 = [0, 1, 2].map(|i| origin[i] + dir[i] * reach);
        let open = spawnflags & START_OPEN != 0;
        Door {
            state: if open { State::Top } else { State::Bottom },
            pos1: origin,
            pos2,
            origin: if open { pos2 } else { origin },
            velocity: [0.0; 3],
            final_dest: origin,
            speed: if speed == 0.0 { DEFAULT_SPEED } else { speed },
            wait,
            locked: spawnflags & LOCKED != 0,
            local_time: 0.0,
            move_done: -1.0,
            blocker: None,
            arrived: sounds.0.to_string(),
            closed: sounds.1.to_string(),
        }
    }

    /// Translated from 0x999720 (decompiled, server.so)
    fn linear_move(&mut self, dest: Vec3) -> bool {
        self.final_dest = dest;
        if dest == self.origin {
            return true;
        }
        let delta = sub(dest, self.origin);
        let travel =
            (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt() / self.speed;
        self.move_done = self.local_time + travel;
        self.velocity = scaled(delta, 1.0 / travel);
        false
    }
}

fn door(world: &mut World, h: Handle) -> &mut Door {
    match &mut world.slot_mut(h).kind {
        Kind::Door(d) => d,
        _ => unreachable!("door call on another class"),
    }
}

/// Translated from 0x4e4420 and 0x4e4540 (decompiled, server.so)
pub(super) fn input(world: &mut World, h: Handle, input: &str) -> bool {
    let d = door(world, h);
    match input {
        "open" => {
            if !matches!(d.state, State::Top | State::GoingUp) && !d.locked {
                go_up(world, h);
            }
        }
        "close" => {
            if d.state != State::Bottom {
                go_down(world, h);
            }
        }
        _ => return false,
    }
    true
}

/// Translated from 0x4e4050 (decompiled, server.so)
fn go_up(world: &mut World, h: Handle) {
    let d = door(world, h);
    d.state = State::GoingUp;
    let dest = d.pos2;
    if d.linear_move(dest) {
        hit_top(world, h);
    }
    world.fire(h, "onopen", Some(h), 0.0);
}

/// Translated from 0x4e4470 (decompiled, server.so)
fn go_down(world: &mut World, h: Handle) {
    let d = door(world, h);
    d.state = State::GoingDown;
    let dest = d.pos1;
    if d.linear_move(dest) {
        hit_bottom(world, h);
    }
    world.fire(h, "onclose", Some(h), 0.0);
}

/// Translated from 0x4e4d40 (decompiled, server.so)
fn hit_top(world: &mut World, h: Handle) {
    let flags = world.slot(h).spawnflags;
    let d = door(world, h);
    d.state = State::Top;
    let sound = d.arrived.clone();
    let returns = flags & NO_AUTO_RETURN == 0 && d.wait != -1.0;
    if flags & SILENT == 0 {
        world.emit(h, &sound);
    }
    if returns {
        let d = door(world, h);
        d.move_done = d.local_time + d.wait;
    }
    let full = if flags & START_OPEN != 0 {
        "onfullyclosed"
    } else {
        "onfullyopen"
    };
    world.fire(h, full, Some(h), 0.0);
}

/// Translated from 0x4e5020 (decompiled, server.so)
fn hit_bottom(world: &mut World, h: Handle) {
    let flags = world.slot(h).spawnflags;
    let d = door(world, h);
    d.state = State::Bottom;
    let sound = if d.closed.is_empty() {
        d.arrived.clone()
    } else {
        d.closed.clone()
    };
    if flags & SILENT == 0 {
        world.emit(h, &sound);
    }
    let full = if flags & START_OPEN != 0 {
        "onfullyopen"
    } else {
        "onfullyclosed"
    };
    world.fire(h, full, None, 0.0);
}

/// Translated from 0x999870 and 0x49dfc0 (decompiled, server.so), the door's shadow reports arrival
pub(super) fn arrive(world: &mut World, h: Handle) {
    let d = door(world, h);
    if !(d.move_done > 0.0 && d.move_done <= d.local_time) {
        return;
    }
    d.origin = d.final_dest;
    d.velocity = [0.0; 3];
    d.move_done = -1.0;
    match d.state {
        State::GoingUp => hit_top(world, h),
        State::GoingDown => hit_bottom(world, h),
        State::Top => go_down(world, h),
        State::Bottom => {}
    }
}

/// Translated from 0x888220, 0x887bd0 and 0x886b60 (decompiled, server.so)
pub(super) fn push(world: &mut World, h: Handle, env: &mut impl Env) {
    let d = door(world, h);
    if d.move_done < 0.0 {
        return;
    }
    let movetime = (d.move_done - d.local_time).min(World::TICK);
    if movetime <= 0.0 {
        return;
    }
    d.local_time += movetime;
    if d.velocity == [0.0; 3] {
        return;
    }
    let delta = scaled(d.velocity, movetime);
    let blocker = env.push(h, delta);
    let d = door(world, h);
    if blocker.is_some() {
        d.local_time -= movetime;
    } else {
        d.origin = [0, 1, 2].map(|i| d.origin[i] + delta[i]);
    }
    let previous = std::mem::replace(&mut d.blocker, blocker);
    let closing = d.state == State::GoingDown;
    if previous != blocker && previous.is_some() {
        let out = if closing {
            "onunblockedclosing"
        } else {
            "onunblockedopening"
        };
        world.fire(h, out, Some(h), 0.0);
    }
    if previous != blocker && blocker.is_some() {
        let out = if closing {
            "onblockedclosing"
        } else {
            "onblockedopening"
        };
        world.fire(h, out, blocker, 0.0);
    }
}
