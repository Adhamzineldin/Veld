//! `veld debug`: hidden commands for looking inside the compiler.

use std::path::Path;

use anyhow::{Context, Result};
use veld_compiler::SourceMap;

use crate::cli::DebugCommand;
use crate::error::Exit;
use crate::pipeline;
use crate::ui::Ui;

pub fn run(ui: &Ui, config: Option<&Path>, cmd: &DebugCommand) -> Result<Exit> {
    match cmd {
        DebugCommand::Ir => ir(ui, config),
        DebugCommand::Ast { file } => ast(ui, file),
    }
}

fn ir(ui: &Ui, config: Option<&Path>) -> Result<Exit> {
    let project = pipeline::load_project(config)?;
    let Some(done) = pipeline::compile_or_report(ui, project)? else {
        return Ok(Exit::User);
    };
    ui.data(&serde_json::to_string_pretty(&done.ir).context("cannot serialize the IR")?);
    Ok(Exit::Ok)
}

fn ast(ui: &Ui, file: &Path) -> Result<Exit> {
    let text = std::fs::read_to_string(file)
        .with_context(|| format!("cannot read `{}`", file.display()))?;
    let mut sources = SourceMap::default();
    let id = sources.add(file.display().to_string(), text.clone());
    let (ast, _comments, diags) = veld_compiler::parse_file(&text, id);
    ui.data(&format!("{ast:#?}"));
    let mut failed = false;
    for d in &diags {
        failed |= d.is_error();
        ui.diagnostic(&sources.render(d));
    }
    Ok(if failed { Exit::User } else { Exit::Ok })
}
