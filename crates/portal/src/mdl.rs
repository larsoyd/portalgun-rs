// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use crate::error::malformed;
use crate::{Error, Result};

pub const LOOPING: i32 = 0x1;

#[derive(Debug, Clone, Default)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    pub pos: [f32; 3],
    pub quat: [f32; 4],
    pub rot: [f32; 3],
    pub pos_scale: [f32; 3],
    pub rot_scale: [f32; 3],
    pub pose_to_bone: [[f32; 4]; 3],
    pub alignment: [f32; 4],
    pub flags: u32,
}

#[derive(Debug, Clone, Default)]
pub struct AnimDesc {
    pub name: String,
    pub fps: f32,
    pub flags: i32,
    pub frames: usize,
    /// absolute offset of the first mstudioanim_t record
    pub data: usize,
    pub section_frames: usize,
    pub sections: Vec<usize>,
    pub movement: [f32; 3],
}

#[derive(Debug, Clone, Default)]
pub struct Event {
    pub cycle: f32,
    pub event: i32,
    pub options: String,
}

#[derive(Debug, Clone, Default)]
pub struct Sequence {
    pub label: String,
    pub activity: String,
    pub looping: bool,
    pub anim: usize,
    pub fade_in: f32,
    pub fade_out: f32,
    pub events: Vec<Event>,
    pub flags: i32,
    pub grid: [usize; 2],
    pub blends: Vec<usize>,
    pub param_start: [f32; 2],
    pub param_end: [f32; 2],
    pub weights: Vec<f32>,
    pub layers: Vec<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub material: usize,
    pub vertex_count: usize,
    pub vertex_offset: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Model {
    pub name: String,
    pub vertex_count: usize,
    /// first vertex of this model in the VVD
    pub vertex_index: usize,
    pub meshes: Vec<Mesh>,
}

#[derive(Debug, Clone, Default)]
pub struct Attachment {
    pub name: String,
    pub bone: usize,
    pub local: [[f32; 4]; 3],
}

#[derive(Default)]
pub struct Mdl {
    pub bytes: Vec<u8>,
    pub version: i32,
    pub name: String,
    pub eye_position: [f32; 3],
    pub bones: Vec<Bone>,
    pub anims: Vec<AnimDesc>,
    pub sequences: Vec<Sequence>,
    pub textures: Vec<String>,
    pub cd_textures: Vec<String>,
    pub skins: Vec<Vec<u16>>,
    pub body_parts: Vec<Vec<Model>>,
    pub attachments: Vec<Attachment>,
}

impl Mdl {
    pub fn parse(bytes: Vec<u8>) -> Result<Mdl> {
        let r = Reader::new(&bytes, "MDL");
        if bytes.get(..4) != Some(b"IDST") {
            return Err(malformed("MDL", "no IDST signature"));
        }
        let version = r.i32(4)?;
        if !(44..=48).contains(&version) {
            return Err(Error::Unsupported(format!("MDL version {version}")));
        }
        let mut m = Mdl {
            version,
            name: r.fixed_str(12, 64)?,
            eye_position: r.f32s(80)?,
            ..Mdl::default()
        };
        m.bones = r.table(156, 216, |o| {
            let parent = r.i32(o + 4)?;
            let row = |i: usize| r.f32s::<4>(o + 96 + 16 * i);
            Ok(Bone {
                name: r.name(o, o)?,
                parent: (parent >= 0).then_some(parent as usize),
                pos: r.f32s(o + 32)?,
                quat: r.f32s(o + 44)?,
                rot: r.f32s(o + 60)?,
                pos_scale: r.f32s(o + 72)?,
                rot_scale: r.f32s(o + 84)?,
                pose_to_bone: [row(0)?, row(1)?, row(2)?],
                alignment: r.f32s(o + 144)?,
                flags: r.i32(o + 160)? as u32,
            })
        })?;
        // Translated from 0x2f5ef0 (decompiled, client.so).
        m.anims = r.table(180, 100, |o| {
            if r.i32(o + 52)? != 0 {
                return Err(Error::Unsupported("external animation blocks".into()));
            }
            let frames = r.i32(o + 16)?.max(1) as usize;
            let section_frames = r.i32(o + 84)?.max(0) as usize;
            let sections = if let Some(count) = frames.checked_div(section_frames) {
                r.counted((count + 2) as i32, r.offset(o, r.i32(o + 80)?)?, 8, |s| {
                    if r.i32(s)? != 0 {
                        return Err(Error::Unsupported("external animation section".into()));
                    }
                    r.offset(o, r.i32(s + 4)?)
                })?
            } else {
                Vec::new()
            };
            let movements = r.i32(o + 20)?;
            let movement = if movements > 0 {
                let last = r.offset(o, r.i32(o + 24)?)? + (movements as usize - 1) * 44;
                r.f32s(last + 32)?
            } else {
                [0.0; 3]
            };
            Ok(AnimDesc {
                name: r.name(o, o + 4)?,
                fps: r.f32(o + 8)?,
                flags: r.i32(o + 12)?,
                frames,
                data: r.offset(o, r.i32(o + 56)?)?,
                section_frames,
                sections,
                movement,
            })
        })?;
        m.sequences = r.table(188, 212, |o| {
            let first = r.offset(o, r.i32(o + 60)?)?;
            let events = r.counted(r.i32(o + 24)?, r.offset(o, r.i32(o + 28)?)?, 80, |e| {
                Ok(Event {
                    cycle: r.f32(e)?,
                    event: r.i32(e + 4)?,
                    options: r.fixed_str(e + 12, 64)?,
                })
            })?;
            let grid = [
                r.i32(o + 68)?.max(1) as usize,
                r.i32(o + 72)?.max(1) as usize,
            ];
            let blends = r.counted((grid[0] * grid[1]) as i32, first, 2, |p| {
                Ok(r.i16(p)? as usize)
            })?;
            let weights = if r.i32(o + 156)? != 0 {
                r.counted(
                    m.bones.len() as i32,
                    r.offset(o, r.i32(o + 156)?)?,
                    4,
                    |p| r.f32(p),
                )?
            } else {
                vec![1.0; m.bones.len()]
            };
            let layers = r.counted(r.i32(o + 148)?, r.offset(o, r.i32(o + 152)?)?, 24, |p| {
                Ok(r.i16(p)? as usize)
            })?;
            Ok(Sequence {
                label: r.name(o, o + 4)?,
                activity: r.name(o, o + 8)?,
                looping: r.i32(o + 12)? & LOOPING != 0,
                anim: r.i16(first)? as usize,
                fade_in: r.f32(o + 104)?,
                fade_out: r.f32(o + 108)?,
                events,
                flags: r.i32(o + 12)?,
                grid,
                blends,
                weights,
                layers,
                param_start: r.f32s(o + 84)?,
                param_end: r.f32s(o + 92)?,
            })
        })?;
        m.textures = r.table(204, 64, |o| r.name(o, o))?;
        m.cd_textures = r.table(212, 4, |o| r.cstr(r.offset(0, r.i32(o)?)?))?;
        let (refs, families, skins) = (r.i32(220)?, r.i32(224)?, r.i32(228)? as usize);
        m.skins = r.counted(families, skins, 2 * refs.max(0) as usize, |f| {
            r.counted(refs, f, 2, |i| Ok(r.i16(i)? as u16))
        })?;
        m.body_parts = r.table(232, 16, |p| {
            r.counted(r.i32(p + 4)?, r.offset(p, r.i32(p + 12)?)?, 148, |o| {
                let meshes = r.counted(r.i32(o + 72)?, r.offset(o, r.i32(o + 76)?)?, 116, |e| {
                    Ok(Mesh {
                        material: r.i32(e)? as usize,
                        vertex_count: r.i32(e + 8)? as usize,
                        vertex_offset: r.i32(e + 12)? as usize,
                    })
                })?;
                Ok(Model {
                    name: r.fixed_str(o, 64)?,
                    vertex_count: r.i32(o + 80)? as usize,
                    vertex_index: r.i32(o + 84)? as usize / VERTEX_SIZE,
                    meshes,
                })
            })
        })?;
        m.attachments = r.table(240, 92, |o| {
            let row = |i: usize| r.f32s::<4>(o + 12 + 16 * i);
            Ok(Attachment {
                name: r.name(o, o)?,
                bone: r.i32(o + 8)? as usize,
                local: [row(0)?, row(1)?, row(2)?],
            })
        })?;
        m.bytes = bytes;
        Ok(m)
    }

