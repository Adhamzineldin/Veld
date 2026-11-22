//! Small file helpers shared by commands that write user-owned files.

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

/// Replaces `path` atomically: the bytes go to a temporary file in the same directory, which
/// is then renamed over the target. A crash never leaves a half-written file.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(dir).with_context(|| format!("cannot create `{}`", dir.display()))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
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
}
