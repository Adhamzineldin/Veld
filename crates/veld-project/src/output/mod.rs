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

/// The plan for writing `files` into `out`, touching nothing: the stale-output check behind
/// `generate --check` (`plan.is_stale()`). Same as a dry-run [`write`].
pub fn check(out: &Path, files: &FileSet) -> Result<Plan, OutputError> {
    write(
        out,
        files,
        &WriteOptions {
            dry_run: true,
            force: false,
        },
    )
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

/// Writes files the user will own and edit (`veld export ci|env|docker`): staged like [`write`], but
/// with no manifest, so the files are never overwritten or deleted by a later `generate` or
/// `clean`. An existing file with different content is refused (`OutputError::Conflicts`) unless
/// `force`; identical files are left untouched. Nothing is ever deleted.
pub fn write_unmanaged(out: &Path, files: &FileSet, force: bool) -> Result<Plan, OutputError> {
    let out = safety::absolute(out)?;
    check_out_dir(&out)?;
    let plan = plan::build(&out, &Manifest::default(), files, force)?;
    let foreign = plan.paths_with(PlanAction::Conflict);
    if !foreign.is_empty() {
        return Err(OutputError::Conflicts {
            foreign,
            edited: Vec::new(),
        });
    }
    if plan.has_changes() {
        fs::create_dir_all(&out).map_err(io_at(&out))?;
        commit::remove_stale_staging(&out)?;
        commit::apply(&out, files, &plan, None)?;
    }
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
    fn check_reports_staleness_without_writing() {
        let tmp = tempfile::tempdir().unwrap();
        let mut files = FileSet::new();
        files.add("a.txt", "a").unwrap();
        assert!(check(tmp.path(), &files).unwrap().is_stale());
        assert!(!tmp.path().join("a.txt").exists());
        write(tmp.path(), &files, &WriteOptions::default()).unwrap();
        assert!(!check(tmp.path(), &files).unwrap().is_stale());
        files.add("b.txt", "b").unwrap();
        let plan = check(tmp.path(), &files).unwrap();
        assert_eq!(plan.stale_entries().len(), 1);
        let mut shrunk = FileSet::new();
        shrunk.add("a.txt", "changed").unwrap();
        std::fs::write(tmp.path().join("a.txt"), "edited").unwrap();
        assert!(check(tmp.path(), &shrunk).unwrap().is_stale());
    }

    #[test]
    fn unmanaged_write_creates_keeps_and_refuses() {
        let tmp = tempfile::tempdir().unwrap();
        let mut files = FileSet::new();
        files.add("ci/a.yml", "a").unwrap();
        files.add("b.txt", "b").unwrap();
        let plan = write_unmanaged(tmp.path(), &files, false).unwrap();
        assert_eq!(plan.paths_with(PlanAction::Create).len(), 2);
        assert!(
            !tmp.path().join(".veld-manifest.json").exists(),
            "no manifest"
        );
        let again = write_unmanaged(tmp.path(), &files, false).unwrap();
        assert!(!again.has_changes());
        std::fs::write(tmp.path().join("b.txt"), "mine").unwrap();
        let err = write_unmanaged(tmp.path(), &files, false).unwrap_err();
        assert!(matches!(err, OutputError::Conflicts { .. }));
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("b.txt")).unwrap(),
            "mine"
        );
        write_unmanaged(tmp.path(), &files, true).unwrap();
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("b.txt")).unwrap(),
            "b"
        );
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