    pub fn sequence_for(&self, activity: &str) -> Option<usize> {
        self.sequences.iter().position(|s| s.activity == activity)
    }

    pub fn sequence_named(&self, label: &str) -> Option<usize> {
        self.sequences.iter().position(|s| s.label == label)
    }

    pub fn attachment(&self, name: &str) -> Option<usize> {
        self.attachments.iter().position(|a| a.name == name)
    }
}

pub const VERTEX_SIZE: usize = 48;

pub(crate) struct Reader<'a> {
    b: &'a [u8],
    what: &'static str,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(b: &'a [u8], what: &'static str) -> Self {
        Reader { b, what }
    }

    pub(crate) fn get<const N: usize>(&self, o: usize) -> Result<[u8; N]> {
        self.b
            .get(o..o + N)
            .map(|s| s.try_into().unwrap())
            .ok_or_else(|| {
                malformed(
                    self.what,
                    format!(
                        "read of {N} bytes at {o:#x} past the end ({:#x})",
                        self.b.len()
                    ),
                )
            })
    }

    pub(crate) fn i32(&self, o: usize) -> Result<i32> {
        Ok(i32::from_le_bytes(self.get(o)?))
    }

    pub(crate) fn i16(&self, o: usize) -> Result<i16> {
        Ok(i16::from_le_bytes(self.get(o)?))
    }

    pub(crate) fn f32(&self, o: usize) -> Result<f32> {
        Ok(f32::from_le_bytes(self.get(o)?))
    }

    pub(crate) fn f32s<const N: usize>(&self, o: usize) -> Result<[f32; N]> {
        let mut out = [0.0; N];
        for (i, v) in out.iter_mut().enumerate() {
            *v = self.f32(o + 4 * i)?;
        }
        Ok(out)
    }

    pub(crate) fn offset(&self, base: usize, rel: i32) -> Result<usize> {
        let o = base as i64 + rel as i64;
        if o < 0 || o as usize > self.b.len() {
            return Err(malformed(
                self.what,
                format!("offset {rel} from {base:#x} leaves the file"),
            ));
        }
        Ok(o as usize)
    }

    pub(crate) fn cstr(&self, o: usize) -> Result<String> {
        let rest = self.b.get(o..).unwrap_or_default();
        let n = rest
            .iter()
            .position(|&c| c == 0)
            .ok_or_else(|| malformed(self.what, format!("unterminated string at {o:#x}")))?;
        Ok(String::from_utf8_lossy(&rest[..n]).into_owned())
    }

    pub(crate) fn fixed_str(&self, o: usize, len: usize) -> Result<String> {
        let s = self
            .b
            .get(o..o + len)
            .ok_or_else(|| malformed(self.what, format!("string at {o:#x} past the end")))?;
        let n = s.iter().position(|&c| c == 0).unwrap_or(len);
        Ok(String::from_utf8_lossy(&s[..n]).into_owned())
    }

    /// a string stored as an offset at field relative to the struct at base
    pub(crate) fn name(&self, base: usize, field: usize) -> Result<String> {
        self.cstr(self.offset(base, self.i32(field)?)?)
    }

    pub(crate) fn counted<T>(
        &self,
        count: i32,
        start: usize,
        stride: usize,
        f: impl Fn(usize) -> Result<T>,
    ) -> Result<Vec<T>> {
        if count < 0 {
            return Err(malformed(
                self.what,
                format!("negative count {count} at {start:#x}"),
            ));
        }
        (0..count as usize).map(|i| f(start + i * stride)).collect()
    }

    /// a count and absolute index pair in the header
    pub(crate) fn table<T>(
        &self,
        at: usize,
        stride: usize,
        f: impl Fn(usize) -> Result<T>,
    ) -> Result<Vec<T>> {
        let start = self.i32(at + 4)?;
        if start < 0 {
            return Err(malformed(
                self.what,
                format!("negative table offset at {at:#x}"),
            ));
        }
        self.counted(self.i32(at)?, start as usize, stride, f)
    }
}
