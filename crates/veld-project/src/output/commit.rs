//! The staged, journaled swap. Every mutation of `out` is a rename, and every
//! displaced file is parked in the staging dir, so a failure can be rolled back.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::error::{OutputError, io_at};
use super::plan::{Plan, PlanAction};
use super::safety::{prune_empty_parents, target_path};
use crate::fileset::{FileSet, MANIFEST_NAME, STAGING_PREFIX};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A path that was changed. `backup` holds the displaced original, if any.
struct Moved {
    target: PathBuf,
    backup: Option<PathBuf>,
}

struct Commit<'a> {
    out: &'a Path,
    files: &'a FileSet,
    staging: PathBuf,
    journal: Vec<Moved>,
    created_dirs: Vec<PathBuf>,
}

/// Applies `plan`, then (last) the manifest. On failure the previous output is restored.
pub(crate) fn apply(out: &Path, files: &FileSet, plan: &Plan, manifest: Option<&[u8]>) -> Result<(), OutputError> {
    let staging = create_staging(out)?;
    let mut commit = Commit { out, files, staging, journal: Vec::new(), created_dirs: Vec::new() };
    let result = commit.run(plan, manifest);
    match result {
        Ok(()) => {
            commit.prune(plan);
            let _ = fs::remove_dir_all(&commit.staging);
            Ok(())
        }
        Err(cause) => {
            if commit.rollback() {
                let _ = fs::remove_dir_all(&commit.staging);
                Err(cause)
            } else {
                Err(OutputError::RollbackIncomplete { cause: cause.to_string(), staging: commit.staging })
            }
        }
    }
}

fn create_staging(out: &Path) -> Result<PathBuf, OutputError> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    for _ in 0..32 {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = out.join(format!("{STAGING_PREFIX}{:x}-{nanos:x}-{n}", std::process::id()));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(io_at(&path)(e)),
        }
    }
    Err(OutputError::Internal("could not create a unique staging directory".into()))
}

fn writes(plan: &Plan) -> impl Iterator<Item = &str> {
    plan.entries
        .iter()
        .filter(|e| matches!(e.action, PlanAction::Create | PlanAction::Update))
        .map(|e| e.path.as_str())
}

