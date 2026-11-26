//! `veld export <openapi|asyncapi|rules|runtime-schema|ir>`.
//!
//! The exports print to stdout, or to `-o FILE`. A file is written with
//! `output::write_unmanaged`: staged and contained like generated output, but never tracked in a
//! manifest, so `generate` and `clean` leave it alone, and a differing existing file is refused
//! unless `--force`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use veld_codegen::FileSet;
use veld_ir::{Ir, ServiceId};
use veld_project::output::{self, PlanAction};

use crate::cli::{DocOut, ExportCommand};
use crate::error::{Exit, user};
use crate::fsutil;
use crate::pipeline;
use crate::ui::Ui;

pub fn run(ui: &Ui, config: Option<&Path>, cmd: &ExportCommand) -> Result<Exit> {
    let project = pipeline::load_project(config)?;
    let Some(done) = pipeline::compile_or_report(ui, project)? else {
        return Ok(Exit::User);
    };
    let ir = &done.ir;
    match cmd {
        ExportCommand::Ir { output } => export_ir(ui, ir, output.as_deref()),
        ExportCommand::Rules(out) => {
            let text = veld_tools::rule_book(ir, &selected_services(ir, &out.service)?);
            document(ui, out, "rules.md", &text)
        }
        ExportCommand::RuntimeSchema(args) => super::runtime_schema::run(ui, &done, args),
        ExportCommand::Asyncapi(out) => {
            let services = selected_services(ir, &out.service)?;
            let value = veld_tools::asyncapi(ir, &services)
                .map_err(|e| user(format!("E0803: `export asyncapi`: {e}")))?;
            document(ui, out, "asyncapi.json", &pretty(&value)?)
        }
        ExportCommand::Openapi(out) => {
            let value = veld_tools::openapi(ir, &selected_services(ir, &out.service)?);
            document(ui, out, "openapi.json", &pretty(&value)?)
        }
    }
}

fn export_ir(ui: &Ui, ir: &Ir, output: Option<&Path>) -> Result<Exit> {
    let text = pretty(&serde_json::to_value(ir).context("cannot serialize the IR")?)?;
    match output {
        Some(path) => {
            fsutil::atomic_write(path, text.as_bytes())?;
            ui.info(&format!("wrote {}", pipeline::display_path(path)));
        }
        None => print!("{text}"),
    }
    Ok(Exit::Ok)
}

fn pretty(value: &serde_json::Value) -> Result<String> {
    let mut text = serde_json::to_string_pretty(value).context("cannot serialize the document")?;
    text.push('\n');
    Ok(text)
}

/// Resolves repeated `--service` names (none means every service).
fn selected_services(ir: &Ir, names: &[String]) -> Result<Vec<ServiceId>> {
    if names.is_empty() {
        return Ok(ir.services.iter().map(|s| s.id).collect());
    }
    names
        .iter()
        .map(|name| match ir.service_by_name(name) {
            Some(s) => Ok(s.id),
            None => {
                let known: Vec<&str> = ir.services.iter().map(|s| s.name.as_str()).collect();
                Err(user(format!(
                    "no service named `{name}` (known services: {})",
                    known.join(", ")
                )))
            }
        })
        .collect()
}

/// Prints `text`, or writes it to `-o FILE` (`default_name` is only used when the path has no
/// file name).
pub(super) fn document(ui: &Ui, out: &DocOut, default_name: &str, text: &str) -> Result<Exit> {
    let Some(path) = &out.output else {
        print!("{text}");
        return Ok(Exit::Ok);
    };
    let name = path.file_name().map_or_else(
        || default_name.to_string(),
        |n| n.to_string_lossy().to_string(),
    );
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let mut files = FileSet::new();
    files
        .add(name, text.as_bytes().to_vec())
        .map_err(|e| user(format!("invalid output file `{}`: {e}", path.display())))?;
    report(ui, &dir, &files, out.force)
}

/// Writes through `write_unmanaged` and prints one line per file.
fn report(ui: &Ui, dir: &Path, files: &FileSet, force: bool) -> Result<Exit> {
    let plan = output::write_unmanaged(dir, files, force).map_err(|e| user(e.to_string()))?;
    for entry in &plan.entries {
        let verb = match entry.action {
            PlanAction::Create => "created",
            PlanAction::Update => "updated",
            PlanAction::Delete
            | PlanAction::Unchanged
            | PlanAction::Conflict
            | PlanAction::Modified => "unchanged",
        };
        ui.info(&format!(
            "{verb} {}",
            pipeline::display_path(&dir.join(&entry.path)).trim_start_matches("./")
        ));
    }
    Ok(Exit::Ok)
}
