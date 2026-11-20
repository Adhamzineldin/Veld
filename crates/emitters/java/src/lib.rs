//! The `java` generator: records, sealed unions, typed errors, a framework-agnostic server core
//! and a `java.net.http` client, on top of one embedded zero-dependency runtime.
//!
//! See `spec/targets/java.md`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod client;
pub mod frameworks;
pub mod models;
pub mod runtime;
pub mod server;
pub mod spec;
pub mod tree;
pub mod types;
pub mod util;

use serde::Deserialize;
use veld_codegen::naming;
use veld_codegen::{
    Capabilities, FileSet, FrameworkInfo, GenDiagnostic, GenRequest, Generator, LanguageInfo,
    Profile, Role, check_capabilities,
};
use veld_ir::Feature;

use crate::tree::Tree;

/// Features the Java generator does not support yet.
pub const UNSUPPORTED: [Feature; 2] = [Feature::StreamingSse, Feature::StreamingWs];

/// Supported `java` option values: the Java release the output is compiled for, oldest first
/// (the default). Selected with the target option `java = "21"`.
pub const JAVA_VERSIONS: [&str; 2] = ["17", "21"];

/// Options of the `java` generator.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    /// The base Java package. Default: `dev.veld.generated.<package of the first service>`.
    pub package: Option<String>,
}

/// The Java generator (roles `server` and `client`).
#[derive(Debug, Default, Clone, Copy)]
pub struct Java;

fn precheck(req: &GenRequest<'_>, caps: &Capabilities) -> Result<(), Vec<GenDiagnostic>> {
    if req.services.is_empty() {
        return Err(vec![GenDiagnostic::new(
            "E0803",
            "no services selected for generation",
        )]);
    }
    if let Some(bad) = req
        .services
        .iter()
        .chain(&req.consumes)
        .find(|s| s.index() >= req.ir.services.len())
    {
        return Err(vec![GenDiagnostic::new(
            "E0803",
            format!("unknown service id {}", bad.0),
        )]);
    }
    let diags = check_capabilities(req.ir, req, caps);
    if diags.is_empty() { Ok(()) } else { Err(diags) }
}

/// The base package: the option, or `dev.veld.generated.<package of the first service>`.
fn base_package(req: &GenRequest<'_>, opts: &Options) -> Result<String, Vec<GenDiagnostic>> {
    if let Some(p) = &opts.package {
        return if util::is_package(p) {
            Ok(p.clone())
        } else {
            Err(vec![GenDiagnostic::new(
                "E0802",
                format!(
                    "invalid options for generator `java`: `{p}` is not a valid Java package name"
                ),
            )])
        };
    }
    let home = req.ir.package(req.ir.service(req.services[0]).package);
    let name = home.name.trim_start_matches('@').replace('/', "_");
    Ok(format!("dev.veld.generated.{}", util::segment(&name)))
}

impl Generator for Java {
    type Options = Options;

    fn id(&self) -> &'static str {
        "java"
    }

    fn roles(&self) -> &'static [Role] {
        &[Role::Server, Role::Client]
    }

    fn frameworks(&self, role: Role) -> Vec<FrameworkInfo> {
        match role {
            Role::Server => frameworks::infos(),
            Role::Client => Vec::new(),
        }
    }

    fn language(&self) -> Option<LanguageInfo> {
        Some(LanguageInfo {
            id: "java".to_string(),
            versions: JAVA_VERSIONS.iter().map(|v| (*v).to_string()).collect(),
        })
    }

    fn capabilities(&self, _role: Role) -> Capabilities {
        Capabilities::all_except(&UNSUPPORTED)
    }

    fn generate(
        &self,
        req: &GenRequest<'_>,
        opts: &Options,
        profile: &Profile,
    ) -> Result<FileSet, Vec<GenDiagnostic>> {
        precheck(req, &self.capabilities(req.role))?;
        let base = base_package(req, opts)?;
        let runtime_package = runtime::package(&base);
        let mut files = FileSet::new();
        runtime::emit(&mut files, &base)?;
        let tree = Tree::new(req.ir, &base, &runtime_package, req.services.clone());
        models::emit(&mut files, &tree)?;
        match req.role {
            Role::Client => client::emit(&mut files, &tree)?,
            Role::Server => {
                let model = frameworks::ServerModel { base: base.clone() };
                frameworks::emit(&mut files, &model, profile)?;
                server::emit(&mut files, &tree)?;
                emit_consumed(&mut files, req, &base, &runtime_package)?;
            }
        }
        Ok(files)
    }
}

/// Emits a client tree under `<base>.clients.<service>` for each consumed service.
fn emit_consumed(
    files: &mut FileSet,
    req: &GenRequest<'_>,
    base: &str,
    runtime_package: &str,
) -> Result<(), Vec<GenDiagnostic>> {
    for sid in &req.consumes {
        let svc = req.ir.service(*sid);
        let mut dir = util::segment(&naming::snake(&svc.name));
        if dir.is_empty() {
            dir.push('_');
        }
        let tree = Tree::new(
            req.ir,
            &format!("{base}.clients.{dir}"),
            runtime_package,
            vec![*sid],
        );
        models::emit(files, &tree)?;
        client::emit(files, &tree)?;
    }
    Ok(())
}
