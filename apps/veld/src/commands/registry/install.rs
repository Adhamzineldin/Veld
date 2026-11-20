//! `veld install` (exactly the locked versions) and the automatic install used by
//! `check` and `generate`.

use std::path::Path;

use anyhow::Result;
use veld_project::config::Project;
use veld_project::packages::Lockfile;
use veld_registry_client::install::{Installed, ensure_installed};
use veld_registry_client::resolver::parse_range;

use super::{Session, fail};
use crate::cli::InstallArgs;
use crate::error::{Exit, user};
use crate::pipeline;
use crate::ui::Ui;

pub fn run(ui: &Ui, config: Option<&Path>, args: &InstallArgs) -> Result<Exit> {
    let project = pipeline::load_project(config)?;
    let Some(lock) = Lockfile::read(&project.root)? else {
        return Err(user(
            "there is no veld.lock; run `veld add @org/name` to resolve and pin dependencies",
        ));
    };
    check_in_sync(&project, &lock)?;
    let session = Session::load(args.allow_insecure)?;
    let done =
        ensure_installed(&project.root, &lock, &mut |p| session.download(p)).map_err(fail)?;
    report(ui, &done);
    ui.info(&format!(
        "{} installed from veld.lock",
        pipeline::plural(done.len(), "package")
    ));
    Ok(Exit::Ok)
}

/// Installs what `veld.lock` pins and the cache lacks. Without a lock this does nothing and
/// never touches the network; with every pinned package present it only verifies checksums.
pub fn auto_install(ui: &Ui, project: &Project) -> Result<()> {
    let Some(lock) = Lockfile::read(&project.root)? else {
        return Ok(());
    };
    if lock.packages.is_empty() {
        return Ok(());
    }
    let session = Session::load(false)?;
    let done =
        ensure_installed(&project.root, &lock, &mut |p| session.download(p)).map_err(fail)?;
    report(ui, &done);
    Ok(())
}

fn report(ui: &Ui, done: &[Installed]) {
    for p in done.iter().filter(|p| p.downloaded) {
        ui.info(&format!("installed {}@{}", p.name, p.version));
    }
}

/// Every `[dependencies]` entry must be pinned by a version that satisfies its range.
fn check_in_sync(project: &Project, lock: &Lockfile) -> Result<()> {
    for (name, range) in &project.dependencies {
        let pinned = lock.get(name).ok_or_else(|| {
            user(format!(
                "{name} is in veld.toml but not in veld.lock; run `veld add {name}`"
            ))
        })?;
        let ok = parse_range(range).map_err(fail)?;
        let matches = semver::Version::parse(&pinned.version).is_ok_and(|v| ok.matches(&v));
        if !matches {
            return Err(user(format!(
                "veld.lock pins {name}@{} which does not satisfy `{range}`; run `veld add {name}@{range}`",
                pinned.version
            )));
        }
    }
    Ok(())
}
