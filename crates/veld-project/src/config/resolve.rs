//! Turns a parsed [`Config`] into a validated [`Project`].

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use veld_ir::PackageKind;

use super::error::{ConfigError, Warning};
use super::paths::normalize;
use super::raw::OneOrMany;
use super::raw::{Config, PluginSection, Role, ServiceSection, TargetSection};
use super::suggest::{closest, hint};
use super::{EmitterRef, Hooks, PackageSpec, PluginSpec, Project, TargetSpec};

pub(super) fn resolve(root: &Path, cfg: Config) -> Result<Project, ConfigError> {
    if cfg.project.name.trim().is_empty() {
        return Err(ConfigError::invalid("`[project].name` is required"));
    }
    let mut warnings = Vec::new();
    let packages = build_packages(root, &cfg, &mut warnings)?;
    let plugins = build_plugins(&cfg.plugins)?;
    let targets = build_targets(root, &cfg, &packages)?;
    check_outputs(root, &targets, &packages)?;
    Ok(Project {
        root: root.to_path_buf(),
        name: cfg.project.name,
        description: cfg.project.description,
        packages,
        targets,
        plugins,
        registry: cfg.registry.and_then(|r| r.url),
        dependencies: cfg.dependencies,
        lint: cfg.lint,
        hooks: Hooks {
            post_generate: match cfg.hooks.post_generate {
                None => Vec::new(),
                Some(OneOrMany::One(c)) => vec![c],
                Some(OneOrMany::Many(c)) => c,
            },
            fail: cfg.hooks.fail,
        },
        warnings,
    })
}

fn valid_name(kind: &str, name: &str) -> Result<(), ConfigError> {
    let ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if ok {
        Ok(())
    } else {
        Err(ConfigError::invalid(format!(
            "{kind} name `{name}` must be non-empty and use only letters, digits, `_` and `-`"
        )))
    }
}

fn package_root(root: &Path, contracts: &str) -> PathBuf {
    normalize(&root.join(contracts))
}

fn build_packages(
    root: &Path,
    cfg: &Config,
    warnings: &mut Vec<Warning>,
) -> Result<Vec<PackageSpec>, ConfigError> {
    let mut packages = Vec::new();
    for (name, lib) in &cfg.libraries {
        valid_name("library", name)?;
        let contracts = lib.contracts.as_deref().ok_or_else(|| {
            ConfigError::invalid(format!("`[libraries.{name}]` is missing `contracts`"))
        })?;
        packages.push(PackageSpec {
            name: name.clone(),
            kind: PackageKind::Library,
            root: package_root(root, contracts),
            base_url: None,
            consumes: Vec::new(),
            prefix: String::new(),
        });
    }
    if cfg.services.is_empty() {
        let contracts = cfg.project.contracts.as_deref().unwrap_or("contracts");
        packages.push(PackageSpec {
            name: cfg.project.name.clone(),
            kind: PackageKind::Service,
            root: package_root(root, contracts),
            base_url: None,
            consumes: Vec::new(),
            prefix: join_prefix(cfg.project.prefix.as_deref(), None)?,
        });
    } else {
        if cfg.project.contracts.is_some() {
            return Err(ConfigError::invalid(
                "`[project].contracts` cannot be combined with `[services.*]`",
            ));
        }
        for (name, svc) in &cfg.services {
            packages.push(build_service(root, name, svc, cfg, warnings)?);
        }
    }
    check_unique_package_names(&packages)?;
    Ok(packages)
}

fn build_service(
    root: &Path,
    name: &str,
    svc: &ServiceSection,
    cfg: &Config,
    warnings: &mut Vec<Warning>,
) -> Result<PackageSpec, ConfigError> {
    valid_name("service", name)?;
    let contracts = svc.contracts.as_deref().ok_or_else(|| {
        ConfigError::invalid(format!("`[services.{name}]` is missing `contracts`"))
    })?;
    Ok(PackageSpec {
        name: name.to_string(),
        kind: PackageKind::Service,
        root: package_root(root, contracts),
        base_url: svc.base_url.clone(),
        consumes: resolve_consumes(name, &svc.consumes, cfg, warnings)?,
        prefix: join_prefix(cfg.project.prefix.as_deref(), svc.prefix.as_deref())?,
    })
}

