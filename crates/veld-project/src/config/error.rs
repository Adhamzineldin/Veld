use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// Errors from loading and validating `veld.toml`. Codes match `spec/config.md`.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("no veld.toml found in `{}` or any parent directory", start.display())]
    NotFound { start: PathBuf },
    #[error("cannot read `{}`: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
    /// Syntax errors, wrong types, missing required keys and other structural problems.
    #[error("E0700: invalid veld.toml: {message}")]
    Invalid { message: String },
    #[error("E0701: unknown key `{key}` in {location}{hint}")]
    UnknownKey {
        location: String,
        key: String,
        suggestion: Option<String>,
        hint: String,
    },
    #[error("E0702: {owner} refers to unknown service `{name}`{hint}")]
    UnknownService {
        owner: String,
        name: String,
        suggestion: Option<String>,
        hint: String,
    },
    #[error(
        "E0703: targets `{first}` and `{second}` have overlapping output directories ({reason})"
    )]
    OutputOverlap {
        first: String,
        second: String,
        reason: &'static str,
    },
    #[error("E0704: target `{target}`: output directory {reason}")]
    OutputForbidden { target: String, reason: String },
    #[error(
        "E0705: plugin `{plugin}` executable hash mismatch (expected {expected}, found {actual})"
    )]
    PluginHashMismatch {
        plugin: String,
        expected: String,
        actual: String,
    },
    #[error("`{}` is a symlink that points outside the package root", path.display())]
    SymlinkEscape { path: PathBuf },
    #[error("`{}` is not valid UTF-8", path.display())]
    NonUtf8Path { path: PathBuf },
}

impl ConfigError {
    /// The stable diagnostic code, if the error has one.
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::Invalid { .. } => Some("E0700"),
            Self::UnknownKey { .. } => Some("E0701"),
            Self::UnknownService { .. } => Some("E0702"),
            Self::OutputOverlap { .. } => Some("E0703"),
            Self::OutputForbidden { .. } => Some("E0704"),
            Self::PluginHashMismatch { .. } => Some("E0705"),
            _ => None,
        }
    }

    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid {
            message: message.into(),
        }
    }
}

/// A non-fatal finding. `W0701`: a service lists itself in `consumes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub code: &'static str,
    pub message: String,
}
