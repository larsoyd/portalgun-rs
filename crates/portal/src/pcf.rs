// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::dmx::{Element, Value};
use crate::error::malformed;
use crate::Result;

const SYSTEM: &str = "DmeParticleSystemDefinition";

/// One renderer, operator, initializer, emitter, force or constraint, by its function name.
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Element,
}

impl Function {
    /// A float parameter, 0 when the file leaves it out as the editor's default does.
    pub fn float(&self, name: &str) -> f32 {
        self.float_or(name, 0.0)
    }

    /// Parameters with the default client.so's parameter table gives when a file leaves one out.
    pub fn float_or(&self, name: &str, default: f32) -> f32 {
        self.params.float(name).unwrap_or(default)
    }

    pub fn int_or(&self, name: &str, default: i32) -> i32 {
        match self.params.get(name) {
            Some(Value::Int(v)) => *v,
            _ => default,
        }
    }

    pub fn bool_or(&self, name: &str, default: bool) -> bool {
        match self.params.get(name) {
            Some(Value::Bool(v)) => *v,
            Some(Value::Int(v)) => *v != 0,
            _ => default,
        }
    }

    pub fn vec3_or(&self, name: &str, default: [f32; 3]) -> [f32; 3] {
        self.params.vec3(name).unwrap_or(default)
    }

    pub fn color_or(&self, name: &str, default: [u8; 4]) -> [u8; 4] {
        match self.params.get(name) {
            Some(Value::Color(c)) => *c,
            _ => default,
        }
    }

    pub fn string_or<'a>(&'a self, name: &str, default: &'a str) -> &'a str {
        match self.params.get(name) {
            Some(Value::String(s)) => s,
            _ => default,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Child {
    /// index into the systems list
    pub system: usize,
    pub delay: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SystemDef {
    pub name: String,
    pub material: String,
    pub max_particles: usize,
    pub initial_particles: usize,
    pub radius: f32,
    pub color: [u8; 4],
    pub max_time_step: f32,
    pub rotation: f32,
    pub rotation_speed: f32,
    pub sequence: i32,
    pub sequence1: i32,
    /// "maximum sim tick rate": the most time simulated while not drawn
    pub max_sim: f32,
    pub min_sim: f32,
    pub min_frames: i32,
    pub renderers: Vec<Function>,
    pub operators: Vec<Function>,
    pub initializers: Vec<Function>,
    pub emitters: Vec<Function>,
    pub forces: Vec<Function>,
    pub constraints: Vec<Function>,
    pub children: Vec<Child>,
}

/// The particle systems of a PCF, in file order, children pointing into the same list.
pub fn systems(elements: &[Element]) -> Result<Vec<SystemDef>> {
    let defs: Vec<usize> = (0..elements.len())
        .filter(|&i| elements[i].kind == SYSTEM)
        .collect();
    defs.iter()
        .map(|&i| system(elements, &defs, &elements[i]))
        .collect()
}

fn refs<'a>(elements: &'a [Element], e: &Element, key: &str) -> Vec<&'a Element> {
    match e.get(key) {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| match v {
                Value::Element(Some(i)) => elements.get(*i),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn system(elements: &[Element], defs: &[usize], e: &Element) -> Result<SystemDef> {
    let int = |k: &str, default: i32| match e.get(k) {
        Some(Value::Int(v)) => *v,
        _ => default,
    };
    let float = |k: &str, default: f32| e.float(k).unwrap_or(default);
    let functions = |k: &str| -> Result<Vec<Function>> {
        refs(elements, e, k)
            .into_iter()
            .map(|f| match f.get("functionName") {
                Some(Value::String(name)) => Ok(Function {
                    name: name.clone(),
                    params: f.clone(),
                }),
                _ => Err(malformed(
                    "PCF",
                    format!("{} in {} has no function name", f.name, e.name),
                )),
            })
            .collect()
    };
    let children = refs(elements, e, "children")
        .into_iter()
        .map(|c| {
            let target = match c.get("child") {
                Some(Value::Element(Some(i))) => defs.iter().position(|d| d == i),
                _ => None,
            };
            let system = target.ok_or_else(|| {
                malformed(
                    "PCF",
                    format!("child {} of {} names no system", c.name, e.name),
                )
            })?;
            Ok(Child {
                system,
                delay: c.float("delay").unwrap_or(0.0),
            })
        })
        .collect::<Result<_>>()?;
    Ok(SystemDef {
        name: e.name.clone(),
        material: match e.get("material") {
            Some(Value::String(s)) => s.clone(),
            _ => "vgui/white".into(),
        },
        max_particles: int("max_particles", 1000).max(0) as usize,
        initial_particles: int("initial_particles", 0).max(0) as usize,
        radius: float("radius", 5.0),
        color: match e.get("color") {
            Some(Value::Color(c)) => *c,
            _ => [255; 4],
        },
        max_time_step: float("maximum time step", 0.1),
        rotation: float("rotation", 0.0),
        rotation_speed: float("rotation_speed", 0.0),
        sequence: int("sequence_number", 0),
        sequence1: int("sequence_number 1", 0),
        max_sim: float("maximum sim tick rate", 0.0),
        min_sim: float("minimum sim tick rate", 0.0),
        min_frames: int("minimum rendered frames", 0),
        renderers: functions("renderers")?,
        operators: functions("operators")?,
        initializers: functions("initializers")?,
        emitters: functions("emitters")?,
        forces: functions("forces")?,
        constraints: functions("constraints")?,
        children,
    })
}
