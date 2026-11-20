//! `clean`: remove only what Veld wrote and the user has not modified.

use std::fs;
use std::io;
use std::path::Path;

use super::error::{OutputError, io_at};
use super::manifest::{Manifest, sha256_hex};
use super::safety::{absolute, ensure_path_clear, prune_empty_parents, target_path};
use crate::fileset::MANIFEST_NAME;

/// What a clean removed or left behind. With `dry_run`, `removed` is what would go.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CleanReport {
    pub removed: Vec<String>,
    /// Listed in the manifest but changed by the user (or no longer a file): kept.
    pub modified: Vec<String>,
    /// Listed in the manifest but already gone.
    pub missing: Vec<String>,
}

pub fn clean(out: &Path, dry_run: bool) -> Result<CleanReport, OutputError> {
    let out = absolute(out)?;
    let mut report = CleanReport::default();
    let Some(manifest) = Manifest::load(&out)? else {
        return Ok(report);
    };
    for (rel, hash) in &manifest.files {
        ensure_path_clear(&out, rel)?;
        let target = target_path(&out, rel)?;
        match fs::symlink_metadata(&target) {
            Ok(meta) if meta.is_file() && sha256_hex(&fs::read(&target).map_err(io_at(&target))?) == *hash => {
                if !dry_run {
                    fs::remove_file(&target).map_err(io_at(&target))?;
                }
                report.removed.push(rel.clone());
            }
            Ok(_) => report.modified.push(rel.clone()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => report.missing.push(rel.clone()),
            Err(e) => return Err(io_at(&target)(e)),
        }
    }
    if !dry_run {
        let manifest_path = out.join(MANIFEST_NAME);
        fs::remove_file(&manifest_path).map_err(io_at(&manifest_path))?;
        report.removed.iter().chain(&report.missing).for_each(|rel| prune_empty_parents(&out, rel));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fileset::FileSet;
    use crate::output::{WriteOptions, write};

    #[test]
    fn no_manifest_is_a_no_op() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("user.txt"), "u").unwrap();
        assert_eq!(clean(tmp.path(), false).unwrap(), CleanReport::default());
        assert!(tmp.path().join("user.txt").exists());
        assert_eq!(clean(&tmp.path().join("absent"), false).unwrap(), CleanReport::default());
    }

    #[test]
    fn removes_unmodified_keeps_modified() {
        let tmp = tempfile::tempdir().unwrap();
        let mut files = FileSet::new();
        files.add("a/x.txt", "x").unwrap();
        files.add("b.txt", "b").unwrap();
        write(tmp.path(), &files, &WriteOptions::default()).unwrap();
        fs::write(tmp.path().join("b.txt"), "edited").unwrap();
        let dry = clean(tmp.path(), true).unwrap();
        assert_eq!(dry.removed, ["a/x.txt"]);
        assert!(tmp.path().join("a/x.txt").exists());
        let report = clean(tmp.path(), false).unwrap();
        assert_eq!(report.removed, ["a/x.txt"]);
        assert_eq!(report.modified, ["b.txt"]);
        assert!(!tmp.path().join("a").exists());
        assert!(!tmp.path().join(MANIFEST_NAME).exists());
        assert_eq!(fs::read_to_string(tmp.path().join("b.txt")).unwrap(), "edited");
    }
}
