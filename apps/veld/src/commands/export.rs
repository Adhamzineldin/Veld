//! `veld export openapi|ir`.

use std::path::Path;

use anyhow::{Context, Result};
use veld_ir::ServiceId;

use crate::cli::ExportCommand;
use crate::error::{Exit, user};
use crate::fsutil;
use crate::pipeline;
use crate::ui::Ui;

pub fn run(ui: &Ui, config: Option<&Path>, cmd: &ExportCommand) -> Result<Exit> {
    let project = pipeline::load_project(config)?;
    let Some(done) = pipeline::compile_or_report(ui, project)? else {
        return Ok(Exit::User);
    };
    let (value, output) = match cmd {
        ExportCommand::Ir { output } => (serde_json::to_value(&done.ir).context("cannot serialize the IR")?, output),
        ExportCommand::Openapi { output, service } => {
            let ids = selected_services(&done.ir, service.as_deref())?;
            (veld_tools::openapi(&done.ir, &ids), output)
        }
    };
    let mut text = serde_json::to_string_pretty(&value).context("cannot serialize the document")?;
    text.push('\n');
    match output {
        Some(path) => {
            fsutil::atomic_write(path, text.as_bytes())?;
            ui.info(&format!("wrote {}", pipeline::display_path(path)));
        }
        None => print!("{text}"),
    }
    Ok(Exit::Ok)
}

fn selected_services(ir: &veld_ir::Ir, name: Option<&str>) -> Result<Vec<ServiceId>> {
    let Some(name) = name else {
        return Ok(ir.services.iter().map(|s| s.id).collect());
    };
    match ir.service_by_name(name) {
        Some(s) => Ok(vec![s.id]),
        None => {
            let known: Vec<&str> = ir.services.iter().map(|s| s.name.as_str()).collect();
            Err(user(format!("no service named `{name}` (known services: {})", known.join(", "))))
        }
    }
}
