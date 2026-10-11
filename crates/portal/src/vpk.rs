// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::error::malformed;
use crate::{Error, Result};

const SIGNATURE: u32 = 0x55aa_1234;
const IN_DIR_FILE: u16 = 0x7fff;

struct Entry {
    preload: (usize, usize),
    archive: u16,
    offset: u64,
    len: usize,
}

/// One VPK archive set, opened through its _dir.vpk file.
pub struct Pak {
    dir_path: PathBuf,
    dir: Vec<u8>,
    data_start: u64,
    entries: HashMap<String, Entry>,
}

pub fn normalize(path: &str) -> String {
    path.trim_start_matches(['/', '\\'])
        .replace('\\', "/")
        .to_ascii_lowercase()
}

impl Pak {
    pub fn open(dir_path: &Path) -> Result<Pak> {
        let dir = std::fs::read(dir_path)?;
        let what = dir_path.display().to_string();
        let u32_at = |o: usize| {
            dir.get(o..o + 4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        };
        if u32_at(0) != Some(SIGNATURE) {
            return Err(malformed(&what, "no VPK signature"));
        }
        let header = match u32_at(4) {
            Some(1) => 12,
            Some(2) => 28,
            v => return Err(Error::Unsupported(format!("VPK version {v:?} in {what}"))),
        };
        let tree_len = u32_at(8).unwrap() as usize;
        let entries =
            parse_tree(&dir, header, header + tree_len).map_err(|r| malformed(&what, r))?;
        Ok(Pak {
            dir_path: dir_path.to_path_buf(),
            data_start: (header + tree_len) as u64,
            dir,
            entries,
        })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains(&self, path: &str) -> bool {
        self.entries.contains_key(&normalize(path))
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        let e = self
            .entries
            .get(&normalize(path))
            .ok_or_else(|| Error::Missing(path.to_string()))?;
        let mut out = self.dir[e.preload.0..e.preload.1].to_vec();
        if e.len == 0 {
            return Ok(out);
        }
        let (file, offset) = if e.archive == IN_DIR_FILE {
            (self.dir_path.clone(), self.data_start + e.offset)
        } else {
            (self.archive_path(e.archive), e.offset)
        };
        let mut f = File::open(&file)?;
        f.seek(SeekFrom::Start(offset))?;
        let start = out.len();
        out.resize(start + e.len, 0);
        f.read_exact(&mut out[start..])?;
        Ok(out)
    }

    fn archive_path(&self, index: u16) -> PathBuf {
        let name = self.dir_path.file_name().unwrap().to_string_lossy();
        let stem = name.strip_suffix("_dir.vpk").unwrap_or(&name);
        self.dir_path
            .with_file_name(format!("{stem}_{index:03}.vpk"))
    }
}

fn parse_tree(
    d: &[u8],
    mut o: usize,
    end: usize,
) -> std::result::Result<HashMap<String, Entry>, String> {
    let cstr = |o: &mut usize| -> std::result::Result<String, String> {
        let rest = d.get(*o..end).ok_or("tree runs past the file")?;
        let n = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or("unterminated name")?;
        let s = String::from_utf8_lossy(&rest[..n]).into_owned();
        *o += n + 1;
        Ok(s)
    };
    let mut entries = HashMap::new();
    loop {
        let ext = cstr(&mut o)?;
        if ext.is_empty() {
            return Ok(entries);
        }
        loop {
            let dir = cstr(&mut o)?;
            if dir.is_empty() {
                break;
            }
            loop {
                let name = cstr(&mut o)?;
                if name.is_empty() {
                    break;
                }
                let b = d.get(o..o + 18).ok_or("entry runs past the file")?;
                let u16_at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
                let u32_at = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
                let preload_len = u16_at(4) as usize;
                let preload = (o + 18, o + 18 + preload_len);
                if preload.1 > d.len() {
                    return Err(format!("preload of {name} runs past the file"));
                }
                let full = if dir.trim().is_empty() {
                    format!("{name}.{ext}")
                } else {
                    format!("{dir}/{name}.{ext}")
                };
                entries.insert(
                    normalize(&full),
                    Entry {
                        preload,
                        archive: u16_at(6),
                        offset: u32_at(8) as u64,
                        len: u32_at(12) as usize,
                    },
                );
                o = preload.1;
            }
        }
    }
}

/// Paks searched in order, then loose folders, like a Source game's gameinfo search paths.
pub struct SearchPath {
    paks: Vec<Pak>,
    loose: Vec<PathBuf>,
}

impl SearchPath {
    pub fn new(paks: Vec<Pak>) -> Self {
        SearchPath {
            paks,
            loose: Vec::new(),
        }
    }

    pub fn with_loose(self, loose: Vec<PathBuf>) -> Self {
        SearchPath { loose, ..self }
    }

    fn loose_file(&self, path: &str) -> Option<PathBuf> {
        let path = normalize(path);
        self.loose
            .iter()
            .map(|d| d.join(&path))
            .find(|f| f.is_file())
    }

    pub fn contains(&self, path: &str) -> bool {
        self.paks.iter().any(|p| p.contains(path)) || self.loose_file(path).is_some()
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        if let Some(pak) = self.paks.iter().find(|p| p.contains(path)) {
            return pak.read(path);
        }
        let file = self
            .loose_file(path)
            .ok_or_else(|| Error::Missing(path.to_string()))?;
        std::fs::read(&file).map_err(|e| Error::Missing(format!("{}: {e}", file.display())))
    }
}
