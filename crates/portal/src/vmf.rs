// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::error::malformed;
use crate::keyvalues::{self, Kv};
use crate::pair::Vec3;
use crate::view::Plane;
use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TexAxis {
    pub axis: Vec3,
    pub shift: f32,
    /// units per texel
    pub scale: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Side {
    pub plane: Plane,
    pub material: String,
    pub u: TexAxis,
    pub v: TexAxis,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Solid {
    pub id: u32,
    pub sides: Vec<Side>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub event: String,
    pub target: String,
    pub input: String,
    pub param: String,
    pub delay: f32,
    /// -1 fires forever
    pub times: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub classname: String,
    pub keys: Vec<(String, String)>,
    pub outputs: Vec<Output>,
    pub solids: Vec<Solid>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Vmf {
    pub world: Vec<Solid>,
    pub entities: Vec<Entity>,
}

impl Entity {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.keys
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    pub fn origin(&self) -> Option<Vec3> {
        self.get("origin").and_then(|v| vec3(v).ok())
    }
}

pub fn parse(text: &str) -> Result<Vmf> {
    let root = keyvalues::parse(text)?;
    let mut vmf = Vmf::default();
    for kv in &root {
        if kv.key.eq_ignore_ascii_case("world") {
            vmf.world.extend(solids(kv)?);
        } else if kv.key.eq_ignore_ascii_case("entity") {
            vmf.entities.push(entity(kv)?);
        }
    }
    Ok(vmf)
}

fn entity(kv: &Kv) -> Result<Entity> {
    let keys: Vec<(String, String)> = kv
        .texts()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let classname = kv
        .get("classname")
        .ok_or_else(|| malformed("vmf", format!("entity {:?} has no classname", kv.get("id"))))?
        .to_string();
    let outputs = kv
        .blocks("connections")
        .flat_map(Kv::texts)
        .map(|(event, v)| output(event, v))
        .collect::<Result<_>>()?;
    Ok(Entity {
        classname,
        keys,
        outputs,
        solids: solids(kv)?,
    })
}

fn output(event: &str, text: &str) -> Result<Output> {
    let fields: Vec<&str> = if text.contains('\u{1b}') {
        text.split('\u{1b}').collect()
    } else {
        text.split(',').collect()
    };
    let bad = || malformed("vmf", format!("output {event} {text:?}"));
    let [target, input, param, delay, times] = fields[..] else {
        return Err(bad());
    };
    Ok(Output {
        event: event.to_string(),
        target: target.to_string(),
        input: input.to_string(),
        param: param.to_string(),
        delay: delay.trim().parse().map_err(|_| bad())?,
        times: times.trim().parse().map_err(|_| bad())?,
    })
}

fn solids(kv: &Kv) -> Result<Vec<Solid>> {
    kv.blocks("solid")
        .map(|s| {
            let id = number(s.get("id").unwrap_or("0"), "solid id")? as u32;
            let sides = s.blocks("side").map(side).collect::<Result<_>>()?;
            Ok(Solid { id, sides })
        })
        .collect()
}

fn side(kv: &Kv) -> Result<Side> {
    let id = kv.get("id").unwrap_or("?");
    let field = |key: &'static str| {
        kv.get(key)
            .ok_or_else(|| malformed("vmf", format!("side {id} has no {key}")))
    };
    Ok(Side {
        plane: plane(field("plane")?).map_err(|e| malformed("vmf", format!("side {id}: {e}")))?,
        material: field("material")?.to_ascii_lowercase(),
        u: tex_axis(field("uaxis")?)?,
        v: tex_axis(field("vaxis")?)?,
    })
}

/// Hammer winds the three points so the normal faces out of the solid
fn plane(text: &str) -> Result<Plane> {
    let points = text
        .split(['(', ')'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(vec3)
        .collect::<Result<Vec<_>>>()?;
    let [a, b, c] = points[..] else {
        return Err(malformed(
            "vmf",
            format!("plane needs three points: {text:?}"),
        ));
    };
    let n = cross(sub(a, b), sub(c, b));
    let len = dot(n, n).sqrt();
    if len < 1e-6 {
        return Err(malformed(
            "vmf",
            format!("plane points are collinear: {text:?}"),
        ));
    }
    let normal = n.map(|x| x / len);
    Ok(Plane {
        normal,
        dist: dot(a, normal),
    })
}

fn tex_axis(text: &str) -> Result<TexAxis> {
    let bad = || malformed("vmf", format!("texture axis {text:?}"));
    let (inner, scale) = text
        .trim()
        .strip_prefix('[')
        .and_then(|t| t.split_once(']'))
        .ok_or_else(bad)?;
    let v = inner
        .split_whitespace()
        .map(|x| x.parse::<f32>().map_err(|_| bad()))
        .collect::<Result<Vec<_>>>()?;
    let [x, y, z, shift] = v[..] else {
        return Err(bad());
    };
    Ok(TexAxis {
        axis: [x, y, z],
        shift,
        scale: scale.trim().parse().map_err(|_| bad())?,
    })
}

fn vec3(text: &str) -> Result<Vec3> {
    let v = text
        .split_whitespace()
        .map(|x| number(x, text))
        .collect::<Result<Vec<_>>>()?;
    match v[..] {
        [x, y, z] => Ok([x, y, z]),
        _ => Err(malformed(
            "vmf",
            format!("expected three numbers: {text:?}"),
        )),
    }
}

fn number(text: &str, what: &str) -> Result<f32> {
    text.trim()
        .parse()
        .map_err(|_| malformed("vmf", format!("bad number {text:?} in {what:?}")))
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
