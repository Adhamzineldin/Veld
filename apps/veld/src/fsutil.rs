//! Small file helpers shared by commands that write user-owned files.

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

/// Replaces `path` atomically: the bytes go to a temporary file in the same directory, which
/// is then renamed over the target. A crash never leaves a half-written file.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(dir).with_context(|| format!("cannot create `{}`", dir.display()))?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.veld-tmp-{}", std::process::id()));
    let result = write_synced(&tmp, bytes).and_then(|()| fs::rename(&tmp, path));
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("cannot write `{}`", path.display()));
    }
    Ok(())
}

fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut f = fs::File::create(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

/// Creates `path` only if it does not exist yet. Returns `false` when it already exists.
pub fn create_new(path: &Path, bytes: &[u8]) -> Result<bool> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("cannot create `{}`", dir.display()))?;
    }
    match fs::OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut f) => {
            f.write_all(bytes).with_context(|| format!("cannot write `{}`", path.display()))?;
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(e).with_context(|| format!("cannot create `{}`", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_and_leaves_no_temp_file() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("a.txt");
        atomic_write(&p, b"one").unwrap();
        atomic_write(&p, b"two").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"two");
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 1);
    }

    #[test]
    fn create_new_never_overwrites() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("sub/b.txt");
        assert!(create_new(&p, b"x").unwrap());
        assert!(!create_new(&p, b"y").unwrap());
        assert_eq!(fs::read(&p).unwrap(), b"x");
    }
}