/// Joins the project and service route prefixes (`/api` + `/v1` = `/api/v1`). Each must start with
/// `/`; trailing slashes are dropped. `/` alone means none.
fn join_prefix(project: Option<&str>, service: Option<&str>) -> Result<String, ConfigError> {
    let mut out = String::new();
    for (key, value) in [
        ("[project].prefix", project),
        ("[services.<name>].prefix", service),
    ] {
        let Some(v) = value else { continue };
        if !v.starts_with('/') {
            return Err(ConfigError::invalid(format!(
                "`{key}` must start with `/`, found `{v}`"
            )));
        }
        out.push_str(v.trim_end_matches('/'));
    }
    Ok(out)
}

fn resolve_consumes(
    owner: &str,
    consumes: &[String],
    cfg: &Config,
    warnings: &mut Vec<Warning>,
) -> Result<Vec<String>, ConfigError> {
    let mut resolved: Vec<String> = Vec::new();
    for name in consumes {
        if name == "*" {
            for other in cfg.services.keys().filter(|s| s.as_str() != owner) {
                if !resolved.contains(other) {
                    resolved.push(other.clone());
                }
            }
        } else if name == owner {
            warnings.push(Warning {
                code: "W0701",
                message: format!("service `{owner}` lists itself in `consumes`; ignoring"),
            });
        } else if !cfg.services.contains_key(name) {
            return Err(unknown_service(
                &format!("service `{owner}`"),
                name,
                cfg.services.keys().map(String::as_str),
            ));
        } else if !resolved.contains(name) {
            resolved.push(name.clone());
        }
    }
    Ok(resolved)
}

fn unknown_service<'a>(
    owner: &str,
    name: &str,
    known: impl Iterator<Item = &'a str> + Clone,
) -> ConfigError {
    let suggestion = closest(name, known.clone());
    let valid: Vec<&str> = known.collect();
    ConfigError::UnknownService {
        owner: owner.to_string(),
        name: name.to_string(),
        hint: hint(suggestion.as_deref(), &valid).replace("valid keys", "known services"),
        suggestion,
    }
}

fn check_unique_package_names(packages: &[PackageSpec]) -> Result<(), ConfigError> {
    let mut seen = BTreeSet::new();
    match packages.iter().find(|p| !seen.insert(p.name.as_str())) {
        Some(dup) => Err(ConfigError::invalid(format!(
            "`{}` is declared as both a library and a service",
            dup.name
        ))),
        None => Ok(()),
    }
}

fn build_plugins(
    plugins: &std::collections::BTreeMap<String, PluginSection>,
) -> Result<Vec<PluginSpec>, ConfigError> {
    plugins
        .iter()
        .map(|(name, p)| {
            let command = p.command.clone().ok_or_else(|| {
                ConfigError::invalid(format!("`[plugins.{name}]` is missing `command`"))
            })?;
            Ok(PluginSpec {
                name: name.clone(),
                command,
                version: p.version.clone(),
                sha256: p.sha256.clone(),
                entry: p.entry.clone(),
            })
        })
        .collect()
}

fn build_targets(
    root: &Path,
    cfg: &Config,
    packages: &[PackageSpec],
) -> Result<Vec<TargetSpec>, ConfigError> {
    let services: Vec<&str> = packages
        .iter()
        .filter(|p| p.kind == PackageKind::Service)
        .map(|p| p.name.as_str())
        .collect();
    let mut targets = cfg
        .targets
        .iter()
        .enumerate()
        .map(|(i, t)| build_target(root, cfg, t, &services, i + 1))
        .collect::<Result<Vec<_>, _>>()?;
    assign_names(&cfg.targets, &mut targets)?;
    Ok(targets)
}

