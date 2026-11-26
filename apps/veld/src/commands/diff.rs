//! `veld diff`: breaking-change detection against a git ref or the IR lock file.
//!
//! `--against` reads the old contracts with `git ls-tree` and `git show`; it never checks
//! anything out and never touches the working tree.

use std::path::Path;
use std::process::Command as Process;

use anyhow::{Context, Result};
use veld_ir::Ir;
use veld_project::config::{PackageSpec, Project};
use veld_tools::{Change, Severity};

use crate::cli::DiffArgs;
use crate::error::{Exit, user};
use crate::pipeline;
use crate::ui::{Style, Ui};

pub fn run(ui: &Ui, config: Option<&Path>, args: &DiffArgs) -> Result<Exit> {
    ui.header("diff", "");
    let project = pipeline::load_project(config)?;
    let Some(done) = pipeline::compile_or_report(ui, project)? else {
        return Ok(Exit::User);
    };
    let (old, label) = match &args.against {
        Some(git_ref) => (
            ir_at_ref(ui, &done.project, git_ref)?,
            format!("git ref `{git_ref}`"),
        ),
        None => (
            read_lock(&done.project)?,
            format!("`{}`", pipeline::LOCK_FILE),
        ),
    };
    let mut changes = veld_tools::diff(&old, &done.ir);
    let report = match args.prove.prove {
        true => super::prove::diff(ui, &old, &done.ir, &mut changes, &args.prove),
        false => None,
    };
    print_changes(ui, &changes, &label);
    print_owners(ui, &done.project, &changes);
    if let Some(report) = &report {
        super::prove::print_report(ui, report);
    }
    let breaking = veld_tools::has_breaking(&changes);
    Ok(if breaking && args.strict {
        Exit::User
    } else {
        Exit::Ok
    })
}

fn read_lock(project: &Project) -> Result<Ir> {
    let path = pipeline::lock_path(project);
    let text = std::fs::read_to_string(&path).map_err(|_| {
        user(format!(
            "`{}` does not exist; run `veld generate` first, or compare with `--against <git-ref>`",
            pipeline::display_path(&path)
        ))
    })?;
    serde_json::from_str(&text).map_err(|e| {
        user(format!(
            "`{}` is not a valid IR snapshot: {e}",
            pipeline::display_path(&path)
        ))
    })
}

fn ir_at_ref(ui: &Ui, project: &Project, git_ref: &str) -> Result<Ir> {
    git(
        &project.root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{git_ref}^{{commit}}"),
        ],
    )
    .map_err(|_| {
        user(format!(
            "`{git_ref}` is not a git commit, branch or tag in this repository"
        ))
    })?;
    let input = pipeline::compile_input_from(project, |pkg| sources_at_ref(pkg, git_ref))?;
    let out = veld_compiler::compile(&input);
    let errors = out.diagnostics.iter().filter(|d| d.is_error()).count();
    match out.ir {
        Some(ir) if errors == 0 => Ok(ir),
        _ => {
            for d in out.diagnostics.iter().filter(|d| d.is_error()) {
                ui.diagnostic(&out.sources.render(d));
            }
            Err(user(format!("the contracts at `{git_ref}` do not compile")))
        }
    }
}

/// The `.veld` files of `pkg` as they were at `git_ref`, relative to the package root.
/// A package that did not exist yet has no files.
fn sources_at_ref(pkg: &PackageSpec, git_ref: &str) -> Result<Vec<(String, String)>> {
    if !pkg.root.is_dir() {
        return Ok(Vec::new());
    }
    let listing = git(
        &pkg.root,
        &["ls-tree", "-r", "-z", "--name-only", git_ref, "--", "."],
    )?;
    let mut names: Vec<&str> = listing
        .split('\0')
        .filter(|n| n.ends_with(".veld"))
        .collect();
    names.sort_unstable();
    names
        .into_iter()
        .map(|name| {
            Ok((
                name.to_string(),
                git(&pkg.root, &["show", &format!("{git_ref}:./{name}")])?,
            ))
        })
        .collect()
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Process::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .context("cannot run `git`; is it installed and on PATH?")?;
    if !out.status.success() {
        anyhow::bail!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The package a change subject belongs to: a declaration is `<package>:<module>.<Name>` and an
/// operation is `veld:<package>/<Service>.<operation>`.
fn package_of(subject: &str) -> Option<&str> {
    let package = match subject.strip_prefix("veld:") {
        Some(rest) => rest.split('/').next()?,
        None => subject.split_once(':')?.0,
    };
    Some(package).filter(|p| !p.is_empty() && !p.contains(['.', '/']))
}

/// `(a, b)` of owners, or nothing when there are none.
fn owners_text(owners: &[String]) -> String {
    if owners.is_empty() {
        String::new()
    } else {
        format!(" ({})", owners.join(", "))
    }
}

/// Under each package with a breaking or possibly breaking change: its owners, and the members
/// that depend on it (`spec/workspace.md`, section 11). Nothing when no member has owners.
fn print_owners(ui: &Ui, project: &Project, changes: &[Change]) {
    if project.members.iter().all(|m| m.owners.is_empty()) {
        return;
    }
    let packages: std::collections::BTreeSet<&str> = changes
        .iter()
        .filter(|c| c.severity != Severity::NonBreaking)
        .filter_map(|c| package_of(&c.subject))
        .collect();
    let imports = crate::selection::imports_of(project);
    let mut lines = Vec::new();
    for package in packages {
        let Some(member) = project.member(package) else {
            continue;
        };
        lines.push(format!("  {package}"));
        if !member.owners.is_empty() {
            lines.push(format!("    owners: {}", member.owners.join(", ")));
        }
        for d in veld_project::config::dependents(project, package, &imports) {
            lines.push(format!("    affects {}{}", d.name, owners_text(&d.owners)));
        }
    }
    if !lines.is_empty() {
        ui.data("Who is affected:");
        lines.iter().for_each(|l| ui.data(l));
    }
}

fn print_changes(ui: &Ui, changes: &[Change], label: &str) {
    if changes.is_empty() {
        ui.ok(&format!("no changes against {label}"));
        return;
    }
    for (severity, title, mark) in [
        (
            Severity::Breaking,
            "Breaking changes",
            ui.out_styled(Style::Red, ui.theme_out().sym().fail),
        ),
        (
            Severity::PossiblyBreaking,
            "Possibly breaking changes (whether they break a caller or a consumer is not known)",
            ui.out_styled(Style::Yellow, "?"),
        ),
        (
            Severity::NonBreaking,
            "Non-breaking changes",
            ui.out_styled(Style::Green, "+"),
        ),
    ] {
        let group: Vec<&Change> = changes.iter().filter(|c| c.severity == severity).collect();
        if group.is_empty() {
            continue;
        }
        ui.data(&format!("{title} ({}) against {label}:", group.len()));
        for c in group {
            ui.data(&format!("  {mark} {}: {}", c.subject, c.message));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_package_comes_from_declarations_and_operations() {
        assert_eq!(package_of("payments:money.Money"), Some("payments"));
        assert_eq!(package_of("veld:orders/Orders.getOrder"), Some("orders"));
        assert_eq!(package_of("nonsense"), None);
        assert_eq!(package_of(""), None);
    }

    #[test]
    fn owners_are_written_in_parentheses_only_when_there_are_some() {
        assert_eq!(owners_text(&[]), "");
        assert_eq!(owners_text(&["a".into(), "b".into()]), " (a, b)");
    }
}
