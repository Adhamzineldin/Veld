//! The one pure resolution function (`spec/workspace.md`, section 7.1): the root file's text
//! and the text of each member's file in, a validated [`Project`] out. No file system access,
//! so the CLI and the playground resolve a workspace the same way.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use toml::Table;
use veld_ir::PackageKind;

use super::error::{ConfigError, Warning};
use super::paths::normalize;
use super::raw::{FileConfig, OneOrMany, PluginSection, Role, TargetSection, WorkspaceSection};
use super::suggest::{closest, hint};
use super::{
    DefaultSpec, EmitterRef, Hooks, MemberKind, MemberSpec, PackageSpec, PluginSpec, Project,
    TargetSpec,
};

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;

/// One parsed file of the workspace.
struct Loaded {
    /// Directory relative to the root, `/` separated; empty for the root directory.
    rel: String,
    /// The file as messages name it: `veld.toml`, `services/bookings/veld.toml`.
    label: String,
    cfg: FileConfig,
}

/// A member after its name and kind are known.
struct Member {
    name: String,
    kind: MemberKind,
    file: Loaded,
}

/// Resolves a workspace. `members` holds `(directory relative to root_dir with /, text of its
/// veld.toml)` for every member below the root, as [`super::expand_members`] selected them.
pub fn resolve(
    root_dir: &Path,
    root_text: &str,
    members: &[(String, String)],
) -> Result<Project, ConfigError> {
    let root_cfg = FileConfig::parse(root_text)?;
    let workspace = root_cfg.workspace.clone();
    let loaded = load_files(&root_cfg, members, workspace.is_some())?;
    let list = name_members(loaded)?;
    let mut warnings = Vec::new();
    let packages = build_packages(root_dir, &list, workspace.as_ref(), &mut warnings)?;
    let plugins = build_plugins(&root_cfg.plugins)?;
    let ctx = TargetCtx {
        root_dir,
        defaults: workspace.as_ref(),
        plugins: &root_cfg.plugins,
        services: services_of(&packages),
    };
    let defaults = build_defaults(workspace.as_ref(), &root_cfg.plugins)?;
    check_apps(&list, &ctx.services)?;
    let targets = build_targets(&list, &ctx)?;
    check_outputs(root_dir, &targets, &packages)?;
    let specs = member_specs(root_dir, &list, &packages, &targets);
    let name = project_name(workspace.as_ref(), &list)?;
    Ok(Project {
        root: root_dir.to_path_buf(),
        file: root_dir.join(super::CONFIG_FILE),
        name,
        description: project_description(workspace.as_ref(), &specs),
        members: specs,
        current: None,
        packages,
        targets,
        defaults,
        plugins,
        registry: root_cfg.registry.and_then(|r| r.url),
        dependencies: root_cfg.dependencies,
        lint: root_cfg.lint,
        hooks: hooks_of(root_cfg.hooks),
        warnings,
    })
}

fn hooks_of(hooks: Option<super::raw::HooksSection>) -> Hooks {
    let Some(h) = hooks else {
        return Hooks::default();
    };
    Hooks {
        post_generate: match h.post_generate {
            None => Vec::new(),
            Some(OneOrMany::One(c)) => vec![c],
            Some(OneOrMany::Many(c)) => c,
        },
        fail: h.fail,
    }
}

fn project_name(ws: Option<&WorkspaceSection>, list: &[Member]) -> Result<String, ConfigError> {
    match ws {
        Some(w) => match w.name.as_deref().map(str::trim) {
            Some(n) if !n.is_empty() => Ok(n.to_string()),
            _ => Err(ConfigError::invalid("`[workspace].name` is required")),
        },
        None => Ok(list.first().map(|m| m.name.clone()).unwrap_or_default()),
    }
}

fn project_description(ws: Option<&WorkspaceSection>, specs: &[MemberSpec]) -> Option<String> {
    match ws {
        Some(w) => w.description.clone(),
        None => specs.first().and_then(|m| m.description.clone()),
    }
}

// ---------------------------------------------------------------------------
// Files and members
// ---------------------------------------------------------------------------

