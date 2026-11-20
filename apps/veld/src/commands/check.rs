//! `veld check`: compile everything and report.

use std::path::Path;

use anyhow::Result;

use crate::error::Exit;
use crate::pipeline;
use crate::ui::Ui;

pub fn run(ui: &Ui, config: Option<&Path>) -> Result<Exit> {
    let project = pipeline::load_project(config)?;
    match pipeline::compile_or_report(ui, project)? {
        Some(done) => {
            ui.info(&pipeline::success_summary(ui, &done.ir));
            Ok(Exit::Ok)
        }
        None => Ok(Exit::User),
    }
}
