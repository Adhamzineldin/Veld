use std::path::PathBuf;

use thiserror::Error;

/// Everything that can go wrong talking to a registry or handling a package.
#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("cannot access `{}`: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("registry URL `{0}` is not valid")]
    BadUrl(String),
    #[error(
        "registry URL `{0}` is not https; use https, or pass --allow-insecure (localhost is always allowed)"
    )]
    Insecure(String),
    #[error("cannot reach the registry: {0}")]
    Network(String),
    #[error("registry returned {status}: {message}")]
    Http { status: u16, message: String },
    #[error("not logged in to {0}; run `veld login --registry {0}`")]
    NotLoggedIn(String),
    #[error("credentials file: {0}")]
    Credentials(String),
    #[error("unsafe or invalid package: {0}")]
    Tarball(String),
    #[error("invalid package manifest: {0}")]
    Manifest(String),
    #[error(
        "checksum mismatch for {what}: expected {expected}, found {actual}\nthe package may have been tampered with; delete .veld/packages and run `veld install`, or re-resolve with `veld add`"
    )]
    Checksum {
        what: String,
        expected: String,
        actual: String,
    },
    #[error("{0}")]
    Resolve(String),
    #[error("{0}")]
    Lock(#[from] veld_project::packages::PackagesError),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, RegistryError>;

pub(crate) fn io(path: &std::path::Path) -> impl FnOnce(std::io::Error) -> RegistryError {
    let path = path.to_path_buf();
    move |source| RegistryError::Io { path, source }
}
