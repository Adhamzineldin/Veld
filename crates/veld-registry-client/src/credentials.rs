//! `~/.veld/credentials.toml`: API tokens keyed by registry URL.
//!
//! Tokens are never printed or logged: [`Token`] has a redacting `Debug` and no `Display`.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{RegistryError, Result, io};

/// An API token. Deliberately opaque.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Token(String);

impl Token {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The secret, for the `Authorization` header only.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(<redacted>)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub token: Token,
}

/// The credentials file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Credentials {
    /// The registry used when nothing else names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(default)]
    pub registries: BTreeMap<String, Entry>,
}

/// Registry URLs are compared without a trailing slash.
pub fn normalize_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

/// The directory holding `credentials.toml`: `$VELD_HOME`, else `~/.veld`.
pub fn home_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("VELD_HOME").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE").filter(|v| !v.is_empty()))
        .ok_or_else(|| RegistryError::Credentials("cannot find the home directory".into()))?;
    Ok(PathBuf::from(home).join(".veld"))
}

/// Path of the credentials file.
pub fn credentials_path() -> Result<PathBuf> {
    Ok(home_dir()?.join("credentials.toml"))
}

impl Credentials {
    /// Loads the file; a missing file is an empty set of credentials.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let mut c: Credentials = toml::from_str(&text).map_err(|e| {
                    RegistryError::Credentials(format!("{}: {}", path.display(), e.message()))
                })?;
                c.registries = c
                    .registries
                    .into_iter()
                    .map(|(k, v)| (normalize_url(&k), v))
                    .collect();
                c.default = c.default.map(|d| normalize_url(&d));
                Ok(c)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(io(path)(e)),
        }
    }

    /// Writes the file, restricted to the current user where the OS supports it.
    pub fn save(&self, path: &Path) -> Result<()> {
        let text =
            toml::to_string_pretty(self).map_err(|e| RegistryError::Credentials(e.to_string()))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(io(dir))?;
        }
        write_private(path, text.as_bytes())
    }

    pub fn token_for(&self, registry: &str) -> Option<&Token> {
        self.registries
            .get(&normalize_url(registry))
            .map(|e| &e.token)
    }

    /// Stores a token and makes `registry` the default.
    pub fn set(&mut self, registry: &str, token: Token) {
        let url = normalize_url(registry);
        self.registries.insert(url.clone(), Entry { token });
        self.default = Some(url);
    }

    /// Removes a token. Returns whether there was one.
    pub fn remove(&mut self, registry: &str) -> bool {
        let url = normalize_url(registry);
        let had = self.registries.remove(&url).is_some();
        if self.default.as_deref() == Some(url.as_str()) {
            self.default = self.registries.keys().next().cloned();
        }
        had
    }
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let tmp = path.with_extension("toml.tmp");
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)
        .map_err(io(&tmp))?;
    f.write_all(bytes).map_err(io(&tmp))?;
    drop(f);
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600)).map_err(io(&tmp))?;
    std::fs::rename(&tmp, path).map_err(io(path))
}

/// Windows: files under the user profile are already private to the user by default ACLs.
#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, bytes).map_err(io(&tmp))?;
    std::fs::rename(&tmp, path).map_err(io(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_normalisation() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sub").join("credentials.toml");
        assert_eq!(Credentials::load(&path).unwrap(), Credentials::default());
        let mut c = Credentials::default();
        c.set("https://reg.example/", Token::new("vtk_secret"));
        c.save(&path).unwrap();
        let back = Credentials::load(&path).unwrap();
        assert_eq!(back, c);
        assert_eq!(back.default.as_deref(), Some("https://reg.example"));
        assert_eq!(
            back.token_for("https://reg.example///").unwrap().expose(),
            "vtk_secret"
        );
        assert!(back.token_for("https://other").is_none());
    }

    #[test]
    fn debug_never_shows_the_token() {
        let mut c = Credentials::default();
        c.set("https://r", Token::new("vtk_topsecret"));
        let shown = format!("{c:?}");
        assert!(!shown.contains("topsecret"), "{shown}");
    }

    #[test]
    fn remove_updates_the_default() {
        let mut c = Credentials::default();
        c.set("https://a", Token::new("1"));
        c.set("https://b", Token::new("2"));
        assert!(c.remove("https://b"));
        assert_eq!(c.default.as_deref(), Some("https://a"));
        assert!(!c.remove("https://b"));
        assert!(c.remove("https://a"));
        assert_eq!(c.default, None);
    }

    #[test]
    fn rejects_unknown_keys_and_garbage() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("c.toml");
        std::fs::write(&path, "bogus = 1\n").unwrap();
        assert!(Credentials::load(&path).is_err());
        std::fs::write(&path, "= = =").unwrap();
        assert!(Credentials::load(&path).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("credentials.toml");
        Credentials::default().save(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
