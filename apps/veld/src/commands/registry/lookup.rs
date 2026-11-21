//! `veld registry info|search|versions|token create`.

use std::path::Path;

use anyhow::Result;
use serde_json::Value;
use veld_registry_client::{Client, RegistryError, Source, resolve_registry};

use super::{Session, fail, project_registry};
use crate::cli::{RegistryCommand, RegistryOpts, TokenCommand};
use crate::error::{Exit, user};
use crate::pipeline;
use crate::ui::Ui;

pub fn run(ui: &Ui, config: Option<&Path>, cmd: &RegistryCommand) -> Result<Exit> {
    ui.header("registry", "");
    match cmd {
        RegistryCommand::Info(opts) => info(ui, config, opts),
        RegistryCommand::Search { query, opts } => search(ui, config, query.as_deref(), opts),
        RegistryCommand::Versions { package, opts } => versions(ui, config, package, opts),
        RegistryCommand::Token(TokenCommand::Create {
            name,
            scopes,
            expires_days,
            opts,
        }) => create_token(ui, config, name, scopes, *expires_days, opts),
    }
}

/// The session, the registry URL in use and a client (authenticated if a token is stored).
fn connect(config: Option<&Path>, opts: &RegistryOpts) -> Result<(Session, String, Client)> {
    let session = Session::load(opts.allow_insecure)?;
    let project = pipeline::load_project(config).ok();
    let registry = resolve_registry(
        opts.registry.as_deref(),
        project_registry(project.as_ref()),
        &session.creds,
    )
    .map_err(fail)?;
    let client = session.client(&registry)?;
    Ok((session, registry, client))
}

fn info(ui: &Ui, config: Option<&Path>, opts: &RegistryOpts) -> Result<Exit> {
    let (session, registry, client) = connect(config, opts)?;
    ui.info(&format!("registry  {}", client.base_url()));
    if session.creds.token_for(&registry).is_none() {
        ui.info("account   not logged in (run `veld login`)");
        return Ok(Exit::Ok);
    }
    match client.me() {
        Ok(name) => ui.info(&format!("account   {name}")),
        Err(RegistryError::Http { status: 401, .. }) => {
            ui.info("account   the stored token was rejected (run `veld login`)");
        }
        Err(e) => return Err(fail(e)),
    }
    Ok(Exit::Ok)
}

fn text<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn search(
    ui: &Ui,
    config: Option<&Path>,
    query: Option<&str>,
    opts: &RegistryOpts,
) -> Result<Exit> {
    let (_, _, client) = connect(config, opts)?;
    let path = match query {
        Some(q) => format!("/api/v1/packages?q={}", encode(q)),
        None => "/api/v1/packages".to_string(),
    };
    let found = client.get_json(&path).map_err(fail)?;
    let items = found.as_array().map(Vec::as_slice).unwrap_or_default();
    if items.is_empty() {
        ui.info("no packages found");
        return Ok(Exit::Ok);
    }
    for p in items {
        let name = format!("@{}/{}", text(p, "org_name"), text(p, "name"));
        ui.data(&format!(
            "{name:<30} {:<10} {}",
            text(p, "latest_version"),
            text(p, "description")
        ));
    }
    Ok(Exit::Ok)
}

/// Percent-encodes a query value.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn versions(ui: &Ui, config: Option<&Path>, package: &str, opts: &RegistryOpts) -> Result<Exit> {
    let Some((org, name)) = veld_project::packages::split_name(package) else {
        return Err(user(format!(
            "`{package}` is not a package name; use @org/name"
        )));
    };
    let (_, _, client) = connect(config, opts)?;
    let list = client.versions(org, name).map_err(fail)?;
    if list.is_empty() {
        ui.info(&format!("{package} has no published versions"));
        return Ok(Exit::Ok);
    }
    for v in list {
        let note = v
            .deprecated
            .as_deref()
            .map(|d| format!("  (deprecated: {d})"))
            .unwrap_or_default();
        ui.data(&format!("{}{note}", v.version));
    }
    Ok(Exit::Ok)
}

fn create_token(
    ui: &Ui,
    config: Option<&Path>,
    name: &str,
    scopes: &[String],
    expires_days: Option<u32>,
    opts: &RegistryOpts,
) -> Result<Exit> {
    for s in scopes {
        if !matches!(s.as_str(), "read" | "write" | "delete") {
            return Err(user(format!(
                "unknown scope `{s}` (use read, write or delete)"
            )));
        }
    }
    let (session, registry, _) = connect(config, opts)?;
    let client = session.authed_client(&registry)?;
    let body = serde_json::json!({
        "name": name,
        "scopes": scopes,
        "expires_days": expires_days.unwrap_or(0),
    });
    let made = client.post_json("/api/v1/tokens", &body).map_err(fail)?;
    let token = text(&made, "token");
    if token.is_empty() {
        return Err(user("the registry did not return a token"));
    }
    ui.ok(&format!(
        "created token `{name}` ({}); it is shown only once:",
        scopes.join(", ")
    ));
    ui.data(token);
    Ok(Exit::Ok)
}
