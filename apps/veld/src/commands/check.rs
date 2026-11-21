//! `veld check`: compile everything and report.

use std::path::Path;
use std::time::Instant;

use anyhow::Result;

use crate::error::Exit;
use crate::pipeline;
use crate::ui::render;
use crate::ui::{Style, Ui};

pub fn run(ui: &Ui, config: Option<&Path>) -> Result<Exit> {
    let project = pipeline::load_project(config)?;
    ui.header("check", &project.name);
    super::registry::auto_install(ui, &project)?;
    let started = Instant::now();
    match pipeline::compile_or_report(ui, project)? {
        Some(done) => {
            ui.info(&format!(
                "{} {}",
                pipeline::success_summary(ui, &done.ir),
                ui.out_styled(
                    Style::Dim,
                    &format!("in {}", render::elapsed(started.elapsed()))
                )
            ));
            Ok(Exit::Ok)
        }
        None => Ok(Exit::User),
    }
}
