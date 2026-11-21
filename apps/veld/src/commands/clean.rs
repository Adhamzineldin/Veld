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
    let mut seen = BTreeSet::new();
    for target in &project.targets {
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
    Ok(Exit::Ok)
}
