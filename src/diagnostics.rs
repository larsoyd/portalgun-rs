// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use bevy::log::{tracing_subscriber, BoxedLayer};
use bevy::prelude::*;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

const LOG_LIMIT: u64 = 8 * 1024 * 1024;

struct Output {
    file: BufWriter<File>,
    path: PathBuf,
    bytes: u64,
    flushed: Instant,
}

#[derive(Resource, Clone)]
pub struct LogFile(Arc<Mutex<Output>>);

impl LogFile {
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let bytes = file.metadata()?.len();
        Ok(Self(Arc::new(Mutex::new(Output {
            file: BufWriter::new(file),
            path: path.to_owned(),
            bytes,
            flushed: Instant::now(),
        }))))
    }

    pub fn flush(&self) -> io::Result<()> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .file
            .flush()
    }

    pub fn layer(app: &mut App) -> Option<BoxedLayer> {
        app.world().get_resource::<Self>().cloned().map(|file| {
            Box::new(
                tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_writer(file),
            ) as BoxedLayer
        })
    }
}

pub struct LogWriter<'a>(MutexGuard<'a, Output>);

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogFile {
    type Writer = LogWriter<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        LogWriter(self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

impl Write for LogWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let out = &mut *self.0;
        if out.bytes >= LOG_LIMIT {
            out.file.flush()?;
            let mut previous = out.path.as_os_str().to_owned();
            previous.push(".previous");
            std::fs::rename(&out.path, previous)?;
            out.file = BufWriter::new(File::create(&out.path)?);
            out.bytes = 0;
        }
        let written = out.file.write(bytes)?;
        out.bytes += written as u64;
        if out.flushed.elapsed().as_secs() >= 1 {
            out.file.flush()?;
            out.flushed = Instant::now();
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.file.flush()
    }
}

pub fn panic_log(file: Option<LogFile>) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        error!("[panic] {panic}");
        if let Some(file) = &file {
            let _ = file.flush();
        }
        previous(panic);
    }));
}

#[derive(Default)]
pub struct Faults(std::collections::HashSet<&'static str>);

impl Faults {
    pub fn raised(&mut self, key: &'static str, failed: bool) -> bool {
        if failed {
            self.0.insert(key)
        } else {
            self.0.remove(key);
            false
        }
    }
}

pub fn invalid_player(player: &crate::walker::Walker) -> Option<&'static str> {
    if !player.feet.iter().all(|v| v.is_finite()) {
        Some("feet")
    } else if !player.velocity.iter().all(|v| v.is_finite()) {
        Some("velocity")
    } else if ![player.yaw, player.pitch, player.roll, player.duck.eye]
        .iter()
        .all(|v| v.is_finite())
    {
        Some("view")
    } else {
        None
    }
}
