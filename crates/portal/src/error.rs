// SPDX-FileCopyrightText: 2026 larsoyd
// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

use std::fmt;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Missing(String),
    Malformed { what: String, reason: String },
    Unsupported(String),
    Refused(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::Missing(path) => write!(f, "not found: {path}"),
            Error::Malformed { what, reason } => write!(f, "malformed {what}: {reason}"),
            Error::Unsupported(what) => write!(f, "unsupported: {what}"),
            Error::Refused(reason) => write!(f, "refused: {reason}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub(crate) fn malformed(what: &str, reason: impl Into<String>) -> Error {
    Error::Malformed {
        what: what.into(),
        reason: reason.into(),
    }
}
