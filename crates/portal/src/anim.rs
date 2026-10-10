use crate::error::malformed;
use crate::mdl::{AnimDesc, Bone, Mdl, Reader};
use crate::Result;

const RAWPOS: u8 = 0x01;
const RAWROT: u8 = 0x02;
const ANIMPOS: u8 = 0x04;
const ANIMROT: u8 = 0x08;
const DELTA: u8 = 0x10;
const RAWROT2: u8 = 0x20;
const BONE_FIXED_ALIGNMENT: u32 = 0x0010_0000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BonePose {
    pub pos: [f32; 3],
    pub quat: [f32; 4],
}

/// cycles per second, so one cycle spans frames - 1 intervals
pub fn cycle_rate(anim: &AnimDesc) -> f32 {
    if anim.frames > 1 {
        anim.fps / (anim.frames - 1) as f32
    } else {
        anim.fps
    }
}

/// Local bone poses of an animation at a fractional frame.
pub fn sample(mdl: &Mdl, anim: usize, frame: f32) -> Result<Vec<BonePose>> {
    let desc = mdl
        .anims
        .get(anim)
        .ok_or_else(|| malformed("animation", format!("no animation {anim} in {}", mdl.name)))?;
    if !(0.0..=(desc.frames - 1) as f32).contains(&frame) {
        return Err(malformed(
            "animation",
            format!(
                "frame {frame} outside {} frames of {}",
                desc.frames, desc.name
            ),
        ));
    }
    let (k, s) = (frame.floor() as usize, frame.fract());
    let r = Reader::new(&mdl.bytes, "animation");
    let mut poses: Vec<BonePose> = mdl
        .bones
        .iter()
        .map(|b| BonePose {
            pos: b.pos,
            quat: b.quat,
        })
        .collect();
    let mut at = desc.data;
    loop {
        let [bone, flags] = r.get::<2>(at)?;
        let next = r.i16(at + 2)?;
        let b = mdl
            .bones
            .get(bone as usize)
            .ok_or_else(|| malformed("animation", format!("record for bone {bone} at {at:#x}")))?;
        poses[bone as usize] = BonePose {
            quat: rotation(&r, at + 4, flags, b, k, s)?,
            pos: position(&r, at + 4, flags, b, k, s)?,
        };
        if next == 0 {
            return Ok(poses);
        }
        at = r.offset(at, next as i32)?;
    }
}

fn rotation(r: &Reader, data: usize, flags: u8, b: &Bone, k: usize, s: f32) -> Result<[f32; 4]> {
    if flags & RAWROT != 0 {
        return Ok(quat48(r.get(data)?));
    }
    if flags & RAWROT2 != 0 {
        return Ok(quat64(u64::from_le_bytes(r.get(data)?)));
    }
    if flags & ANIMROT == 0 {
        return Ok(if flags & DELTA != 0 {
            [0.0, 0.0, 0.0, 1.0]
        } else {
            b.quat
        });
    }
    let (mut a1, mut a2) = ([0.0; 3], [0.0; 3]);
    for axis in 0..3 {
        (a1[axis], a2[axis]) = track(r, data, axis, k, b.rot_scale[axis])?;
        if flags & DELTA == 0 {
            a1[axis] += b.rot[axis];
            a2[axis] += b.rot[axis];
        }
    }
    let mut q = if s > 0.001 && a1 != a2 {
        blend(angle_quat(a1), angle_quat(a2), s)
    } else {
        angle_quat(a1)
    };
    if flags & DELTA == 0 && b.flags & BONE_FIXED_ALIGNMENT != 0 {
        q = align(b.alignment, q);
    }
    Ok(q)
}

fn position(r: &Reader, data: usize, flags: u8, b: &Bone, k: usize, s: f32) -> Result<[f32; 3]> {
    if flags & RAWPOS != 0 {
        let at = data
            + if flags & RAWROT != 0 { 6 } else { 0 }
            + if flags & RAWROT2 != 0 { 8 } else { 0 };
        let h: [u8; 6] = r.get(at)?;
        return Ok([0, 2, 4].map(|i| half(u16::from_le_bytes([h[i], h[i + 1]]))));
    }
    if flags & ANIMPOS == 0 {
        return Ok(if flags & DELTA != 0 { [0.0; 3] } else { b.pos });
    }
    let ptr = data + if flags & ANIMROT != 0 { 6 } else { 0 };
    let mut pos = [0.0; 3];
    for (axis, p) in pos.iter_mut().enumerate() {
        let (v1, v2) = track(r, ptr, axis, k, b.pos_scale[axis])?;
        *p = v1 * (1.0 - s) + v2 * s;
        if flags & DELTA == 0 {
            *p += b.pos[axis];
        }
    }
    Ok(pos)
}

