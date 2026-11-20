//! `veld add @org/name[@range]`: record the dependency, resolve it, install it and lock it.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use veld_project::packages::{Lockfile, split_name};
use veld_registry_client::depsedit::set_dependency;
use veld_registry_client::install::ensure_installed;
use veld_registry_client::resolve_registry;
use veld_registry_client::resolver::{parse_range, resolve};

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
    if let Some(r) = range {
        parse_range(r).map_err(fail)?;
    }
    let config_file = pipeline::config_file(config)?;
    let project = veld_project::config::load(&config_file)?;
    let session = Session::load(args.allow_insecure)?;
    let registry = resolve_registry(
        args.registry.as_deref(),
        project.registry.as_deref(),
        &session.creds,
    )
    .map_err(fail)?;
    let client = session.client(&registry)?;
    let registry = client.base_url().to_string();

    let mut deps = project.dependencies.clone();
    let wanted = range.unwrap_or("*").to_string();
    // Resolve the new dependency first, so an unknown package or range changes nothing.
    let fresh = resolve(
        &BTreeMap::from([(name.to_string(), wanted)]),
        &registry,
        &client,
    )
    .map_err(fail)?;
    let chosen = fresh.packages[0].clone();
    let written = range.map_or_else(|| format!("^{}", chosen.version), str::to_string);
    deps.insert(name.to_string(), written.clone());

    let lock = merged_lock(&project.root, &deps, name, chosen, &registry, &client)?;
    ensure_installed(&project.root, &lock, &mut |p| session.download(p)).map_err(fail)?;

    let text = std::fs::read_to_string(&config_file)
        .with_context(|| format!("cannot read `{}`", config_file.display()))?;
    let set_registry = project.registry.is_none().then_some(registry.as_str());
    let edited = set_dependency(&text, name, &written, set_registry).map_err(fail)?;
    atomic_write(&config_file, edited.as_bytes())?;
    atomic_write(
        &project.root.join(veld_project::packages::LOCK_FILE),
        lock.to_toml().as_bytes(),
    )?;
    let version = &lock
        .get(name)
        .map(|p| p.version.clone())
        .unwrap_or_default();
    ui.info(&format!(
        "added {name}@{version} (veld.toml and veld.lock updated)"
    ));
    Ok(Exit::Ok)
}

/// The existing pins that still satisfy `[dependencies]`, plus the new one; anything else
/// that is declared but not pinned is resolved.
fn merged_lock(
    root: &Path,
    deps: &BTreeMap<String, String>,
    added: &str,
    chosen: veld_project::packages::LockedPackage,
    registry: &str,
    client: &veld_registry_client::Client,
) -> Result<Lockfile> {
    let old = Lockfile::read(root)?.unwrap_or_default();
    let mut packages = vec![chosen];
    let mut unresolved = BTreeMap::new();
    for (dep, range) in deps.iter().filter(|(d, _)| d.as_str() != added) {
        let kept = old.get(dep).filter(|p| {
            parse_range(range)
                .is_ok_and(|req| semver::Version::parse(&p.version).is_ok_and(|v| req.matches(&v)))
        });
        match kept {
            Some(p) => packages.push(p.clone()),
            None => {
                unresolved.insert(dep.clone(), range.clone());
            }
        }
    }
    if !unresolved.is_empty() {
        packages.extend(
            resolve(&unresolved, registry, client)
                .map_err(fail)?
                .packages,
        );
    }
    Ok(Lockfile { packages })
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
