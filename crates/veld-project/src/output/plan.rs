//! Computes what a write would do, without touching anything.

use std::fs;
use std::io;
use std::path::Path;

use super::error::{OutputError, io_at};
use super::manifest::{Manifest, sha256_hex};
use super::safety::{ensure_path_clear, target_path};
use crate::fileset::FileSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanAction {
    Create,
    Update,
    Delete,
    Unchanged,
    /// An existing file Veld does not own, with different content.
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanEntry {
    pub path: String,
    pub action: PlanAction,
}

/// The per-path outcome of a write, sorted by path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub entries: Vec<PlanEntry>,
}

impl Plan {
    /// Whether any file would be created, updated or deleted.
    pub fn has_changes(&self) -> bool {
        self.entries.iter().any(|e| matches!(e.action, PlanAction::Create | PlanAction::Update | PlanAction::Delete))
    }

    pub fn conflicts(&self) -> Vec<String> {
        self.paths_with(PlanAction::Conflict)
    }

    pub fn paths_with(&self, action: PlanAction) -> Vec<String> {
        self.entries.iter().filter(|e| e.action == action).map(|e| e.path.clone()).collect()
    }

    pub fn action_of(&self, path: &str) -> Option<PlanAction> {
        self.entries.iter().find(|e| e.path == path).map(|e| e.action)
    }
}

pub(crate) fn build(out: &Path, old: &Manifest, files: &FileSet, force: bool) -> Result<Plan, OutputError> {
    let mut entries = Vec::new();
    for (rel, bytes) in files.iter() {
        entries.push(PlanEntry { path: rel.to_string(), action: classify(out, rel, bytes, old, force)? });
    }
    for rel in old.files.keys().filter(|p| !files.contains(p)) {
        if stale_exists(out, rel)? {
            entries.push(PlanEntry { path: rel.clone(), action: PlanAction::Delete });
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Plan { entries })
}

fn classify(out: &Path, rel: &str, bytes: &[u8], old: &Manifest, force: bool) -> Result<PlanAction, OutputError> {
    ensure_path_clear(out, rel)?;
    let target = target_path(out, rel)?;
    let meta = match fs::symlink_metadata(&target) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(PlanAction::Create),
        Err(e) => return Err(io_at(&target)(e)),
    };
    if !meta.is_file() {
        return Err(OutputError::Blocked { path: rel.to_string(), reason: "the path exists and is not a regular file" });
    }
    let existing = fs::read(&target).map_err(io_at(&target))?;
    Ok(if sha256_hex(&existing) == sha256_hex(bytes) {
        PlanAction::Unchanged
    } else if old.files.contains_key(rel) || force {
        PlanAction::Update
    } else {
        PlanAction::Conflict
    })
}

fn stale_exists(out: &Path, rel: &str) -> Result<bool, OutputError> {
    ensure_path_clear(out, rel)?;
    let target = target_path(out, rel)?;
    match fs::symlink_metadata(&target) {
        Ok(m) => Ok(m.is_file()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(io_at(&target)(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[(&str, &str)]) -> FileSet {
        let mut fs = FileSet::new();
        for (p, c) in items {
            fs.add(*p, *c).unwrap();
        }
        fs
    }

    #[test]
    fn classifies_each_case() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path();
        fs::write(out.join("same"), "1").unwrap();
        fs::write(out.join("managed"), "old").unwrap();
        fs::write(out.join("foreign"), "user").unwrap();
        fs::write(out.join("stale"), "s").unwrap();
        let old = Manifest::from_files(&set(&[("managed", "old"), ("stale", "s"), ("gone", "g")]));
        let new = set(&[("same", "1"), ("managed", "new"), ("foreign", "gen"), ("fresh", "f")]);
        let plan = build(out, &old, &new, false).unwrap();
        let get = |p: &str| plan.action_of(p).unwrap();
        assert_eq!(get("same"), PlanAction::Unchanged);
        assert_eq!(get("managed"), PlanAction::Update);
        assert_eq!(get("foreign"), PlanAction::Conflict);
        assert_eq!(get("fresh"), PlanAction::Create);
        assert_eq!(get("stale"), PlanAction::Delete);
        assert_eq!(plan.action_of("gone"), None);
        assert_eq!(build(out, &old, &new, true).unwrap().action_of("foreign"), Some(PlanAction::Update));
    }

    #[test]
    fn directory_in_the_way_is_blocked() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("d")).unwrap();
        let err = build(tmp.path(), &Manifest::default(), &set(&[("d", "x")]), true).unwrap_err();
        assert!(matches!(err, OutputError::Blocked { .. }));
    }
}
