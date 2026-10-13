// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::path::{Path, PathBuf};

use crate::particles::random::sha256;
use crate::vpk::{Pak, SearchPath};
use crate::{Error, Result};

/// Loose folders searched after every pak, as the game mounts them
const LOOSE: [&str; 2] = ["portal", "hl2"];

/// Traced against these files, by sha256. Anything else is a different game as far as we care.
pub const PINNED: [(&str, &str); 6] = [
    (
        "portal/portal_pak_dir.vpk",
        "11ac8d930872d398d975d8f2e3a43fa2c47d2894619b5584efdda7072bde48b9",
    ),
    (
        "hl2/hl2_sound_misc_dir.vpk",
        "3ac73e26801f06d02abaa2601dac9e481613ca53f240bcb57ee52b66af403331",
    ),
    (
        "hl2/hl2_misc_dir.vpk",
        "b2040ee966f0cc2d22be82f81a74908612ee651d3c68e3998b416ec0e253073a",
    ),
    (
        "hl2/hl2_textures_dir.vpk",
        "666339e5a4acabf958dd9f281c2142d94ca061f2b87a687c887c343134822e79",
    ),
    (
        "portal/bin/client.so",
        "00b6e45eb84a93f0974c23d3e2656aa9e32e9ada64443f651899a78cde694aae",
    ),
    (
        "portal/bin/server.so",
        "9079f5dc98add02a0b10755f033353b32fa2d73c98645aca5df27a97a56f30fb",
    ),
];

pub const WINDOWS_PINNED: [(&str, &str); 6] = [
    PINNED[0],
    PINNED[1],
    PINNED[2],
    PINNED[3],
    (
        "portal/bin/client.dll",
        "59abd9eea5c6a6a5a48bd5be3903b66bc97e7d2e1c893ac521d055fcea2da3d5",
    ),
    (
        "portal/bin/server.dll",
        "6e0df352d464b1446ba32418578267ec623a6df2d326cf3265e2500f6f3a6162",
    ),
];

pub fn manifest(root: &Path) -> &'static [(&'static str, &'static str); 6] {
    if root.join("portal/bin/client.so").exists() {
        &PINNED
    } else {
        &WINDOWS_PINNED
    }
}

/// The install root: the argument, then PORTAL_DIR, then Steam's default library.
pub fn root(arg: Option<&str>, env: Option<&str>, home: &Path) -> PathBuf {
    arg.or(env)
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share/Steam/steamapps/common/Portal"))
}

pub struct Install {
    pub root: PathBuf,
    pub search: SearchPath,
    pub client: Vec<u8>,
}

impl Install {
    /// Every pinned file read and checked before anything gets opened.
    pub fn open(root: &Path) -> Result<Install> {
        let mut client = Vec::new();
        for &(file, want) in manifest(root) {
            let bytes = std::fs::read(root.join(file))
                .map_err(|e| Error::Refused(format!("{file} under {}: {e}", root.display())))?;
            let got = hex(&sha256(&bytes));
            if got != want {
                return Err(Error::Refused(format!(
                    "{file} sha256={got} expected={want}"
                )));
            }
            if file.ends_with("client.so") || file.ends_with("client.dll") {
                client = bytes;
            }
        }
        let search = SearchPath::new(
            PINNED
                .iter()
                .filter(|(file, _)| file.ends_with(".vpk"))
                .map(|(file, _)| Pak::open(&root.join(file)))
                .collect::<Result<_>>()?,
        )
        .with_loose(LOOSE.iter().map(|d| root.join(d)).collect());
        Ok(Install {
            root: root.to_path_buf(),
            search,
            client,
        })
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
