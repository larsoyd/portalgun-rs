// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Action {
    Fire1,
    Fire2,
    Jump,
    JumpHeld(bool),
    Crouch(bool),
    /// degrees of yaw and pitch, added at once
    Look(f32, f32),
    /// forward and side input, held until the next move
    Move(f32, f32),
}

/// Timed actions for runs without a player, one per line as seconds, action, arguments.
#[derive(Debug)]
pub struct Script {
    pending: Vec<(f32, Action)>,
}

impl Script {
    pub fn parse(text: &str) -> Result<Script, String> {
        let mut pending = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let at = || format!("line {}: {line}", n + 1);
            let words: Vec<&str> = line.split_whitespace().collect();
            let number = |i: usize| {
                words
                    .get(i)
                    .and_then(|w| w.parse::<f32>().ok())
                    .filter(|v| v.is_finite())
                    .ok_or_else(at)
            };
            let time = number(0).and_then(|t| if t >= 0.0 { Ok(t) } else { Err(at()) })?;
            let action = match words.get(1).copied() {
                Some("fire1") => Action::Fire1,
                Some("fire2") => Action::Fire2,
                Some("jump") => Action::Jump,
                Some("jump-held") => Action::JumpHeld(number(2)? != 0.0),
                Some("crouch") => Action::Crouch(number(2)? != 0.0),
                Some("look") => Action::Look(number(2)?, number(3)?),
                Some("move") => Action::Move(number(2)?, number(3)?),
                _ => return Err(format!("{}, unknown action", at())),
            };
            pending.push((time, action));
        }
        pending.sort_by(|a, b| a.0.total_cmp(&b.0));
        pending.reverse();
        Ok(Script { pending })
    }

    pub fn due(&mut self, now: f32) -> Vec<Action> {
        let mut due = Vec::new();
        while self.pending.last().is_some_and(|&(t, _)| t <= now) {
            due.extend(self.pending.pop().map(|(_, a)| a));
        }
        due
    }
}
