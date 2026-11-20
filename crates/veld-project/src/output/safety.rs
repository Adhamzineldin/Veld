//! Path containment and symlink checks.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::error::{OutputError, io_at};
use crate::fileset::validate_rel_path;

pub(crate) fn absolute(out: &Path) -> Result<PathBuf, OutputError> {
    std::path::absolute(out).map_err(io_at(out))
}

/// Joins a validated relative path onto `out`, checking containment.
pub(crate) fn target_path(out: &Path, rel: &str) -> Result<PathBuf, OutputError> {
    let escapes = || OutputError::PathEscapes {
        path: rel.to_string(),
    };
    validate_rel_path(rel).map_err(|_| escapes())?;
    let mut path = out.to_path_buf();
    rel.split('/').for_each(|c| path.push(c));
    // `Path::starts_with` compares whole components, i.e. a trailing-separator prefix test.
    if path.starts_with(out) && path != out {
        Ok(path)
    } else {
        Err(escapes())
    }
}

pub(crate) fn is_link(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const REPARSE_POINT: u32 = 0x400;
        meta.file_attributes() & REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    false
}

/// Walks every existing component of `rel` under `out`. Errors if one is a
/// symlink, or if a non-final component exists but is not a directory.
pub(crate) fn ensure_path_clear(out: &Path, rel: &str) -> Result<(), OutputError> {
    let comps: Vec<&str> = rel.split('/').collect();
    let mut cur = out.to_path_buf();
    for (i, comp) in comps.iter().enumerate() {
        cur.push(comp);
        match fs::symlink_metadata(&cur) {
            Ok(meta) if is_link(&meta) => return Err(OutputError::Symlink { path: cur }),
            Ok(meta) if i + 1 < comps.len() && !meta.is_dir() => {
                return Err(OutputError::Blocked {
                    path: rel.to_string(),
                    reason: "a parent path exists and is not a directory",
                });
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                return Err(OutputError::Io {
                    path: cur,
                    source: e,
                });
            }
        }
    }
    Ok(())
}

/// Removes now-empty parent directories of `rel`, stopping at `out` or the
/// first directory that still has content. Never removes `out` itself.
pub(crate) fn prune_empty_parents(out: &Path, rel: &str) {
    let Some(mut dir) = target_path(out, rel)
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    else {
        return;
    };
    while dir != out && dir.starts_with(out) {
        if fs::remove_dir(&dir).is_err() {
            return;
        }
        match dir.parent() {
            Some(p) => dir = p.to_path_buf(),
            None => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_path_stays_inside() {
        let out = std::env::temp_dir().join("veld-safety");
        let p = target_path(&out, "a/b.txt").unwrap();
        assert!(p.starts_with(&out));
        assert!(target_path(&out, "../b").is_err());
        assert!(target_path(&out, "/abs").is_err());
    }

    #[test]
    fn clear_path_detects_file_parent() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("f"), "x").unwrap();
        assert!(matches!(
            ensure_path_clear(tmp.path(), "f/g"),
            Err(OutputError::Blocked { .. })
        ));
        assert!(ensure_path_clear(tmp.path(), "f").is_ok());
        assert!(ensure_path_clear(tmp.path(), "missing/g").is_ok());
    }

    #[test]
    fn prune_stops_at_non_empty_and_out() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("a/b")).unwrap();
        fs::write(tmp.path().join("a/keep"), "x").unwrap();
        prune_empty_parents(tmp.path(), "a/b/c.txt");
        assert!(tmp.path().join("a").exists());
        assert!(!tmp.path().join("a/b").exists());
        fs::remove_file(tmp.path().join("a/keep")).unwrap();
        prune_empty_parents(tmp.path(), "a/x");
        assert!(!tmp.path().join("a").exists());
        assert!(tmp.path().exists());
    }
}
