// reads back only what to_dds writes: block formats and masked uncompressed pixels

#[derive(Debug, Clone, PartialEq)]
pub enum Format {
    Bc1,
    Bc2,
    Bc3 { premultiplied: bool },
    Masked { bits: u32, masks: [u32; 4] },
}

pub struct Dds {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
    mips: usize,
    format: Format,
}

const LUMINANCE: u32 = 0x2_0000;

fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

impl Dds {
    pub fn parse(bytes: Vec<u8>) -> Result<Dds, String> {
        if bytes.len() < 128 || &bytes[..4] != b"DDS " {
            return Err(format!("not a dds, len={}", bytes.len()));
        }
        let format = match (word(&bytes, 80) & 0x4 != 0, &bytes[84..88]) {
            (true, b"DXT1") => Format::Bc1,
            (true, b"DXT3") => Format::Bc2,
            (true, b"DXT5") => Format::Bc3 {
                premultiplied: false,
            },
            (true, cc) => return Err(format!("fourcc={cc:?}")),
            _ => Format::Masked {
                bits: word(&bytes, 88),
                masks: [92, 96, 100, 104].map(|at| word(&bytes, at)),
            },
        };
        let luminance = word(&bytes, 80) & LUMINANCE != 0;
        let format = match format {
            Format::Masked { bits, masks } if luminance => Format::Masked {
                bits,
                masks: [masks[0], masks[0], masks[0], masks[3]],
            },
            f => f,
        };
        Ok(Dds {
            width: word(&bytes, 16),
            height: word(&bytes, 12),
            mips: word(&bytes, 28).max(1) as usize,
            format,
            bytes,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn mip_count(&self) -> usize {
        self.mips
    }

    pub fn format(&self) -> Format {
        self.format.clone()
    }

    fn size(&self, level: usize) -> usize {
        let (w, h) = ((self.width >> level).max(1), (self.height >> level).max(1));
        match self.format {
            Format::Bc1 => (w.div_ceil(4) * h.div_ceil(4) * 8) as usize,
            Format::Bc2 | Format::Bc3 { .. } => (w.div_ceil(4) * h.div_ceil(4) * 16) as usize,
            Format::Masked { bits, .. } => (w * h * bits / 8) as usize,
        }
    }

    pub fn level_data(&self, face: usize, level: usize) -> Option<&[u8]> {
        assert_eq!(face, 0, "no cube maps");
        let at = 128 + (0..level).map(|l| self.size(l)).sum::<usize>();
        self.bytes.get(at..at + self.size(level))
    }

    pub fn decode_rgba(&self, face: usize, level: usize) -> Option<Vec<u8>> {
        let Format::Masked { bits, masks } = self.format else {
            return None;
        };
        let step = (bits / 8) as usize;
        let channel = |px: u32, mask: u32| match mask {
            0 => 255,
            m => ((px & m) >> m.trailing_zeros()) as u8,
        };
        let data = self.level_data(face, level)?;
        Some(
            data.chunks(step)
                .flat_map(|c| {
                    let px = c.iter().rev().fold(0u32, |a, &b| a << 8 | b as u32);
                    masks.map(|m| channel(px, m))
                })
                .collect(),
        )
    }
}