fn load_files(
    root_cfg: &FileConfig,
    members: &[(String, String)],
    is_workspace: bool,
) -> Result<Vec<Loaded>, ConfigError> {
    let root_members = root_cfg.member_tables();
    if !is_workspace && root_members.is_empty() {
        return Err(ConfigError::workspace(
            "`veld.toml` has neither a `[workspace]` table nor a member table (`[service]`, `[app]` or `[library]`)",
        ));
    }
    if !is_workspace && !members.is_empty() {
        return Err(ConfigError::workspace(
            "members were given for a `veld.toml` without a `[workspace]` table",
        ));
    }
    let mut out = Vec::new();
    if !root_members.is_empty() {
        one_member_table("veld.toml", &root_members)?;
        out.push(Loaded {
            rel: String::new(),
            label: "veld.toml".into(),
            cfg: root_cfg.clone(),
        });
    }
    let mut sorted: Vec<&(String, String)> = members.iter().collect();
    sorted.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    for (rel, text) in sorted {
        out.push(load_member_file(rel, text)?);
    }
    Ok(out)
}

fn one_member_table(label: &str, tables: &[&str]) -> Result<(), ConfigError> {
    if tables.len() > 1 {
        return Err(ConfigError::workspace(format!(
            "`{label}` has more than one member table ({}); a file is one service, app or library",
            tables.join(", ")
        )));
    }
    Ok(())
}

