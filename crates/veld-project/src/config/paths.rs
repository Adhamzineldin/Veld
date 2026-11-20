//! Lexical path helpers and config discovery.

use std::path::{Component, Path, PathBuf};

use super::error::ConfigError;

pub const CONFIG_FILE: &str = "veld.toml";

/// Collapses `.` and `..` lexically (no filesystem access, no symlink resolution).
pub(crate) fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                let can_pop = matches!(out.components().next_back(), Some(Component::Normal(_)));
                if can_pop {
                    out.pop();
                } else if !matches!(
                    out.components().next_back(),
                    Some(Component::RootDir | Component::Prefix(_))
                ) {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Absolute, lexically normalized form of `path`.
pub(crate) fn absolutize(path: &Path) -> Result<PathBuf, ConfigError> {
    std::path::absolute(path)
        .map(|p| normalize(&p))
        .map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })
}

/// Searches `start` (a directory, or a file whose directory is used) and its
/// ancestors for `veld.toml`.
pub fn find_config(start: &Path) -> Result<PathBuf, ConfigError> {
    let abs = absolutize(start)?;
    let dir = if abs.is_file() {
        abs.parent().map(Path::to_path_buf).unwrap_or(abs.clone())
    } else {
        abs.clone()
    };
    dir.ancestors()
        .map(|d| d.join(CONFIG_FILE))
        .find(|candidate| candidate.is_file())
        .ok_or(ConfigError::NotFound { start: abs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn normalizes_dots() {
        let base = std::env::temp_dir();
        assert_eq!(
            normalize(&base.join("a/./b/../c")),
            normalize(&base.join("a/c"))
        );
        assert_eq!(normalize(Path::new("a/../../b")), PathBuf::from("../b"));
        assert_eq!(
            normalize(
                &base
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("x")
            )
            .file_name()
            .unwrap(),
            "x"
        );
    }

    #[test]
    fn finds_config_upward() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("a/b/c");
        fs::create_dir_all(&nested).unwrap();
        fs::write(tmp.path().join("a").join(CONFIG_FILE), "").unwrap();
        let found = find_config(&nested).unwrap();
        assert_eq!(
            found,
            absolutize(&tmp.path().join("a").join(CONFIG_FILE)).unwrap()
        );
        let from_file = find_config(&found).unwrap();
        assert_eq!(from_file, found);
    }
}
