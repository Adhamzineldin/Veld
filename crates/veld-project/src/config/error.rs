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
        "E0703: targets `{first}` and `{second}` have overlapping output directories ({reason}: `{dir}`); set `out` on one of them"
    )]
    OutputOverlap {
        first: String,
        second: String,
        reason: &'static str,
        /// The shared directory, relative to the workspace root.
        dir: String,
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
    #[error("E0709: this veld.toml uses the layout of earlier betas (found {found}); {help}")]
    EarlierLayout { found: String, help: String },
    #[error("E0710: invalid workspace: {message}")]
    Workspace { message: String },
    #[error("E0711: {message}")]
    NotAllowed { message: String },
    #[error("E0803: no member named `{name}` (members: {members})")]
    UnknownMember { name: String, members: String },
    #[error("E0712: cannot read the changes: {message}")]
    Affected { message: String },
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
            Self::EarlierLayout { .. } => Some("E0709"),
            Self::Workspace { .. } => Some("E0710"),
            Self::NotAllowed { .. } => Some("E0711"),
            Self::Affected { .. } => Some("E0712"),
            Self::UnknownMember { .. } => Some("E0803"),
            Self::NotFound { .. }
            | Self::Io { .. }
            | Self::SymlinkEscape { .. }
            | Self::NonUtf8Path { .. } => None,
        }
    }

    pub(crate) fn workspace(message: impl Into<String>) -> Self {
        Self::Workspace {
            message: message.into(),
        }
    }

    pub(crate) fn not_allowed(message: impl Into<String>) -> Self {
        Self::NotAllowed {
            message: message.into(),
        }
    }

    pub fn affected(message: impl Into<String>) -> Self {
        Self::Affected {
            message: message.into(),
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