fn load_member_file(rel: &str, text: &str) -> Result<Loaded, ConfigError> {
    let label = format!("{rel}/veld.toml");
    let cfg = FileConfig::parse(text)?;
    if cfg.workspace.is_some() {
        return Err(ConfigError::workspace(format!(
            "`{label}` holds a `[workspace]` table, but it is inside the workspace of the root `veld.toml`; a workspace cannot hold a workspace"
        )));
    }
    let tables = cfg.member_tables();
    if tables.is_empty() {
        return Err(ConfigError::workspace(format!(
            "`{label}` has no member table; add `[service]`, `[app]` or `[library]`"
        )));
    }
    one_member_table(&label, &tables)?;
    if let Some(table) = cfg.root_only_tables().first() {
        return Err(ConfigError::not_allowed(format!(
            "`{table}` is not allowed in `{label}`: it belongs in the workspace root `veld.toml`"
        )));
    }
    Ok(Loaded {
        rel: rel.to_string(),
        label,
        cfg,
    })
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn name_members(files: Vec<Loaded>) -> Result<Vec<Member>, ConfigError> {
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    let mut out = Vec::new();
    for file in files {
        let (kind, written) = kind_and_name(&file.cfg);
        let name = match written {
            Some(n) => n,
            None if file.rel.is_empty() => {
                return Err(ConfigError::workspace(format!(
                    "the member in `{}` must set `name`: the name of the root directory is not part of the repository",
                    file.label
                )));
            }
            None => file.rel.rsplit('/').next().unwrap_or_default().to_string(),
        };
        if !valid_name(&name) {
            return Err(ConfigError::workspace(format!(
                "member name `{name}` in `{}` must be non-empty and use only letters, digits, `_` and `-`; set `name` in the member table",
                file.label
            )));
        }
        if let Some(first) = seen.insert(name.clone(), file.label.clone()) {
            return Err(ConfigError::workspace(format!(
                "two members are named `{name}`: `{first}` and `{}`",
                file.label
            )));
        }
        out.push(Member { name, kind, file });
    }
    Ok(out)
}

fn kind_and_name(cfg: &FileConfig) -> (MemberKind, Option<String>) {
    if let Some(s) = &cfg.service {
        (MemberKind::Service, s.name.clone())
    } else if let Some(a) = &cfg.app {
        (MemberKind::App, a.name.clone())
    } else {
        (
            MemberKind::Library,
            cfg.library.as_ref().and_then(|l| l.name.clone()),
        )
    }
}

fn member_dir(root: &Path, rel: &str) -> PathBuf {
    if rel.is_empty() {
        root.to_path_buf()
    } else {
        normalize(&root.join(rel))
    }
}

// ---------------------------------------------------------------------------
// Packages
// ---------------------------------------------------------------------------

fn build_packages(
    root: &Path,
    list: &[Member],
    ws: Option<&WorkspaceSection>,
    warnings: &mut Vec<Warning>,
) -> Result<Vec<PackageSpec>, ConfigError> {
    let mut libraries = Vec::new();
    let mut services = Vec::new();
    let service_names: Vec<&str> = {
        let mut v: Vec<&str> = list
            .iter()
            .filter(|m| m.kind == MemberKind::Service)
            .map(|m| m.name.as_str())
            .collect();
        v.sort_unstable();
        v
    };
    for m in list {
        let dir = member_dir(root, &m.file.rel);
        match m.kind {
            MemberKind::Library => libraries.push(library_package(m, &dir)),
            MemberKind::Service => {
                services.push(service_package(m, &dir, ws, &service_names, warnings)?);
            }
            MemberKind::App => {}
        }
    }
    libraries.sort_by(|a: &PackageSpec, b| a.name.cmp(&b.name));
    services.sort_by(|a: &PackageSpec, b| a.name.cmp(&b.name));
    libraries.extend(services);
    Ok(libraries)
}

fn contracts_dir(dir: &Path, contracts: Option<&str>) -> PathBuf {
    normalize(&dir.join(contracts.unwrap_or("contracts")))
}

fn library_package(m: &Member, dir: &Path) -> PackageSpec {
    let contracts = m
        .file
        .cfg
        .library
        .as_ref()
        .and_then(|l| l.contracts.as_deref());
    PackageSpec {
        name: m.name.clone(),
        kind: PackageKind::Library,
        root: contracts_dir(dir, contracts),
        base_url: None,
        consumes: Vec::new(),
        prefix: String::new(),
    }
}

fn service_package(
    m: &Member,
    dir: &Path,
    ws: Option<&WorkspaceSection>,
    services: &[&str],
    warnings: &mut Vec<Warning>,
) -> Result<PackageSpec, ConfigError> {
    let svc = m.file.cfg.service.clone().unwrap_or_default();
    Ok(PackageSpec {
        name: m.name.clone(),
        kind: PackageKind::Service,
        root: contracts_dir(dir, svc.contracts.as_deref()),
        base_url: svc.base_url.clone(),
        consumes: resolve_consumes(&m.name, &svc.consumes, services, warnings)?,
        prefix: join_prefix(ws.and_then(|w| w.prefix.as_deref()), svc.prefix.as_deref())?,
    })
}

/// Joins the workspace and service route prefixes (`/api` + `/v1` = `/api/v1`). Each must
/// start with `/`; trailing slashes are dropped. `/` alone means none.
fn join_prefix(workspace: Option<&str>, service: Option<&str>) -> Result<String, ConfigError> {
    let mut out = String::new();
    for (key, value) in [
        ("[workspace].prefix", workspace),
        ("[service].prefix", service),
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
    services: &[&str],
    warnings: &mut Vec<Warning>,
) -> Result<Vec<String>, ConfigError> {
    let mut resolved: Vec<String> = Vec::new();
    for name in consumes {
        if name == "*" {
            for other in services.iter().filter(|s| **s != owner) {
                if !resolved.iter().any(|r| r == other) {
                    resolved.push((*other).to_string());
                }
            }
        } else if name == owner {
            warnings.push(Warning {
                code: "W0701",
                message: format!("service `{owner}` lists itself in `consumes`; ignoring"),
            });
        } else if !services.contains(&name.as_str()) {
            return Err(unknown_service(
                &format!("service `{owner}`"),
                name,
                services.iter().copied(),
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

fn services_of(packages: &[PackageSpec]) -> Vec<String> {
    packages
        .iter()
        .filter(|p| p.kind == PackageKind::Service)
        .map(|p| p.name.clone())
        .collect()
}

fn build_plugins(
    plugins: &BTreeMap<String, PluginSection>,
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

// ---------------------------------------------------------------------------
// Targets
// ---------------------------------------------------------------------------

struct TargetCtx<'a> {
    root_dir: &'a Path,
    defaults: Option<&'a WorkspaceSection>,
    plugins: &'a BTreeMap<String, PluginSection>,
    services: Vec<String>,
}

/// The targets of every member, ordered by member, then servers before clients, then by their
/// order in the file. Names follow section 6.2.
fn build_targets(list: &[Member], ctx: &TargetCtx<'_>) -> Result<Vec<TargetSpec>, ConfigError> {
    let mut targets = Vec::new();
    for m in list {
        targets.extend(member_targets(m, ctx)?);
    }
    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
    for t in &targets {
        let owner = list
            .iter()
            .find(|m| m.name == t.member)
            .map_or("", |m| m.file.label.as_str());
        if let Some(first) = seen.insert(t.name.as_str(), owner) {
            return Err(ConfigError::workspace(format!(
                "two targets are named `{}`: in `{first}` and in `{owner}`",
                t.name
            )));
        }
    }
    Ok(targets)
}

fn member_targets(m: &Member, ctx: &TargetCtx<'_>) -> Result<Vec<TargetSpec>, ConfigError> {
    let cfg = &m.file.cfg;
    let servers = cfg.server.as_ref().map(|t| t.list()).unwrap_or_default();
    let clients = cfg.client.as_ref().map(|t| t.list()).unwrap_or_default();
    if m.kind == MemberKind::Library && !(servers.is_empty() && clients.is_empty()) {
        return Err(ConfigError::not_allowed(format!(
            "a library has no targets: remove `[server]` and `[client]` from `{}`",
            m.file.label
        )));
    }
    if m.kind == MemberKind::App && !servers.is_empty() {
        return Err(ConfigError::not_allowed(format!(
            "an app has no server: `[server]` is not allowed in `{}`; an app only consumes services, so write `[client]` there or move the server into a `[service]` member",
            m.file.label
        )));
    }
    let both = !servers.is_empty() && !clients.is_empty();
    let mut out = Vec::new();
    for (role, items) in [(Role::Server, servers), (Role::Client, clients)] {
        for t in &items {
            let name = target_name(m, role, t, items.len(), both)?;
            out.push(build_target(m, role, t, name, ctx)?);
        }
    }
    Ok(out)
}

/// Section 6.2: the member's name for its only target, `<member>-server` or `<member>-client`
/// when it has targets of both roles, and an explicit `name` when a role has several.
fn target_name(
    m: &Member,
    role: Role,
    t: &TargetSection,
    of_role: usize,
    both: bool,
) -> Result<String, ConfigError> {
    if let Some(name) = &t.name {
        if name.is_empty() {
            return Err(ConfigError::workspace(format!(
                "a target in `{}` has an empty `name`",
                m.file.label
            )));
        }
        return Ok(name.clone());
    }
    if of_role > 1 {
        return Err(ConfigError::workspace(format!(
            "`{}` has {of_role} `[[{}]]` targets: each one needs a `name`",
            m.file.label,
            role.name()
        )));
    }
    Ok(if both {
        format!("{}-{}", m.name, role.name())
    } else {
        m.name.clone()
    })
}

fn build_target(
    m: &Member,
    role: Role,
    t: &TargetSection,
    name: String,
    ctx: &TargetCtx<'_>,
) -> Result<TargetSpec, ConfigError> {
    let at = format!("`[{}]` in `{}`", role.name(), m.file.label);
    let id = t
        .emitter
        .as_deref()
        .ok_or_else(|| ConfigError::invalid(format!("{at} is missing `emitter`")))?;
    let emitter = parse_emitter(ctx.plugins, id, &at)?;
    let (framework, options) = with_defaults(ctx, role, id, t);
    let dir = member_dir(ctx.root_dir, &m.file.rel);
    let out = match (&t.out, &emitter) {
        (Some(out), _) => out.clone(),
        (None, EmitterRef::Builtin(e)) => default_out(e).to_string(),
        (None, EmitterRef::Plugin(_)) => {
            return Err(ConfigError::invalid(format!(
                "{at} is missing `out` (a plugin has no default)"
            )));
        }
    };
    let (service, services) = served(m, role, ctx)?;
    Ok(TargetSpec {
        name,
        member: m.name.clone(),
        role,
        emitter,
        framework,
        service,
        services,
        out: normalize(&dir.join(out)),
        clients: t.clients.unwrap_or(true),
        app: t.app.as_deref().map(|a| normalize(&dir.join(a))),
        options,
    })
}

/// The `[workspace.defaults.<role>.<emitter>]` tables, ordered by role then emitter.
fn build_defaults(
    ws: Option<&WorkspaceSection>,
    plugins: &BTreeMap<String, PluginSection>,
) -> Result<Vec<DefaultSpec>, ConfigError> {
    let Some(ws) = ws else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for (role_name, by_emitter) in &ws.defaults {
        let role = if role_name == "server" {
            Role::Server
        } else {
            Role::Client
        };
        for (id, table) in by_emitter {
            let at = format!("`[workspace.defaults.{role_name}.{id}]`");
            let emitter = parse_emitter(plugins, id, &at)?;
            let mut options = table.clone();
            let framework = match options.remove("framework") {
                None => None,
                Some(toml::Value::String(f)) => Some(f),
                Some(_) => {
                    return Err(ConfigError::invalid(format!(
                        "{at}: `framework` must be a string"
                    )));
                }
            };
            out.push(DefaultSpec {
                role,
                emitter,
                framework,
                options,
            });
        }
    }
    Ok(out)
}

/// Section 6.1.
fn default_out(emitter: &str) -> &'static str {
    match emitter {
        "java" => "src/main/java",
        "rust" => "generated",
        _ => "src/generated",
    }
}

/// The framework and options of a target after the workspace defaults (section 4.2): the
/// member's `framework` replaces the default one (and with it a default `framework-version`),
/// and options merge key by key with the member's winning.
fn with_defaults(
    ctx: &TargetCtx<'_>,
    role: Role,
    emitter: &str,
    t: &TargetSection,
) -> (Option<String>, Table) {
    let default = ctx
        .defaults
        .and_then(|w| w.defaults.get(role.name()))
        .and_then(|by_emitter| by_emitter.get(emitter));
    let Some(default) = default else {
        return (t.framework.clone(), t.options.clone());
    };
    let mut options = default.clone();
    let default_framework = options
        .remove("framework")
        .and_then(|v| v.as_str().map(str::to_string));
    if t.framework.is_some() {
        options.remove("framework-version");
    }
    for (k, v) in &t.options {
        options.insert(k.clone(), v.clone());
    }
    (t.framework.clone().or(default_framework), options)
}

fn parse_emitter(
    plugins: &BTreeMap<String, PluginSection>,
    id: &str,
    at: &str,
) -> Result<EmitterRef, ConfigError> {
    match id.strip_prefix("plugin:") {
        Some("") => Err(ConfigError::invalid(format!(
            "{at}: `plugin:` needs a plugin name"
        ))),
        Some(name) if plugins.contains_key(name) => Ok(EmitterRef::Plugin(name.to_string())),
        Some(name) => Err(ConfigError::invalid(format!(
            "{at}: plugin `{name}` is not declared under `[plugins.{name}]`"
        ))),
        None if id.is_empty() => Err(ConfigError::invalid(format!("{at}: `emitter` is empty"))),
        None => Ok(EmitterRef::Builtin(id.to_string())),
    }
}

/// A server serves its own service; a client of a service covers it, a client of an app
/// covers the services in `uses` (default: all).
fn served(
    m: &Member,
    role: Role,
    ctx: &TargetCtx<'_>,
) -> Result<(Option<String>, Vec<String>), ConfigError> {
    match (role, m.kind) {
        (Role::Server, _) => Ok((Some(m.name.clone()), Vec::new())),
        (Role::Client, MemberKind::App) => {
            let list = uses_of(m, &ctx.services)?;
            Ok((None, list))
        }
        (Role::Client, _) => Ok((None, vec![m.name.clone()])),
    }
}

/// Every app's `uses` names services of the workspace, whether or not the app has targets.
fn check_apps(list: &[Member], services: &[String]) -> Result<(), ConfigError> {
    for m in list.iter().filter(|m| m.kind == MemberKind::App) {
        uses_of(m, services)?;
    }
    Ok(())
}

/// `uses` of an app: the named services (checked), or every service by default.
fn uses_of(m: &Member, services: &[String]) -> Result<Vec<String>, ConfigError> {
    let app = m.file.cfg.app.clone().unwrap_or_default();
    let Some(list) = app.uses else {
        return Ok(services.to_vec());
    };
    let owner = format!("app `{}`", m.name);
    let mut out: Vec<String> = Vec::new();
    for name in list {
        if !services.contains(&name) {
            return Err(unknown_service(
                &owner,
                &name,
                services.iter().map(String::as_str),
            ));
        }
        if !out.contains(&name) {
            out.push(name);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Members
// ---------------------------------------------------------------------------

fn member_specs(
    root: &Path,
    list: &[Member],
    packages: &[PackageSpec],
    targets: &[TargetSpec],
) -> Vec<MemberSpec> {
    let services: Vec<String> = services_of(packages);
    list.iter()
        .map(|m| {
            let dir = member_dir(root, &m.file.rel);
            let (owners, package, version, description) = publish_meta(&m.file.cfg);
            MemberSpec {
                name: m.name.clone(),
                kind: m.kind,
                file: dir.join(super::CONFIG_FILE),
                contracts: packages
                    .iter()
                    .find(|p| p.name == m.name && m.kind != MemberKind::App)
                    .map(|p| p.root.clone()),
                owners,
                depends_on: depends_on(m, packages, &services),
                package,
                version,
                description,
                targets: targets
                    .iter()
                    .filter(|t| t.member == m.name)
                    .map(|t| t.name.clone())
                    .collect(),
                dir,
            }
        })
        .collect()
}

fn depends_on(m: &Member, packages: &[PackageSpec], services: &[String]) -> Vec<String> {
    match m.kind {
        MemberKind::Service => packages
            .iter()
            .find(|p| p.name == m.name)
            .map(|p| p.consumes.clone())
            .unwrap_or_default(),
        MemberKind::App => uses_of(m, services).unwrap_or_default(),
        MemberKind::Library => Vec::new(),
    }
}

type PublishMeta = (Vec<String>, Option<String>, Option<String>, Option<String>);

fn publish_meta(cfg: &FileConfig) -> PublishMeta {
    if let Some(s) = &cfg.service {
        return (
            s.owners.clone(),
            s.package.clone(),
            s.version.clone(),
            s.description.clone(),
        );
    }
    if let Some(l) = &cfg.library {
        return (
            l.owners.clone(),
            l.package.clone(),
            l.version.clone(),
            l.description.clone(),
        );
    }
    let owners = cfg
        .app
        .as_ref()
        .map(|a| a.owners.clone())
        .unwrap_or_default();
    (owners, None, None, None)
}

// ---------------------------------------------------------------------------
// Output directories
// ---------------------------------------------------------------------------

pub(super) fn fold(p: &Path) -> PathBuf {
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
                let shared = if a.out.starts_with(&b.out) {
                    &b.out
                } else {
                    &a.out
                };
                return Err(ConfigError::OutputOverlap {
                    first: a.name.clone(),
                    second: b.name.clone(),
                    reason,
                    dir: relative_to(root, shared),
                });
            }
        }
    }
    Ok(())
}

/// `path` below `root` with `/` separators, or the path itself when it lies elsewhere.
fn relative_to(root: &Path, path: &Path) -> String {
    let (a, b): (Vec<_>, Vec<_>) = (root.components().collect(), path.components().collect());
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    if common == 0 {
        return path.to_string_lossy().replace('\\', "/");
    }
    let mut parts: Vec<String> = vec!["..".into(); a.len() - common];
    parts.extend(
        b[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
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
        return Err(forbid("is the workspace root".into()));
    }
    match packages.iter().find(|p| nested(&t.out, &p.root)) {
        Some(p) => Err(forbid(format!(
            "overlaps the contracts directory of `{}` (inside it, or containing it)",
            p.name
        ))),
        None => Ok(()),
    }
}
