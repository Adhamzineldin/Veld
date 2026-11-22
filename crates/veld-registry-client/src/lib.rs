//! The Veld package registry client.
//!
//! Credentials, deterministic pack and hardened unpack, the semver resolver, the lock and the
//! project-local cache. The registry server itself is a separate Go service (`registry/`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod credentials;
pub mod depsedit;
pub mod error;
pub mod http;
pub mod install;
pub mod manifest;
pub mod publish;
pub mod resolver;
pub mod tarball;

pub use credentials::{Credentials, Token};
pub use error::{RegistryError, Result};
pub use http::{Client, Source, VersionInfo};

use credentials::normalize_url;

/// Picks the registry URL: the flag, then `[registry] url` in `veld.toml`, then the
/// `VELD_REGISTRY` environment variable, then the default of the credentials file.
pub fn resolve_registry(
    flag: Option<&str>,
    project: Option<&str>,
    creds: &Credentials,
) -> Result<String> {
    let env = std::env::var("VELD_REGISTRY").ok();
    [flag, project, env.as_deref(), creds.default.as_deref()]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|s| !s.is_empty())
        .map(normalize_url)
        .ok_or_else(|| {
            RegistryError::Resolve(
                "no registry configured: set `[registry] url` in veld.toml, pass --registry, \
                 or run `veld login --registry <url>`"
                    .into(),
            )
        })
}
