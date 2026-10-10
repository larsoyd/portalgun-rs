use crate::error::malformed;
use crate::mdl::Reader;
use crate::{Error, Result};

const ENVMAP: u32 = 0x4000;
const HIGH_RES: [u8; 3] = [0x30, 0, 0];
const SHEET: [u8; 3] = [0x10, 0, 0];

pub struct Vtf {
    pub width: u32,
    pub height: u32,
    pub format: i32,
    pub mips: usize,
    pub flags: u32,
    data: usize,
}

enum Layout {
    Block(u32, [u8; 4]),
    /// bits, pixel format flags, r g b a masks
    Masks(u32, u32, [u32; 4]),
}

const RGB: u32 = 0x40;
const ALPHA_PIXELS: u32 = 0x1;
const LUMINANCE: u32 = 0x2_0000;

fn layout(format: i32) -> Result<Layout> {
    Ok(match format {
        0 => Layout::Masks(
            32,
            RGB | ALPHA_PIXELS,
            [0xff, 0xff00, 0xff_0000, 0xff00_0000],
        ),
        1 => Layout::Masks(
            32,
            RGB | ALPHA_PIXELS,
            [0xff00_0000, 0xff_0000, 0xff00, 0xff],
        ),
        2 => Layout::Masks(24, RGB, [0xff, 0xff00, 0xff_0000, 0]),
        3 => Layout::Masks(24, RGB, [0xff_0000, 0xff00, 0xff, 0]),
        5 => Layout::Masks(8, LUMINANCE, [0xff, 0, 0, 0]),
        6 => Layout::Masks(16, LUMINANCE | ALPHA_PIXELS, [0xff, 0, 0, 0xff00]),
        12 => Layout::Masks(
            32,
            RGB | ALPHA_PIXELS,
            [0xff_0000, 0xff00, 0xff, 0xff00_0000],
        ),
        16 => Layout::Masks(32, RGB, [0xff_0000, 0xff00, 0xff, 0]),
        13 => Layout::Block(8, *b"DXT1"),
        14 => Layout::Block(16, *b"DXT3"),
        15 => Layout::Block(16, *b"DXT5"),
        other => return Err(Error::Unsupported(format!("VTF image format {other}"))),
    })
}

fn level_size(layout: &Layout, w: u32, h: u32) -> usize {
    match layout {
        Layout::Block(block, _) => (w.div_ceil(4).max(1) * h.div_ceil(4).max(1) * block) as usize,
        Layout::Masks(bits, _, _) => (w * h * bits / 8) as usize,
    }
}

fn level_dims(w: u32, h: u32, level: usize) -> (u32, u32) {
    ((w >> level).max(1), (h >> level).max(1))
}

impl Vtf {
    pub fn parse(bytes: &[u8]) -> Result<Vtf> {
        let r = Reader::new(bytes, "VTF");
        if bytes.get(..4) != Some(b"VTF\0") || r.i32(4)? != 7 {
            return Err(malformed("VTF", "no VTF 7.x signature"));
        }
        let minor = r.i32(8)?;
        let [w0, w1, h0, h1] = r.get::<4>(16)?;
        let (width, height) = (
            u16::from_le_bytes([w0, w1]) as u32,
            u16::from_le_bytes([h0, h1]) as u32,
        );
        let low = r.i32(57)?;
        let [low_w, low_h] = r.get::<2>(61)?;
        let data = if minor >= 3 {
            let resources =
                r.counted(r.i32(68)?, 80, 8, |o| Ok((r.get::<3>(o)?, r.i32(o + 4)?)))?;
            let (_, at) = resources
                .iter()
                .find(|(tag, _)| *tag == HIGH_RES)
                .ok_or_else(|| malformed("VTF", "no high resolution image resource"))?;
            *at as usize
        } else {
            let thumb = if low == -1 {
                0
            } else {
                level_size(&layout(low)?, low_w as u32, low_h as u32)
            };
            r.i32(12)? as usize + thumb
        };
        Ok(Vtf {
            width,
            height,
            format: r.i32(52)?,
            mips: r.get::<1>(56)?[0].max(1) as usize,
            flags: r.i32(20)? as u32,
            data,
        })
    }
}

/// The sprite sheet resource (tag 0x10) of a 7.3 VTF, if it has one.
pub fn sheet(bytes: &[u8]) -> Result<Option<&[u8]>> {
    let r = Reader::new(bytes, "VTF");
    if bytes.get(..4) != Some(b"VTF\0") || r.i32(4)? != 7 || r.i32(8)? < 3 {
        return Ok(None);
    }
    let resources = r.counted(r.i32(68)?, 80, 8, |o| Ok((r.get::<3>(o)?, r.i32(o + 4)?)))?;
    let Some((_, at)) = resources.iter().find(|(tag, _)| *tag == SHEET) else {
        return Ok(None);
    };
    let at = *at as usize;
    let size = r.i32(at)? as usize;
    bytes
        .get(at + 4..at + 4 + size)
        .map(Some)
        .ok_or_else(|| malformed("VTF", format!("sheet of {size} bytes runs past the file")))
}

/// The first frame of a VTF as a DDS file with its full mip chain.
pub fn to_dds(bytes: &[u8]) -> Result<Vec<u8>> {
    let v = Vtf::parse(bytes)?;
    if v.flags & ENVMAP != 0 {
        return Err(Error::Unsupported("VTF cube maps".into()));
    }
    let layout = layout(v.format)?;
    let frames = Reader::new(bytes, "VTF").get::<2>(24)?;
    let frames = u16::from_le_bytes(frames).max(1) as usize;
    let mut levels = vec![Vec::new(); v.mips];
    let mut at = v.data;
    for level in (0..v.mips).rev() {
        let (w, h) = level_dims(v.width, v.height, level);
        let size = level_size(&layout, w, h);
        let img = bytes.get(at..at + size).ok_or_else(|| {
            malformed(
                "VTF",
                format!(
                    "mip {level} ({w}x{h}) at {at:#x} past the end ({:#x})",
                    bytes.len()
                ),
            )
        })?;
        levels[level] = img.to_vec();
        at += size * frames;
    }
    Ok(dds_file(&v, &layout, &levels.concat()))
}

fn dds_file(v: &Vtf, layout: &Layout, data: &[u8]) -> Vec<u8> {
    let mut h = vec![0u8; 128];
    let mut put = |at: usize, x: u32| h[at..at + 4].copy_from_slice(&x.to_le_bytes());
    put(0, u32::from_le_bytes(*b"DDS "));
    put(4, 124);
    put(8, 0x1 | 0x2 | 0x4 | 0x1000 | 0x2_0000);
    put(12, v.height);
    put(16, v.width);
    put(28, v.mips as u32);
    put(76, 32);
    match layout {
        Layout::Block(_, four_cc) => {
            put(80, 0x4);
            put(84, u32::from_le_bytes(*four_cc));
        }
        Layout::Masks(bits, flags, masks) => {
            put(80, *flags);
            put(88, *bits);
            for (i, m) in masks.iter().enumerate() {
                put(92 + 4 * i, *m);
            }
        }
    }
    put(108, 0x1000 | 0x40_0000 | 0x8);
    h.extend_from_slice(data);
    h
}
