use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

/// Errors from [`write`](super::write) and [`clean`](super::clean).
#[derive(Debug, Error)]
pub enum OutputError {
    #[error("I/O error at `{}`: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
    #[error("{}", conflict_message(foreign, edited))]
    Conflicts {
        /// Existing files Veld did not create.
        foreign: Vec<String>,
        /// Files Veld generated earlier that the user has edited since.
        edited: Vec<String>,
    },
    #[error("refusing to write through symlink `{}`", path.display())]
    Symlink { path: PathBuf },
    #[error("cannot write `{path}`: {reason}")]
    Blocked { path: String, reason: &'static str },
    #[error("output path `{}` exists but is not a directory", path.display())]
    OutNotDirectory { path: PathBuf },
    #[error("invalid manifest `{}`: {reason}", path.display())]
    ManifestInvalid { path: PathBuf, reason: String },
    #[error("output path `{path}` escapes the output directory")]
    PathEscapes { path: String },
    #[error("no content staged for `{path}`")]
    MissingContent { path: String },
    #[error("internal error: {0}")]
    Internal(String),
    #[error("write failed ({cause}) and rollback was incomplete; previous files are kept in `{}`", staging.display())]
    RollbackIncomplete { cause: String, staging: PathBuf },
}

/// Builds an `Io` error mapper for `path`.
pub(crate) fn io_at(path: &Path) -> impl FnOnce(io::Error) -> OutputError + '_ {
    move |source| OutputError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn conflict_message(foreign: &[String], edited: &[String]) -> String {
    let mut parts = Vec::new();
    if !edited.is_empty() {
        parts.push(format!(
            "you edited generated file(s) {}; generating would overwrite or delete your changes",
            edited.join(", ")
        ));
    }
    if !foreign.is_empty() {
        parts.push(format!(
            "{} existing file(s) were not created by Veld: {}",
            foreign.len(),
            foreign.join(", ")
        ));
    }
    format!("{} (use --force to overwrite)", parts.join("; "))
}
