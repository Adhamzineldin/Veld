//! `PluginGenerator`: a plugin behind the same `DynGenerator` interface as the built-ins.

use std::path::{Path, PathBuf};
use std::time::Duration;

use veld_codegen::{Capabilities, DynGenerator, FileSet, GenDiagnostic, GenRequest, Role};
use veld_project::config::PluginSpec;

use crate::command::CommandLine;
use crate::error::PluginError;
use crate::protocol::{self, Handshake, Outcome};
use crate::session::{Session, describe};

/// How the host talks to plugins. The defaults are the protocol's; tests shorten them.
#[derive(Debug, Clone)]
pub struct HostConfig {
    /// Relative plugin commands are resolved against this directory (the project root).
    pub project_root: PathBuf,
    pub veld_version: String,
    pub handshake_timeout: Duration,
    pub exit_grace: Duration,
}

impl HostConfig {
    #[must_use]
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            veld_version: env!("CARGO_PKG_VERSION").to_string(),
            handshake_timeout: protocol::HANDSHAKE_TIMEOUT,
            exit_grace: protocol::EXIT_GRACE,
        }
    }
}

/// Spawns plugin processes for one command. Knows nothing about `veld.toml` pins.
#[derive(Debug, Clone)]
pub struct Runner {
    pub name: String,
    pub command: CommandLine,
    pub config: HostConfig,
}

impl Runner {
    fn resolve(&self) -> Result<PathBuf, PluginError> {
        self.command
            .resolve(&self.config.project_root)
            .map_err(|m| PluginError::process(&self.name, m))
    }

    /// Starts the plugin and completes the handshake. The session is killed on any error.
    pub fn start(&self, exe: &Path) -> Result<(Session, Handshake), PluginError> {
        let anchored = self.command.anchored(&self.config.project_root);
        let mut session = Session::spawn(&self.name, exe, &anchored)?;
        session.send(&protocol::hello(&self.config.veld_version))?;
        let reply = session.recv(self.config.handshake_timeout, "its `hello`")?;
        match protocol::parse_handshake(&self.name, &reply) {
            Ok(h) => Ok((session, h)),
            Err(e) => {
                session.kill();
                Err(e)
            }
        }
    }

    /// Handshake only: used to learn the plugin's roles and capabilities.
    pub fn handshake(&self, exe: &Path) -> Result<Handshake, PluginError> {
        let (mut session, hello) = self.start(exe)?;
        session.finish(self.config.exit_grace);
        Ok(hello)
    }

    /// One complete run: handshake, `generate`, `result`, and a clean exit.
    pub fn generate(
        &self,
        exe: &Path,
        req: &GenRequest<'_>,
        options: &serde_json::Value,
    ) -> Result<Outcome, PluginError> {
        let (timeout, options) = protocol::split_options(&self.name, options)?;
        let (mut session, hello) = self.start(exe)?;
        if !hello.roles.contains(&req.role) {
            session.kill();
            return Err(PluginError::protocol(
                &self.name,
                format!("does not support the `{}` role", req.role.name()),
            ));
        }
        session
            .send(&protocol::generate_message(req, &options)?)
            .inspect_err(|_| session.kill())?;
        let reply = session.recv(timeout, "a `result`")?;
        let outcome = protocol::parse_result(&self.name, &reply).inspect_err(|_| session.kill())?;
        match session.finish(self.config.exit_grace) {
            Some(status) if !status.success() => Err(PluginError::process(
                &self.name,
                format!("exited with {} after sending its result", describe(&status)),
            )),
            _ => Ok(outcome),
        }
    }
}

/// A plugin declared in `veld.toml`, ready to be used as an emitter.
#[derive(Debug)]
pub struct PluginGenerator {
    spec: PluginSpec,
    runner: Runner,
    id: &'static str,
    roles: &'static [Role],
    capabilities: Capabilities,
}

impl PluginGenerator {
    /// Verifies the pinned hash (E0705), starts the plugin once and reads its handshake.
    pub fn connect(spec: &PluginSpec, config: HostConfig) -> Result<Self, PluginError> {
        let command = CommandLine::parse(&spec.command)
            .ok_or_else(|| PluginError::process(&spec.name, "its `command` is empty"))?;
        let runner = Runner {
            name: spec.name.clone(),
            command,
            config,
        };
        let exe = verified_executable(spec, &runner)?;
        let hello = runner.handshake(&exe)?;
        if let Some(pinned) = &spec.version
            && pinned != &hello.version
        {
            return Err(PluginError::protocol(
                &spec.name,
                format!(
                    "veld.toml pins version {pinned}, but the plugin reports {}",
                    hello.version
                ),
            ));
        }
        Ok(Self {
            spec: spec.clone(),
            runner,
            id: Box::leak(spec.name.clone().into_boxed_str()),
            roles: Box::leak(hello.roles.into_boxed_slice()),
            capabilities: hello.capabilities,
        })
    }
}

/// Resolves the executable and checks it against the pinned hash.
fn verified_executable(spec: &PluginSpec, runner: &Runner) -> Result<PathBuf, PluginError> {
    let exe = runner.resolve()?;
    spec.verify_executable(&exe).map_err(|e| match e {
        veld_project::config::ConfigError::PluginHashMismatch {
            plugin,
            expected,
            actual,
        } => PluginError::HashMismatch {
            plugin,
            expected,
            actual,
        },
        other => PluginError::process(&spec.name, other.to_string()),
    })?;
    Ok(exe)
}

impl DynGenerator for PluginGenerator {
    fn id(&self) -> &'static str {
        self.id
    }

    fn roles(&self) -> &'static [Role] {
        self.roles
    }

    fn frameworks(&self, _role: Role) -> &'static [&'static str] {
        &[]
    }

    fn capabilities(&self, _role: Role) -> Capabilities {
        self.capabilities.clone()
    }

    fn generate_dyn(
        &self,
        req: &GenRequest<'_>,
        opts: &serde_json::Value,
    ) -> Result<FileSet, Vec<GenDiagnostic>> {
        // The hash is checked again before every run, not only when the plugin was first loaded.
        let exe = verified_executable(&self.spec, &self.runner).map_err(|e| vec![e.into()])?;
        let outcome = self
            .runner
            .generate(&exe, req, opts)
            .map_err(|e| vec![e.into()])?;
        let (warnings, errors): (Vec<_>, Vec<_>) = outcome
            .diagnostics
            .into_iter()
            .partition(protocol::is_warning);
        for w in &warnings {
            eprintln!("[plugin:{}] warning: {}", self.id, w.message);
        }
        if errors.is_empty() {
            Ok(outcome.files)
        } else {
            Err(errors)
        }
    }
}
