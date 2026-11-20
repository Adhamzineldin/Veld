//! `veld generate`: the full pipeline, for every target or one.

use std::path::Path;

use anyhow::Result;
use veld_project::output::{PlanAction, WriteOptions};

use crate::cli::GenerateArgs;
use crate::error::Exit;
use crate::pipeline::{self, Written};
use crate::ui::{Style, Ui};

pub fn run(ui: &Ui, config: Option<&Path>, args: &GenerateArgs) -> Result<Exit> {
    let project = pipeline::load_project(config)?;
    generate(ui, project, args)
}

/// One full run over an already loaded project. Nothing is written unless every target
/// compiled, passed its capability check and generated, and no target has a conflict.
pub fn generate(ui: &Ui, project: veld_project::config::Project, args: &GenerateArgs) -> Result<Exit> {
    pipeline::select_targets(&project, args.target.as_deref())?;
    let Some(done) = pipeline::compile_or_report(ui, project)? else {
        return Ok(Exit::User);
    };
    let targets = pipeline::select_targets(&done.project, args.target.as_deref())?;
    let mut outputs = Vec::new();
    let mut failures = Vec::new();
    for target in targets {
        let result = pipeline::build_job(&done.project, &done.ir, target)
            .and_then(|job| pipeline::run_job(&done.ir, &job));
        match result {
            Ok(files) => outputs.push((target, files)),
            Err(messages) => failures.extend(messages),
        }
    }
    if !failures.is_empty() {
        failures.iter().for_each(|m| ui.error(m));
        return Ok(Exit::User);
    }
    let opts = WriteOptions { dry_run: args.dry_run, force: args.force };
    let written = pipeline::write_all(&outputs, opts)?;
    print_results(ui, &written, args.dry_run);
    if written.iter().any(|w| !w.plan.conflicts().is_empty()) {
        return Ok(Exit::User);
    }
    if !args.dry_run && args.target.is_none() {
        pipeline::write_lock(&done.project, &done.ir)?;
    }
    Ok(Exit::Ok)
}

fn print_results(ui: &Ui, written: &[Written], dry_run: bool) {
    for w in written {
        let conflicts = w.plan.conflicts();
        if !conflicts.is_empty() {
            report_conflicts(ui, w, &conflicts);
            continue;
        }
        let [created, updated, deleted, unchanged] = pipeline::tally(&w.plan);
        let out = pipeline::display_path(&w.out);
        let line = if dry_run {
            format!("{} -> {out}: would create {created}, update {updated}, delete {deleted} ({unchanged} unchanged)", w.name)
        } else {
            format!("{} -> {out}: {created} created, {updated} updated, {deleted} deleted, {unchanged} unchanged", w.name)
        };
        ui.info(&format!("{} {line}", ui.out_styled(Style::Green, if dry_run { "~" } else { "✓" })));
        if dry_run {
            list_changes(ui, w);
        }
    }
}

fn list_changes(ui: &Ui, w: &Written) {
    for e in &w.plan.entries {
        let mark = match e.action {
            PlanAction::Create => "+",
            PlanAction::Update => "~",
            PlanAction::Delete => "-",
            PlanAction::Unchanged | PlanAction::Conflict => continue,
        };
        ui.info(&format!("    {mark} {}", e.path));
    }
}

fn report_conflicts(ui: &Ui, w: &Written, conflicts: &[String]) {
    let out = pipeline::display_path(&w.out);
    ui.error(&format!(
        "target `{}`: {} file(s) in `{out}` already exist and were not created by Veld; nothing was written",
        w.name,
        conflicts.len()
    ));
    for path in conflicts {
        eprintln!("  conflict: {path}");
    }
    eprintln!("  move them away, or re-run with --force to overwrite them");
}
