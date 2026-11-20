//! `veld setup`: wire generated targets into the apps that consume them.

use std::path::Path;

use anyhow::Result;

use crate::cli::SetupArgs;
use crate::error::Exit;
use crate::pipeline;
use crate::setup::{self, Plan};
use crate::ui::Ui;

pub fn run(ui: &Ui, config: Option<&Path>, args: &SetupArgs) -> Result<Exit> {
    let project = pipeline::load_project(config)?;
    let targets = pipeline::select_targets(&project, args.target.as_deref())?;
    let plan = setup::plan(&project, &targets)?;
    report(ui, &project.root, &plan, args.dry_run);
    if !args.dry_run {
        setup::apply(&plan)?;
    }
    Ok(Exit::Ok)
}

fn report(ui: &Ui, root: &Path, plan: &Plan, dry_run: bool) {
    let verb = if dry_run { "would change" } else { "changed" };
    for (path, edit) in &plan.edits {
        if edit.original == edit.text {
            continue;
        }
        ui.info(&format!("{verb} {}", setup::relative(root, path)));
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
            ui.info(&format!(
                "next: run `npm install` in {}",
                setup::relative(root, dir)
            ));
        }
    } else if plan.manual.is_empty() && plan.notes.is_empty() {
        ui.info("nothing to do");
    }
}