impl Commit<'_> {
    fn run(&mut self, plan: &Plan, manifest: Option<&[u8]>) -> Result<(), OutputError> {
        self.stage_files(plan)?;
        self.place_files(plan)?;
        self.remove_stale(plan)?;
        if let Some(bytes) = manifest {
            self.write_manifest(bytes)?;
        }
        Ok(())
    }

    fn staged_path(&self, rel: &str) -> PathBuf {
        rel.split('/').fold(self.staging.join("files"), |p, c| p.join(c))
    }

    fn backup_path(&self, rel: &str) -> PathBuf {
        rel.split('/').fold(self.staging.join("backup"), |p, c| p.join(c))
    }

    fn stage_files(&self, plan: &Plan) -> Result<(), OutputError> {
        for rel in writes(plan) {
            let bytes = self.files.get(rel).ok_or_else(|| OutputError::MissingContent { path: rel.to_string() })?;
            let dest = self.staged_path(rel);
            make_parent(&dest)?;
            fs::write(&dest, bytes).map_err(io_at(&dest))?;
        }
        Ok(())
    }

    fn place_files(&mut self, plan: &Plan) -> Result<(), OutputError> {
        for rel in writes(plan) {
            let target = target_path(self.out, rel)?;
            if let Some(parent) = target.parent() {
                self.ensure_dirs(parent)?;
            }
            self.displace(&target, rel)?;
            let staged = self.staged_path(rel);
            fs::rename(&staged, &target).map_err(io_at(&target))?;
        }
        Ok(())
    }

    fn remove_stale(&mut self, plan: &Plan) -> Result<(), OutputError> {
        for entry in plan.entries.iter().filter(|e| e.action == PlanAction::Delete) {
            let target = target_path(self.out, &entry.path)?;
            self.displace(&target, &entry.path)?;
        }
        Ok(())
    }

    fn write_manifest(&mut self, bytes: &[u8]) -> Result<(), OutputError> {
        let target = self.out.join(MANIFEST_NAME);
        let staged = self.staging.join("manifest.new");
        fs::write(&staged, bytes).map_err(io_at(&staged))?;
        self.displace(&target, MANIFEST_NAME)?;
        fs::rename(&staged, &target).map_err(io_at(&target))
    }

    /// Moves an existing `target` into the backup area and journals it. If
    /// nothing exists there, journals the (about to be created) path.
    fn displace(&mut self, target: &Path, rel: &str) -> Result<(), OutputError> {
        if fs::symlink_metadata(target).is_err() {
            self.journal.push(Moved { target: target.to_path_buf(), backup: None });
            return Ok(());
        }
        let backup = self.backup_path(rel);
        make_parent(&backup)?;
        fs::rename(target, &backup).map_err(io_at(target))?;
        self.journal.push(Moved { target: target.to_path_buf(), backup: Some(backup) });
        Ok(())
    }

    /// Creates missing directories down to `dir`, remembering them for rollback.
    fn ensure_dirs(&mut self, dir: &Path) -> Result<(), OutputError> {
        let mut missing = Vec::new();
        let mut cur = dir;
        while cur != self.out && fs::symlink_metadata(cur).is_err() {
            missing.push(cur.to_path_buf());
            match cur.parent() {
                Some(p) => cur = p,
                None => break,
            }
        }
        for d in missing.into_iter().rev() {
            fs::create_dir(&d).map_err(io_at(&d))?;
            self.created_dirs.push(d);
        }
        Ok(())
    }

    /// Restores every journaled path. Returns false if something could not be restored.
    fn rollback(&mut self) -> bool {
        let mut ok = true;
        for moved in self.journal.drain(..).rev() {
            if fs::symlink_metadata(&moved.target).is_ok() && fs::remove_file(&moved.target).is_err() {
                ok = false;
                continue;
            }
            if let Some(backup) = moved.backup {
                ok &= fs::rename(&backup, &moved.target).is_ok();
            }
        }
        for dir in self.created_dirs.drain(..).rev() {
            let _ = fs::remove_dir(&dir);
        }
        ok
    }

    fn prune(&self, plan: &Plan) {
        for entry in plan.entries.iter().filter(|e| e.action == PlanAction::Delete) {
            prune_empty_parents(self.out, &entry.path);
        }
    }
}

fn make_parent(path: &Path) -> Result<(), OutputError> {
    match path.parent() {
        Some(parent) => fs::create_dir_all(parent).map_err(io_at(parent)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::plan::PlanEntry;

    fn entry(path: &str, action: PlanAction) -> PlanEntry {
        PlanEntry { path: path.into(), action }
    }

    #[test]
    fn failure_after_partial_swap_rolls_back() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path();
        fs::write(out.join("a"), "old-a").unwrap();
        fs::write(out.join("blocker"), "i am a file").unwrap();
        let mut files = FileSet::new();
        files.add("a", "new-a").unwrap();
        files.add("blocker/child", "x").unwrap();
        // A hand-built plan that skips the plan-time checks: "a" swaps fine, then
        // creating "blocker/child" fails because "blocker" is a file.
        let plan = Plan { entries: vec![entry("a", PlanAction::Update), entry("blocker/child", PlanAction::Create)] };
        assert!(apply(out, &files, &plan, Some(b"{}")).is_err());
        assert_eq!(fs::read_to_string(out.join("a")).unwrap(), "old-a");
        assert_eq!(fs::read_to_string(out.join("blocker")).unwrap(), "i am a file");
        assert!(!out.join(MANIFEST_NAME).exists());
        let leftovers: Vec<_> = fs::read_dir(out).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(leftovers.len(), 2, "staging dir must be removed: {leftovers:?}");
    }

    #[test]
    fn missing_staged_content_changes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let files = FileSet::new();
        let plan = Plan { entries: vec![entry("nope", PlanAction::Create)] };
        let err = apply(tmp.path(), &files, &plan, None).unwrap_err();
        assert!(matches!(err, OutputError::MissingContent { .. }));
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 0);
    }
}
