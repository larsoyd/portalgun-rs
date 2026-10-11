// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::error::malformed;
use crate::mdl::Reader;
use crate::{Error, Result};

const HEADER: &[u8] = b"<!-- dmx encoding binary 2 ";
const ARRAYS: u8 = 14;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Element(Option<usize>),
    Int(i32),
    Float(f32),
    Bool(bool),
    String(String),
    Binary(Vec<u8>),
    Time(f32),
    Color([u8; 4]),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Angle([f32; 3]),
    Quat([f32; 4]),
    Matrix([f32; 16]),
    Array(Vec<Value>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub kind: String,
    pub name: String,
    pub attributes: Vec<(String, Value)>,
}

impl Element {
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.attributes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v)
    }

    pub fn float(&self, name: &str) -> Option<f32> {
        match self.get(name)? {
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn vec3(&self, name: &str) -> Option<[f32; 3]> {
        match self.get(name)? {
            Value::Vec3(v) => Some(*v),
            _ => None,
        }
    }
}

struct Cursor<'a> {
    r: Reader<'a>,
    b: &'a [u8],
    at: usize,
}

impl Cursor<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N]> {
        let v = self.r.get::<N>(self.at)?;
        self.at += N;
        Ok(v)
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take()?))
    }

    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take()?))
    }

    fn f32s<const N: usize>(&mut self) -> Result<[f32; N]> {
        let mut out = [0.0; N];
        for v in out.iter_mut() {
            *v = f32::from_le_bytes(self.take()?);
        }
        Ok(out)
    }

    fn cstr(&mut self) -> Result<String> {
        let s = self.r.cstr(self.at)?;
        self.at += s.len() + 1;
        Ok(s)
    }

    fn value(&mut self, kind: u8) -> Result<Value> {
        Ok(match kind {
            1 => match self.i32()? {
                -1 => Value::Element(None),
                i if i >= 0 => Value::Element(Some(i as usize)),
                i => return Err(Error::Unsupported(format!("DMX element reference {i}"))),
            },
            2 => Value::Int(self.i32()?),
            3 => Value::Float(self.f32s::<1>()?[0]),
            4 => Value::Bool(self.take::<1>()?[0] != 0),
            5 => Value::String(self.cstr()?),
            6 => {
                let len = self.i32()?.max(0) as usize;
                let data = self
                    .b
                    .get(self.at..self.at + len)
                    .ok_or_else(|| malformed("DMX", "binary past the end"))?;
                self.at += len;
                Value::Binary(data.to_vec())
            }
            7 => Value::Time(self.i32()? as f32 / 10_000.0),
            8 => Value::Color(self.take()?),
            9 => Value::Vec2(self.f32s()?),
            10 => Value::Vec3(self.f32s()?),
            11 => Value::Vec4(self.f32s()?),
            12 => Value::Angle(self.f32s()?),
            13 => Value::Quat(self.f32s()?),
            14 => Value::Matrix(self.f32s()?),
            k if k > ARRAYS && k <= 2 * ARRAYS => {
                let count = self.i32()?;
                if count < 0 {
                    return Err(malformed(
                        "DMX",
                        format!("array of {count} at {:#x}", self.at),
                    ));
                }
                Value::Array(
                    (0..count)
                        .map(|_| self.value(k - ARRAYS))
                        .collect::<Result<_>>()?,
                )
            }
            k => {
                return Err(malformed(
                    "DMX",
                    format!("attribute type {k} at {:#x}", self.at),
                ))
            }
        })
    }
}

fn references(v: &Value) -> Vec<usize> {
    match v {
        Value::Element(Some(i)) => vec![*i],
        Value::Array(a) => a.iter().flat_map(references).collect(),
        _ => Vec::new(),
    }
}

/// A binary version 2 DMX file, as Portal's particle files are stored.
pub fn parse(bytes: &[u8]) -> Result<Vec<Element>> {
    if !bytes.starts_with(HEADER) {
        return Err(Error::Unsupported(
            "DMX that is not binary encoding 2".into(),
        ));
    }
    let header_end = bytes
        .iter()
        .position(|&b| b == b'\n')
        .ok_or_else(|| malformed("DMX", "no header line"))?;
    let mut c = Cursor {
        r: Reader::new(bytes, "DMX"),
        b: bytes,
        at: header_end + 2,
    };
    let strings = (0..c.u16()?)
        .map(|_| c.cstr())
        .collect::<Result<Vec<_>>>()?;
    let string = |i: u16| {
        strings
            .get(i as usize)
            .cloned()
            .ok_or_else(|| malformed("DMX", format!("string {i} of {}", strings.len())))
    };
    let count = c.i32()?;
    if count < 0 {
        return Err(malformed("DMX", format!("{count} elements")));
    }
    let mut elements = Vec::new();
    for _ in 0..count {
        let kind = string(c.u16()?)?;
        let name = c.cstr()?;
        c.take::<16>()?;
        elements.push(Element {
            kind,
            name,
            attributes: Vec::new(),
        });
    }
    for e in elements.iter_mut() {
        for _ in 0..c.i32()?.max(0) {
            let name = string(c.u16()?)?;
            let kind = c.take::<1>()?[0];
            let value = c.value(kind)?;
            e.attributes.push((name, value));
        }
    }
    let n = elements.len();
    if let Some(bad) = elements
        .iter()
        .flat_map(|e| e.attributes.iter().flat_map(|(_, v)| references(v)))
        .find(|&i| i >= n)
    {
        return Err(malformed("DMX", format!("element {bad} of {n}")));
    }
    Ok(elements)
}