/// Decodes one run-length track the way Source's ExtractAnimValue does.
fn track(r: &Reader, ptr: usize, axis: usize, frame: usize, scale: f32) -> Result<(f32, f32)> {
    let offset = r.i16(ptr + 2 * axis)?;
    if offset <= 0 {
        return Ok((0.0, 0.0));
    }
    let mut v = ptr + offset as usize;
    let header = |v: usize| -> Result<(usize, usize)> {
        let [valid, total] = r.get::<2>(v)?;
        Ok((valid as usize, total as usize))
    };
    let value = |v: usize, i: usize| -> Result<f32> { Ok(r.i16(v + 2 * i)? as f32 * scale) };
    let mut k = frame;
    let (mut valid, mut total) = header(v)?;
    while total <= k {
        k -= total;
        v += 2 * (valid + 1);
        (valid, total) = header(v)?;
        if total == 0 {
            return Ok((0.0, 0.0));
        }
    }
    if valid > k {
        let v1 = value(v, k + 1)?;
        let v2 = if valid > k + 1 {
            value(v, k + 2)?
        } else if total > k + 1 {
            v1
        } else {
            value(v, valid + 2)?
        };
        Ok((v1, v2))
    } else {
        let v1 = value(v, valid)?;
        let v2 = if total > k + 1 {
            v1
        } else {
            value(v, valid + 2)?
        };
        Ok((v1, v2))
    }
}

fn quat48(b: [u8; 6]) -> [f32; 4] {
    let x = (u16::from_le_bytes([b[0], b[1]]) as f32 - 32768.0) / 32768.0;
    let y = (u16::from_le_bytes([b[2], b[3]]) as f32 - 32768.0) / 32768.0;
    let zw = u16::from_le_bytes([b[4], b[5]]);
    let z = ((zw & 0x7fff) as f32 - 16384.0) / 16384.0;
    with_w(x, y, z, zw & 0x8000 != 0)
}

fn quat64(bits: u64) -> [f32; 4] {
    let axis = |shift: u32| ((bits >> shift) & 0x1f_ffff) as f32 - 1048576.0;
    with_w(
        axis(0) / 1048576.5,
        axis(21) / 1048576.5,
        axis(42) / 1048576.5,
        bits >> 63 != 0,
    )
}

fn with_w(x: f32, y: f32, z: f32, negative: bool) -> [f32; 4] {
    let w = (1.0 - x * x - y * y - z * z).max(0.0).sqrt();
    [x, y, z, if negative { -w } else { w }]
}

fn half(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = ((h >> 10) & 0x1f) as i32;
    let mant = (h & 0x3ff) as f32;
    sign * match exp {
        0 => mant / 1024.0 * 2f32.powi(-14),
        31 => f32::INFINITY,
        e => (1.0 + mant / 1024.0) * 2f32.powi(e - 15),
    }
}

/// Source's AngleQuaternion for a RadianEuler (x roll, y pitch, z yaw).
pub fn angle_quat(a: [f32; 3]) -> [f32; 4] {
    let (sy, cy) = (a[2] * 0.5).sin_cos();
    let (sp, cp) = (a[1] * 0.5).sin_cos();
    let (sr, cr) = (a[0] * 0.5).sin_cos();
    [
        sr * cp * cy - cr * sp * sy,
        cr * sp * cy + sr * cp * sy,
        cr * cp * sy - sr * sp * cy,
        cr * cp * cy + sr * sp * sy,
    ]
}

fn align(p: [f32; 4], q: [f32; 4]) -> [f32; 4] {
    let d: f32 = p.iter().zip(q).map(|(a, b)| a * b).sum();
    if d < 0.0 {
        q.map(|v| -v)
    } else {
        q
    }
}

pub fn blend(p: [f32; 4], q: [f32; 4], t: f32) -> [f32; 4] {
    let q = align(p, q);
    let mut out = [0.0; 4];
    for i in 0..4 {
        out[i] = p[i] * (1.0 - t) + q[i] * t;
    }
    let n = out.iter().map(|v| v * v).sum::<f32>().sqrt();
    if n > 0.0 {
        out.map(|v| v / n)
    } else {
        out
    }
}