fn build_target(
    root: &Path,
    cfg: &Config,
    t: &TargetSection,
    services: &[&str],
    n: usize,
) -> Result<TargetSpec, ConfigError> {
    let missing =
        |key: &str| ConfigError::invalid(format!("`[[targets]]` #{n} is missing `{key}`"));
    let role = t.role.ok_or_else(|| missing("role"))?;
    let emitter = parse_emitter(
        cfg,
        t.emitter.as_deref().ok_or_else(|| missing("emitter"))?,
        n,
    )?;
    let out = normalize(&root.join(t.out.as_deref().ok_or_else(|| missing("out"))?));
    let (service, target_services) = target_services(role, t, services, n)?;
    Ok(TargetSpec {
        name: String::new(),
        role,
        emitter,
        framework: t.framework.clone(),
        service,
        services: target_services,
        out,
        clients: t.clients.unwrap_or(true),
        app: t.app.as_deref().map(|a| normalize(&root.join(a))),
        options: t.options.clone(),
    })
}

fn parse_emitter(cfg: &Config, id: &str, n: usize) -> Result<EmitterRef, ConfigError> {
    match id.strip_prefix("plugin:") {
        Some("") => Err(ConfigError::invalid(format!(
            "`[[targets]]` #{n}: `plugin:` needs a plugin name"
        ))),
        Some(name) if cfg.plugins.contains_key(name) => Ok(EmitterRef::Plugin(name.to_string())),
        Some(name) => Err(ConfigError::invalid(format!(
            "`[[targets]]` #{n}: plugin `{name}` is not declared under `[plugins.{name}]`"
        ))),
        None if id.is_empty() => Err(ConfigError::invalid(format!(
            "`[[targets]]` #{n}: `emitter` is empty"
        ))),
        None => Ok(EmitterRef::Builtin(id.to_string())),
    }
}

/// Servers serve one `service` (implied when there is only one); clients cover `services`.
fn target_services(
    role: Role,
    t: &TargetSection,
    services: &[&str],
    n: usize,
) -> Result<(Option<String>, Vec<String>), ConfigError> {
    let owner = format!("`[[targets]]` #{n}");
    match role {
        Role::Server => {
            if t.services.is_some() {
                return Err(ConfigError::invalid(format!(
                    "{owner}: `services` is only valid for role = \"client\" or \"spec\""
                )));
            }
            let name = match (&t.service, services) {
                (Some(s), _) => s.as_str(),
                (None, [only]) => only,
                (None, _) => {
                    return Err(ConfigError::invalid(format!(
                        "{owner}: role = \"server\" requires `service`"
                    )));
                }
            };
            check_known(&owner, name, services)?;
            Ok((Some(name.to_string()), Vec::new()))
        }
        Role::Client => {
            if t.service.is_some() {
                return Err(ConfigError::invalid(format!(
                    "{owner}: `service` is only valid for role = \"server\""
                )));
            }
            let list = match &t.services {
                Some(list) => list.clone(),
                None => services.iter().map(|s| s.to_string()).collect(),
            };
            list.iter()
                .try_for_each(|s| check_known(&owner, s, services))?;
            Ok((None, dedup(list)))
        }
    }
}

fn dedup(list: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in list {
        if !out.contains(&item) {
            out.push(item);
        }
    }
    out
}

fn check_known(owner: &str, name: &str, services: &[&str]) -> Result<(), ConfigError> {
    if services.contains(&name) {
        Ok(())
    } else {
        Err(unknown_service(owner, name, services.iter().copied()))
    }
}

/// Explicit names must be unique; unnamed targets get `<role>-<emitter>[-N]`.
fn assign_names(raw: &[TargetSection], targets: &mut [TargetSpec]) -> Result<(), ConfigError> {
    let mut taken = BTreeSet::new();
    for name in raw.iter().filter_map(|t| t.name.as_deref()) {
        if name.is_empty() || !taken.insert(name.to_string()) {
            return Err(ConfigError::invalid(format!(
                "target name `{name}` is empty or used more than once"
            )));
        }
    }
    for (spec, r) in targets.iter_mut().zip(raw) {
        spec.name = match &r.name {
            Some(name) => name.clone(),
            None => unique_default_name(spec, &mut taken),
        };
    }
    Ok(())
}

