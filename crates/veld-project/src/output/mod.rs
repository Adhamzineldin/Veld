//! The safe, staged output writer.
//!
//! Guarantees: generation never deletes anything Veld did not write (tracked by
//! `.veld-manifest.json`), refuses to clobber unmanaged files and files the user has edited
//! since Veld wrote them (their hash differs from the manifest; `--force` overrides), never follows
//! symlinks, swaps files in through a staging directory with rollback on
//! failure, and writes the manifest last.

mod clean;
mod commit;
mod error;
mod manifest;
mod plan;
mod safety;

use std::fs;
use std::io;
use std::path::Path;

pub use clean::{CleanReport, clean};
pub use error::OutputError;
pub use manifest::{Manifest, sha256_hex};
pub use plan::{Plan, PlanAction, PlanEntry};

use crate::fileset::FileSet;
use error::io_at;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WriteOptions {
    /// Compute and return the plan; write nothing at all.
    pub dry_run: bool,
    /// Overwrite unmanaged files and user-edited generated files whose content differs.
    pub force: bool,
}

/// Writes `files` into `out`, returning what was (or, for a dry run, would be) done.
pub fn write(out: &Path, files: &FileSet, opts: &WriteOptions) -> Result<Plan, OutputError> {
    let out = safety::absolute(out)?;
    check_out_dir(&out)?;
    let old = Manifest::load(&out)?;
    if !opts.dry_run {
        commit::remove_stale_staging(&out)?;
    }
    let plan = plan::build(&out, &old.clone().unwrap_or_default(), files, opts.force)?;
    if opts.dry_run {
        return Ok(plan);
    }
    let foreign = plan.paths_with(PlanAction::Conflict);
    let edited = plan.paths_with(PlanAction::Modified);
    if !foreign.is_empty() || !edited.is_empty() {
        return Err(OutputError::Conflicts { foreign, edited });
    }
    let new = Manifest::from_files(files);
    let manifest_current = old.as_ref() == Some(&new);
    if !plan.has_changes() && manifest_current {
        return Ok(plan);
    }
    fs::create_dir_all(&out).map_err(io_at(&out))?;
    let bytes = if manifest_current {
        None
    } else {
        Some(new.to_bytes()?)
    };
    commit::apply(&out, files, &plan, bytes.as_deref())?;
    Ok(plan)
}

fn check_out_dir(out: &Path) -> Result<(), OutputError> {
    match fs::metadata(out) {
        Ok(m) if !m.is_dir() => Err(OutputError::OutNotDirectory {
            path: out.to_path_buf(),
        }),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_at(out)(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_that_is_a_file_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let f = tmp.path().join("f");
        fs::write(&f, "x").unwrap();
        let err = write(&f, &FileSet::new(), &WriteOptions::default()).unwrap_err();
        assert!(matches!(err, OutputError::OutNotDirectory { .. }));
    }

    #[test]
    fn unchanged_rewrite_touches_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let mut files = FileSet::new();
        files.add("a.txt", "a").unwrap();
        write(tmp.path(), &files, &WriteOptions::default()).unwrap();
        let plan = write(tmp.path(), &files, &WriteOptions::default()).unwrap();
        assert!(!plan.has_changes());
    }
}
