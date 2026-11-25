//! `veld clean`: remove generated files the user has not modified.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use veld_project::output;

use crate::cli::CleanArgs;
use crate::error::{Exit, user};
use crate::pipeline;
use crate::ui::Ui;

pub fn run(ui: &Ui, config: Option<&Path>, args: &CleanArgs) -> Result<Exit> {
    ui.header("clean", "");
    let project = pipeline::load_project(config)?;
    let Some(selection) = crate::selection::resolve(ui, &project, &args.select)? else {
        return Ok(Exit::Ok);
    };
    if args.lock && !selection.full {
        return Err(user(
            "`--lock` removes the baseline of the whole workspace; use it without a member selection",
        ));
    }
    let mut seen = BTreeSet::new();
    for target in selection.targets(&project) {
        if !seen.insert(target.out.clone()) {
            continue;
        }
        let report = output::clean(&target.out, args.dry_run)
            .map_err(|e| user(format!("target `{}`: {e}", target.name)))?;
        let out = pipeline::display_path(&target.out);
        let verb = if args.dry_run {
            "would remove"
        } else {
            "removed"
        };
        ui.ok(&format!(
            "{} -> {out}: {verb} {}",
            target.name,
            pipeline::plural(report.removed.len(), "file")
        ));
        for path in &report.modified {
            ui.info(&ui.out_styled(
                crate::ui::Style::Yellow,
                &format!("    kept {path} (modified since it was generated)"),
            ));
        }
    }
    if args.lock {
        clean_lock(ui, &project, args.dry_run)?;
    }
    Ok(Exit::Ok)
}

/// `clean --lock`: removes the breaking-change baseline (`veld.lock.ir.json`).
fn clean_lock(ui: &Ui, project: &veld_project::config::Project, dry_run: bool) -> Result<()> {
    let path = pipeline::lock_path(project);
    if !path.is_file() {
        ui.info(&format!("no {} to remove", pipeline::LOCK_FILE));
        return Ok(());
    }
    if !dry_run {
        std::fs::remove_file(&path)
            .map_err(|e| user(format!("cannot remove `{}`: {e}", path.display())))?;
    }
    let verb = if dry_run { "would remove" } else { "removed" };
    ui.ok(&format!(
        "{verb} {} (the breaking-change baseline)",
        pipeline::LOCK_FILE
    ));
    Ok(())
}