fn unique_default_name(spec: &TargetSpec, taken: &mut BTreeSet<String>) -> String {
    let role = if spec.role == Role::Server {
        "server"
    } else {
        "client"
    };
    let (EmitterRef::Builtin(e) | EmitterRef::Plugin(e)) = &spec.emitter;
    let base = format!("{role}-{e}");
    let mut name = base.clone();
    let mut n = 2;
    while !taken.insert(name.clone()) {
        name = format!("{base}-{n}");
        n += 1;
    }
    name
}

fn fold(p: &Path) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(p.to_string_lossy().to_lowercase())
    } else {
        p.to_path_buf()
    }
}

fn nested(a: &Path, b: &Path) -> bool {
    let (a, b) = (fold(a), fold(b));
    a.starts_with(&b) || b.starts_with(&a)
}

fn check_outputs(
    root: &Path,
    targets: &[TargetSpec],
    packages: &[PackageSpec],
) -> Result<(), ConfigError> {
    for t in targets {
        check_out_allowed(root, t, packages)?;
    }
    for (i, a) in targets.iter().enumerate() {
        for b in &targets[i + 1..] {
            if nested(&a.out, &b.out) {
                let reason = if fold(&a.out) == fold(&b.out) {
                    "same directory"
                } else {
                    "one is inside the other"
                };
                return Err(ConfigError::OutputOverlap {
                    first: a.name.clone(),
                    second: b.name.clone(),
                    reason,
                });
            }
        }
    }
    Ok(())
}

