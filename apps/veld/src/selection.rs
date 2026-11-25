//! Which members a command works on: the current member, `-p`, `--workspace` and
//! `--affected --since <ref>` (`spec/workspace.md`, sections 9 and 10).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use veld_compiler::FileId;
use veld_project::config::{
    self, ConfigError, Project, SelectBy, Selection, affected, collect_sources, selection_of,
};

use crate::cli::SelectArgs;
use crate::ui::Ui;

/// Resolves the selection of a command. `None` means `--affected` found nothing to do (the
/// message is already printed and the run ends with exit 0).
pub fn resolve(ui: &Ui, project: &Project, args: &SelectArgs) -> Result<Option<Selection>> {
    if args.affected {
        let since = args.since.as_deref().unwrap_or_default();
        return affected_selection(ui, project, since);
    }
    let by = if !args.package.is_empty() {
        SelectBy::Packages(&args.package)
    } else if args.workspace {
        SelectBy::Workspace
    } else {
        SelectBy::Default
    };
    Ok(Some(config::select(project, by)?))
}

fn affected_selection(ui: &Ui, project: &Project, since: &str) -> Result<Option<Selection>> {
    let changed = changed_paths(&project.root, since)?;
    let imports = imports_of(project);
    let list = affected(project, &changed, &imports);
    if list.is_empty() {
        ui.info(&format!("nothing is affected since {since}"));
        return Ok(None);
    }
    ui.info(&format!("affected since {since}:"));
    let width = list.iter().map(|a| a.member.len()).max().unwrap_or(0);
    for a in &list {
        ui.info(&format!("  {:<width$}   {}", a.member, a.reason));
    }
    Ok(Some(selection_of(project, &list)))
}

/// For each member with contracts, the other members whose packages its contracts `use`.
pub fn imports_of(project: &Project) -> BTreeMap<String, BTreeSet<String>> {
    let mut out = BTreeMap::new();
    for pkg in &project.packages {
        let mut used = BTreeSet::new();
        for (_, text) in collect_sources(pkg).unwrap_or_default() {
            let (file, _, _) = veld_compiler::parse_file(&text, FileId(0));
            for u in &file.uses {
                let Some(name) = u.package.as_ref().map(|p| p.name.as_str()) else {
                    continue;
                };
                if name != pkg.name && project.member(name).is_some() {
                    used.insert(name.to_string());
                }
            }
        }
        out.insert(pkg.name.clone(), used);
    }
    out
}

fn git(dir: &Path, args: &[&str]) -> Result<Vec<u8>, ConfigError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| ConfigError::affected(format!("cannot run `git`: {e}")))?;
    if out.status.success() {
        return Ok(out.stdout);
    }
    let why = String::from_utf8_lossy(&out.stderr).trim().to_string();
    Err(ConfigError::affected(why))
}

fn text(bytes: Vec<u8>) -> String {
    String::from_utf8_lossy(&bytes).trim().to_string()
}

/// The files that differ between the merge base of `since` and `HEAD`, and the working tree,
/// plus untracked files that are not ignored, as absolute paths (section 10.1).
fn changed_paths(root: &Path, since: &str) -> Result<Vec<PathBuf>, ConfigError> {
    let top = text(git(root, &["rev-parse", "--show-toplevel"]).map_err(|_| {
        ConfigError::affected(format!(
            "`{}` is not inside a git work tree",
            root.display()
        ))
    })?);
    let top = PathBuf::from(top);
    // The work tree's top as the workspace root spells it (a symlinked temp directory has two
    // spellings): go up from the root by the number of components of its prefix in the tree.
    let prefix = text(git(root, &["rev-parse", "--show-prefix"])?);
    let up = prefix.split('/').filter(|c| !c.is_empty()).count();
    let spelled = root.ancestors().nth(up).map_or_else(|| top.clone(), Path::to_path_buf);
    let base = text(git(&top, &["merge-base", since, "HEAD"]).map_err(|e| {
        ConfigError::affected(format!("unknown ref `{since}` ({e})"))
    })?);
    let mut paths = names(git(&top, &["diff", "--name-only", "-z", "--no-renames", &base])?);
    paths.extend(names(git(
        &top,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?));
    paths.sort();
    paths.dedup();
    Ok(paths.into_iter().map(|p| spelled.join(p)).collect())
}

fn names(bytes: Vec<u8>) -> Vec<String> {
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nul_separated_names_are_split() {
        assert_eq!(names(b"a/b\0c\0".to_vec()), ["a/b", "c"]);
        assert!(names(Vec::new()).is_empty());
    }
}
