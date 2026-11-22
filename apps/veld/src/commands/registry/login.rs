//! `veld login`: store an API token after checking it with the registry.

use std::io::{BufRead, IsTerminal};
use std::path::Path;

use anyhow::Result;
use veld_registry_client::{Token, resolve_registry};

use super::{Session, fail, project_registry};
use crate::cli::LoginArgs;
use crate::error::{Exit, user};
use crate::pipeline;
use crate::ui::Ui;

pub fn run(ui: &Ui, config: Option<&Path>, args: &LoginArgs) -> Result<Exit> {
    ui.header("login", "");
    let mut session = Session::load(args.allow_insecure)?;
    let project = pipeline::load_project(config).ok();
    let registry = resolve_registry(
        args.registry.as_deref(),
        project_registry(project.as_ref()),
        &session.creds,
    )
    .map_err(fail)?;
    let token = match &args.token {
        Some(t) => t.trim().to_string(),
        None => read_token(&registry)?,
    };
    if token.is_empty() {
        return Err(user("the token is empty"));
    }
    let client = veld_registry_client::Client::new(
        &registry,
        Some(Token::new(token.clone())),
        session.allow_insecure,
    )
    .map_err(fail)?;
    let username = client.me().map_err(|e| {
        if matches!(
            e,
            veld_registry_client::RegistryError::Http { status: 401, .. }
        ) {
            user(format!("{registry} rejected the token"))
        } else {
            fail(e)
        }
    })?;
    let registry = client.base_url().to_string();
    session.creds.set(&registry, Token::new(token));
    session.creds.save(&session.path).map_err(fail)?;
    ui.ok(&format!("logged in to {registry} as {username}"));
    Ok(Exit::Ok)
}

/// Reads one line from stdin. The token is never echoed back.
fn read_token(registry: &str) -> Result<String> {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        eprintln!("Paste an API token for {registry} (create one with POST /api/v1/tokens):");
    }
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .map_err(|e| user(format!("cannot read the token from stdin: {e}")))?;
    Ok(line.trim().to_string())
}
