//! `veld logout`: forget the stored token.

use std::path::Path;

use anyhow::Result;
use veld_registry_client::resolve_registry;

use super::{Session, fail, project_registry};
use crate::cli::LogoutArgs;
use crate::error::Exit;
use crate::pipeline;
use crate::ui::Ui;

pub fn run(ui: &Ui, config: Option<&Path>, args: &LogoutArgs) -> Result<Exit> {
    let mut session = Session::load(false)?;
    let project = pipeline::load_project(config).ok();
    let registry = resolve_registry(
        args.registry.as_deref(),
        project_registry(project.as_ref()),
        &session.creds,
    )
    .map_err(fail)?;
    if session.creds.remove(&registry) {
        session.creds.save(&session.path).map_err(fail)?;
        ui.info(&format!("logged out of {registry}"));
    } else {
        ui.info(&format!("not logged in to {registry}"));
    }
    Ok(Exit::Ok)
}
