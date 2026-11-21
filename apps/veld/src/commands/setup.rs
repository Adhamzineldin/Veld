//! `veld setup`: wire generated targets into the apps that consume them.

use std::path::Path;

use anyhow::Result;

use crate::cli::SetupArgs;
use crate::error::Exit;
use crate::pipeline;
use crate::setup::{self, Plan};
use crate::ui::{Style, Ui};

pub fn run(ui: &Ui, config: Option<&Path>, args: &SetupArgs) -> Result<Exit> {
    ui.header("setup", "");
    let project = pipeline::load_project(config)?;
    let targets = pipeline::select_targets(&project, args.target.as_deref())?;
    apply_to(ui, &project, &targets, args.dry_run)?;
    Ok(Exit::Ok)
}

/// Plans, reports and (unless `dry_run`) applies the app wiring for `targets`. Also used by
/// `veld generate --setup`.
pub fn apply_to(
    ui: &Ui,
    project: &veld_project::config::Project,
    targets: &[&veld_project::config::TargetSpec],
    dry_run: bool,
) -> Result<()> {
    let plan = setup::plan(project, targets)?;
    report(ui, &project.root, &plan, dry_run);
    if !dry_run {
        setup::apply(&plan)?;
    }
    Ok(())
}

fn report(ui: &Ui, root: &Path, plan: &Plan, dry_run: bool) {
    let verb = if dry_run { "would change" } else { "changed" };
    for (path, edit) in &plan.edits {
        if edit.original == edit.text {
            continue;
        }
        let shown = setup::relative(root, path);
        if dry_run {
            ui.info(&format!(
                "{} {verb} {shown}",
                ui.out_styled(Style::Yellow, "~")
            ));
        } else {
            ui.ok(&format!("{verb} {shown}"));
        }
        for line in &edit.lines {
            ui.info(&format!("  {line}"));
        }
    }
    for note in &plan.notes {
        ui.info(note);
    }
    for m in &plan.manual {
        ui.warn(&format!(
            "{}: cannot edit `{}` safely ({}); add this yourself:",
            m.target,
            setup::relative(root, &m.file),
            m.reason
        ));
        for line in m.snippet.lines() {
            ui.info(&format!("    {line}"));
        }
    }
    if dry_run {
        ui.info("dry run: nothing was written");
    } else if plan.changes() {
        for dir in &plan.npm_dirs {
            ui.hint(&format!(
                "next: run `npm install` in {}",
                setup::relative(root, dir)
            ));
        }
    } else if plan.manual.is_empty() && plan.notes.is_empty() {
        ui.info("nothing to do");
    }
}
