//! `veld test --from-violation <file>`: the `example` block of a captured violation
//! (`spec/rules.md` section 17.3).
//!
//! Standard output holds the block and nothing else. The notes go to standard error.

use std::path::Path;

use anyhow::Result;
use veld_testing::from_violation;

use crate::error::{Exit, user};
use crate::pipeline;
use crate::ui::Ui;

pub fn run(ui: &Ui, config: Option<&Path>, file: &Path) -> Result<Exit> {
    let text = std::fs::read_to_string(file)
        .map_err(|e| user(format!("cannot read `{}`: {e}", file.display())))?;
    let event: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| user(format!("`{}` is not JSON: {e}", file.display())))?;
    let project = pipeline::load_project(config)?;
    let Some(done) = pipeline::compile_or_report(ui, project)? else {
        return Ok(Exit::User);
    };
    let found = from_violation(&done.ir, &event).map_err(|e| user(e.to_string()))?;
    ui.data(&found.block);
    for note in &found.notes {
        eprintln!("note: {note}");
    }
    Ok(Exit::Ok)
}
