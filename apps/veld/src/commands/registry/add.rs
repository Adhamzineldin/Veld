//! `veld add @org/name[@range]`: record the dependency, resolve it, install it and lock it.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use veld_project::packages::{Lockfile, split_name};
use veld_registry_client::depsedit::set_dependency;
use veld_registry_client::install::ensure_installed;
use veld_registry_client::resolve_registry;
use veld_registry_client::resolver::{parse_range, resolve_preferring};

use super::{Session, fail};
use crate::cli::AddArgs;
use crate::error::{Exit, user};
use crate::fsutil::atomic_write;
use crate::pipeline;
use crate::ui::Ui;

/// Splits `@org/name` or `@org/name@range`.
fn parse_spec(spec: &str) -> Result<(&str, Option<&str>)> {
    let (name, range) = match spec.get(1..).and_then(|rest| rest.find('@')) {
        Some(i) => (&spec[..=i], Some(&spec[i + 2..])),
        None => (spec, None),
    };
    if split_name(name).is_none() {
        return Err(user(format!(
            "`{spec}` is not a package reference; use @org/name or @org/name@^1.2"
        )));
    }
    Ok((name, range))
}

pub fn run(ui: &Ui, config: Option<&Path>, args: &AddArgs) -> Result<Exit> {
    let (name, range) = parse_spec(&args.package)?;
    ui.header("add", name);
    if let Some(r) = range {
        parse_range(r).map_err(fail)?;
    }
    let project = pipeline::load_project(config)?;
    // `[dependencies]` belongs to the workspace root, whichever member the command runs in.
    let config_file = project.file.clone();
    let session = Session::load(args.allow_insecure)?;
    let registry = resolve_registry(
        args.registry.as_deref(),
        project.registry.as_deref(),
        &session.creds,
    )
    .map_err(fail)?;
    let client = session.client(&registry)?;
    let registry = client.base_url().to_string();

    let old = Lockfile::read(&project.root)?.unwrap_or_default();
    let mut deps = project.dependencies.clone();
    deps.insert(name.to_string(), range.unwrap_or("*").to_string());
    // Resolve the whole graph first, so an unknown package, a bad range or a conflict changes
    // nothing. Packages the lock already pins keep their version when it still fits; the added
    // package is re-resolved to the newest.
    let prefer: BTreeMap<String, String> = old
        .packages
        .iter()
        .filter(|p| p.name != name)
        .map(|p| (p.name.clone(), p.version.clone()))
        .collect();
    let lock = resolve_preferring(&deps, &registry, &client, &prefer).map_err(fail)?;
    let chosen = lock.get(name).map(|p| p.version.clone()).ok_or_else(|| {
        user(format!(
            "internal error: {name} is missing from the resolution"
        ))
    })?;
    let written = range.map_or_else(|| format!("^{chosen}"), str::to_string);
    ensure_installed(&project.root, &lock, &mut |p| session.download(p)).map_err(fail)?;

    let text = std::fs::read_to_string(&config_file)
        .with_context(|| format!("cannot read `{}`", config_file.display()))?;
    let set_registry = project.registry.is_none().then_some(registry.as_str());
    let edited = set_dependency(&text, name, &written, set_registry).map_err(fail)?;
    commit(&config_file, &text, &edited, &project.root, &lock)?;
    let transitive = lock.packages.len().saturating_sub(deps.len());
    let extra = match transitive {
        0 => String::new(),
        1 => ", plus 1 package it depends on".to_string(),
        n => format!(", plus {n} packages it depends on"),
    };
    ui.ok(&format!(
        "added {name}@{chosen} (veld.toml and veld.lock updated{extra})"
    ));
    Ok(Exit::Ok)
}

/// Writes `veld.toml` and `veld.lock` together: if the second write fails, the first is undone.
fn commit(
    config_file: &Path,
    old_text: &str,
    new_text: &str,
    root: &Path,
    lock: &Lockfile,
) -> Result<()> {
    atomic_write(config_file, new_text.as_bytes())?;
    let lock_path = root.join(veld_project::packages::LOCK_FILE);
    if let Err(e) = atomic_write(&lock_path, lock.to_toml().as_bytes()) {
        let _ = atomic_write(config_file, old_text.as_bytes());
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_specs() {
        assert_eq!(
            parse_spec("@acme/billing").unwrap(),
            ("@acme/billing", None)
        );
        assert_eq!(
            parse_spec("@acme/billing@^1.2").unwrap(),
            ("@acme/billing", Some("^1.2"))
        );
        assert_eq!(
            parse_spec("@acme/billing@1.2.3").unwrap(),
            ("@acme/billing", Some("1.2.3"))
        );
        for bad in [
            "billing",
            "@acme",
            "@acme/",
            "acme/billing@1",
            "",
            "@",
            "@a/b/c",
        ] {
            assert!(parse_spec(bad).is_err(), "{bad}");
        }
    }
}
