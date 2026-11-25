//! `veld publish`: pack the project's library or service and upload it.

use std::path::Path;

use anyhow::Result;
use veld_registry_client::publish::plan;
use veld_registry_client::resolve_registry;

use super::{Session, fail};
use crate::cli::PublishArgs;
use crate::error::Exit;
use crate::pipeline;
use crate::ui::Ui;

pub fn run(ui: &Ui, config: Option<&Path>, args: &PublishArgs) -> Result<Exit> {
    let project = pipeline::load_project(config)?;
    let plan = plan(&project, args.package.as_deref()).map_err(fail)?;
    let size = plan.packed.bytes.len();
    ui.header("publish", &plan.name);
    ui.info(&format!(
        "{}@{}: {} files, {size} bytes, sha256 {}",
        plan.name, plan.version, plan.files, plan.packed.sha256
    ));
    if args.dry_run {
        ui.info("dry run: nothing was uploaded");
        ui.hint("run `veld publish` without --dry-run to upload");
        return Ok(Exit::Ok);
    }
    let session = Session::load(args.allow_insecure)?;
    let registry = resolve_registry(
        args.registry.as_deref(),
        project.registry.as_deref(),
        &session.creds,
    )
    .map_err(fail)?;
    let client = session.authed_client(&registry)?;
    let (org, name) = plan.parts().map_err(fail)?;
    let spin = ui.spinner(&format!("Uploading {}@{}", plan.name, plan.version));
    let sent = client.publish(org, name, &plan.version, &plan.packed.bytes);
    spin.finish();
    sent.map_err(fail)?;
    ui.ok(&format!(
        "published {}@{} to {}",
        plan.name,
        plan.version,
        client.base_url()
    ));
    Ok(Exit::Ok)
}
