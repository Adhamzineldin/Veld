//! The registry commands: `login`, `logout`, `add`, `publish` and `install`, plus the
//! automatic install that `check` and `generate` run when `veld.lock` pins missing packages.

mod add;
mod install;
mod login;
mod logout;
mod publish;

use std::path::PathBuf;

use anyhow::Result;
use veld_project::config::Project;
use veld_project::packages::LockedPackage;
use veld_registry_client::credentials::credentials_path;
use veld_registry_client::{Client, Credentials, RegistryError, Source};

use crate::cli::Command;
use crate::error::{Exit, user};
use crate::ui::Ui;

pub use install::auto_install;

/// Runs a registry command; `None` when `command` is not one of them.
pub fn run(ui: &Ui, config: Option<&std::path::Path>, command: &Command) -> Option<Result<Exit>> {
    Some(match command {
        Command::Login(a) => login::run(ui, config, a),
        Command::Logout(a) => logout::run(ui, config, a),
        Command::Add(a) => add::run(ui, config, a),
        Command::Publish(a) => publish::run(ui, config, a),
        Command::Install(a) => install::run(ui, config, a),
        _ => return None,
    })
}

/// Registry errors are the user's to fix (bad input, network, tampering): exit code 1.
pub(super) fn fail(e: RegistryError) -> anyhow::Error {
    user(e.to_string())
}

/// Shared state of one command: the credentials file and the connection policy.
pub(super) struct Session {
    pub path: PathBuf,
    pub creds: Credentials,
    pub allow_insecure: bool,
}

impl Session {
    pub fn load(allow_insecure: bool) -> Result<Self> {
        let path = credentials_path().map_err(fail)?;
        let creds = Credentials::load(&path).map_err(fail)?;
        let allow_insecure = allow_insecure || std::env::var_os("VELD_ALLOW_INSECURE").is_some();
        Ok(Self {
            path,
            creds,
            allow_insecure,
        })
    }

    /// A client for `registry`, authenticated when a token is stored for it.
    pub fn client(&self, registry: &str) -> Result<Client> {
        Client::new(
            registry,
            self.creds.token_for(registry).cloned(),
            self.allow_insecure,
        )
        .map_err(fail)
    }

    /// A client that must be authenticated.
    pub fn authed_client(&self, registry: &str) -> Result<Client> {
        match self.creds.token_for(registry) {
            Some(t) => Client::new(registry, Some(t.clone()), self.allow_insecure).map_err(fail),
            None => Err(fail(RegistryError::NotLoggedIn(registry.to_string()))),
        }
    }

    /// Downloads the archive a lock entry pins, from the registry the entry names.
    pub fn download(&self, p: &LockedPackage) -> veld_registry_client::Result<Vec<u8>> {
        let (org, name) = veld_project::packages::split_name(&p.name)
            .ok_or_else(|| RegistryError::Resolve(format!("bad package name `{}`", p.name)))?;
        let client = Client::new(
            &p.registry,
            self.creds.token_for(&p.registry).cloned(),
            self.allow_insecure,
        )?;
        client.download(org, name, &p.version)
    }
}

/// The project's `[registry] url`, if a project can be loaded here.
pub(super) fn project_registry(project: Option<&Project>) -> Option<&str> {
    project.and_then(|p| p.registry.as_deref())
}