fn check_out_allowed(
    root: &Path,
    t: &TargetSpec,
    packages: &[PackageSpec],
) -> Result<(), ConfigError> {
    let forbid = |reason: String| ConfigError::OutputForbidden {
        target: t.name.clone(),
        reason,
    };
    if fold(&t.out) == fold(root) {
        return Err(forbid("is the project root".into()));
    }
    match packages.iter().find(|p| nested(&t.out, &p.root)) {
        Some(p) => Err(forbid(format!(
            "overlaps the contracts directory of `{}` (inside it, or containing it)",
            p.name
        ))),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::super::load_str;
    use super::*;

    fn root() -> PathBuf {
        std::env::temp_dir().join("veld-resolve-proj")
    }

    fn load(text: &str) -> Result<Project, ConfigError> {
        load_str(&root(), text)
    }

    const SINGLE: &str = r#"
[project]
name = "shop"
[[targets]]
role = "server"
emitter = "node"
framework = "express"
out = "../api/src/generated"
[[targets]]
role = "client"
emitter = "typescript"
out = "../web/src/generated"
"#;

    const WORKSPACE: &str = r#"
[project]
name = "bank"
[libraries.common]
contracts = "contracts/common"
[services.iam]
contracts = "contracts/iam"
base-url = "http://iam:3001"
[services.orders]
contracts = "contracts/orders"
consumes = ["iam", "payments"]
[services.payments]
contracts = "contracts/payments"
consumes = ["orders"]
[[targets]]
name = "orders-api"
service = "orders"
role = "server"
emitter = "node"
out = "services/orders/src/generated"
[[targets]]
role = "client"
services = ["iam", "orders"]
emitter = "typescript"
out = "web/src/generated"
"#;

    #[test]
    fn single_service_mode() {
        let p = load(SINGLE).unwrap();
        assert_eq!(p.packages.len(), 1);
        assert_eq!(p.packages[0].name, "shop");
        assert_eq!(p.packages[0].root, normalize(&root().join("contracts")));
        assert_eq!(p.targets[0].service.as_deref(), Some("shop"));
        assert_eq!(p.targets[1].services, ["shop"]);
        assert_eq!(
            p.targets[0].out,
            normalize(&root().join("../api/src/generated"))
        );
        assert!(p.targets[0].out.is_absolute());
        assert_eq!(p.targets[0].name, "server-node");
        assert_eq!(p.targets[1].name, "client-typescript");
        assert!(p.targets[0].clients);
        assert_eq!(p.targets[0].app, None);
    }

    #[test]
    fn app_is_resolved_against_the_project_root() {
        let text = SINGLE.replace(
            "out = \"../web/src/generated\"
",
            "out = \"../web/src/generated\"
app = \"../web\"
",
        );
        let p = load(&text).unwrap();
        assert_eq!(p.targets[1].app, Some(normalize(&root().join("../web"))));
        assert_eq!(p.targets[0].app, None);
    }

    #[test]
    fn workspace_mode_with_cycles() {
        let p = load(WORKSPACE).unwrap();
        assert_eq!(
            p.packages
                .iter()
                .map(|x| x.name.as_str())
                .collect::<Vec<_>>(),
            ["common", "iam", "orders", "payments"]
        );
        assert_eq!(p.package("common").unwrap().kind, PackageKind::Library);
        assert_eq!(p.package("orders").unwrap().consumes, ["iam", "payments"]);
        assert_eq!(
            p.package("iam").unwrap().base_url.as_deref(),
            Some("http://iam:3001")
        );
        assert_eq!(p.targets[0].name, "orders-api");
        assert_eq!(p.targets[1].services, ["iam", "orders"]);
        assert!(p.warnings.is_empty());
    }

    #[test]
    fn client_services_default_to_all_services_not_libraries() {
        let text = WORKSPACE.replace("services = [\"iam\", \"orders\"]\n", "");
        let p = load(&text).unwrap();
        assert_eq!(p.targets[1].services, ["iam", "orders", "payments"]);
    }

    #[test]
    fn unknown_consumed_service_is_e0702_with_suggestion() {
        let err = load(&WORKSPACE.replace("\"iam\", \"payments\"", "\"iamm\", \"payments\""))
            .unwrap_err();
        assert_eq!(err.code(), Some("E0702"));
        assert!(err.to_string().contains("did you mean `iam`"), "{err}");
    }

    #[test]
    fn consuming_a_library_is_unknown_service() {
        let err = load(&WORKSPACE.replace("\"iam\", \"payments\"", "\"common\"")).unwrap_err();
        assert_eq!(err.code(), Some("E0702"));
    }

    #[test]
    fn consumes_wildcard_expands_to_every_other_service() {
        let p = load(&WORKSPACE.replace("[\"iam\", \"payments\"]", "[\"*\"]")).unwrap();
        assert_eq!(p.package("orders").unwrap().consumes, ["iam", "payments"]);
        assert!(p.warnings.is_empty());
        let p =
            load(&WORKSPACE.replace("[\"iam\", \"payments\"]", "[\"payments\", \"*\"]")).unwrap();
        assert_eq!(p.package("orders").unwrap().consumes, ["payments", "iam"]);
    }

    #[test]
    fn prefixes_join_project_then_service() {
        let text = WORKSPACE
            .replace(
                "name = \"bank\"",
                "name = \"bank\"
prefix = \"/api/\"",
            )
            .replace("base-url = \"http://iam:3001\"", "prefix = \"/v1\"");
        let p = load(&text).unwrap();
        assert_eq!(p.package("iam").unwrap().prefix, "/api/v1");
        assert_eq!(p.package("orders").unwrap().prefix, "/api");
        assert_eq!(p.package("common").unwrap().prefix, "");
        let map = p.prefixes();
        assert_eq!(map.len(), 3);
        assert_eq!(map["iam"], "/api/v1");
    }

    #[test]
    fn single_service_project_prefix_and_bad_prefix() {
        let p = load(&SINGLE.replace(
            "name = \"shop\"",
            "name = \"shop\"
prefix = \"/api\"",
        ))
        .unwrap();
        assert_eq!(p.packages[0].prefix, "/api");
        let err = load(&SINGLE.replace(
            "name = \"shop\"",
            "name = \"shop\"
prefix = \"api\"",
        ))
        .unwrap_err();
        assert!(err.to_string().contains("must start with"), "{err}");
    }

    #[test]
    fn self_consumption_warns() {
        let p = load(&WORKSPACE.replace("\"iam\", \"payments\"", "\"orders\", \"iam\"")).unwrap();
        assert_eq!(p.warnings.len(), 1);
        assert_eq!(p.warnings[0].code, "W0701");
        assert_eq!(p.package("orders").unwrap().consumes, ["iam"]);
    }

    #[test]
    fn duplicate_and_nested_outs_are_e0703() {
        let same = SINGLE.replace("../web/src/generated", "../api/src/generated");
        assert_eq!(load(&same).unwrap_err().code(), Some("E0703"));
        let nested = SINGLE.replace("../web/src/generated", "../api/src/generated/web");
        assert_eq!(load(&nested).unwrap_err().code(), Some("E0703"));
        let reverse = SINGLE.replace("../web/src/generated", "../api/src");
        assert_eq!(load(&reverse).unwrap_err().code(), Some("E0703"));
        let dotted = SINGLE.replace("../web/src/generated", "../x/../api/src/generated/");
        assert_eq!(load(&dotted).unwrap_err().code(), Some("E0703"));
    }

    #[test]
    fn forbidden_outs_are_e0704() {
        for out in [
            ".",
            "contracts",
            "contracts/sub",
            "sub/../contracts/x",
            "..",
        ] {
            let text = SINGLE.replace("../web/src/generated", out);
            let err = load(&text).unwrap_err();
            assert_eq!(err.code(), Some("E0704"), "out = {out}: {err}");
        }
    }

    #[test]
    fn target_rules() {
        let no_service = WORKSPACE.replace("service = \"orders\"\n", "");
        assert_eq!(load(&no_service).unwrap_err().code(), Some("E0700"));
        let bad_service = WORKSPACE.replace("service = \"orders\"", "service = \"nope\"");
        assert_eq!(load(&bad_service).unwrap_err().code(), Some("E0702"));
        let missing_out = SINGLE.replace("out = \"../web/src/generated\"\n", "");
        assert_eq!(load(&missing_out).unwrap_err().code(), Some("E0700"));
        let dup_name = format!(
            "{SINGLE}\n[[targets]]\nname=\"a\"\nrole=\"client\"\nemitter=\"x\"\nout=\"o1\"\n[[targets]]\nname=\"a\"\nrole=\"client\"\nemitter=\"x\"\nout=\"o2\"\n"
        );
        assert_eq!(load(&dup_name).unwrap_err().code(), Some("E0700"));
    }

    #[test]
    fn default_target_names_do_not_collide() {
        let text =
            format!("{SINGLE}\n[[targets]]\nrole=\"client\"\nemitter=\"typescript\"\nout=\"o3\"\n");
        let p = load(&text).unwrap();
        assert_eq!(p.targets[2].name, "client-typescript-2");
    }

    #[test]
    fn plugin_emitters() {
        let text = format!(
            "{SINGLE}\n[[targets]]\nrole=\"client\"\nemitter=\"plugin:kotlin\"\nout=\"k\"\n[plugins.kotlin]\ncommand=\"veld-emit-kotlin\"\n"
        );
        let p = load(&text).unwrap();
        assert_eq!(p.targets[2].emitter, EmitterRef::Plugin("kotlin".into()));
        assert_eq!(p.plugins[0].command, "veld-emit-kotlin");
        let undeclared = format!(
            "{SINGLE}\n[[targets]]\nrole=\"client\"\nemitter=\"plugin:kotlin\"\nout=\"k\"\n"
        );
        assert_eq!(load(&undeclared).unwrap_err().code(), Some("E0700"));
    }

    #[test]
    fn project_contracts_conflicts_with_services() {
        let text = WORKSPACE.replace("name = \"bank\"", "name = \"bank\"\ncontracts = \"c\"");
        assert_eq!(load(&text).unwrap_err().code(), Some("E0700"));
    }

    #[test]
    fn options_registry_and_dependencies_carry_through() {
        let text = format!(
            "{SINGLE}\n[targets.options]\nvalidate = true\n[registry]\nurl = \"https://r\"\n[dependencies]\n\"@a/b\" = \"^1\"\n"
        );
        let p = load(&text).unwrap();
        assert_eq!(
            p.targets[1].options.get("validate"),
            Some(&toml::Value::Boolean(true))
        );
        assert_eq!(p.registry.as_deref(), Some("https://r"));
        assert_eq!(p.dependencies["@a/b"], "^1");
    }

    #[test]
    fn missing_project_name_is_rejected() {
        assert_eq!(load("[project]\n").unwrap_err().code(), Some("E0700"));
        assert_eq!(load("").unwrap_err().code(), Some("E0700"));
    }
}
