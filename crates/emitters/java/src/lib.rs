//! The `java` generator: records, sealed unions, typed errors, a framework-agnostic server core
//! and a `java.net.http` client, on top of one embedded zero-dependency runtime.
//!
//! See `spec/targets/java.md`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod client;
pub mod frameworks;
pub mod models;
pub mod readme;
pub mod rule_expr;
pub mod rules;
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

/// Supported `java` option values: the Java release the output is compiled for, oldest first
/// (the default). Selected with the target option `java = "21"`.
pub const JAVA_VERSIONS: [&str; 2] = ["17", "21"];

/// Options of the `java` generator.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    /// The base Java package. Default: `dev.veld.generated.<package of the first service>`.
    pub package: Option<String>,
    /// `false` skips runtime request validation by default (the types stay). Default: `true`.
    pub validate: Option<bool>,
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

/// The `jdk` framework has no WebSocket server: a contract with `ws` endpoints needs `spring`.
fn check_framework_features(
    req: &GenRequest<'_>,
    profile: &Profile,
) -> Result<(), Vec<GenDiagnostic>> {
    let framework = profile.framework().unwrap_or(frameworks::BASELINE);
    if req.role == Role::Server
        && framework == frameworks::BASELINE
        && req
            .ir
            .features_used(&req.services)
            .contains(&Feature::StreamingWs)
    {
        return Err(vec![GenDiagnostic::new(
            "E0801",
            "generator `java` with framework `jdk` cannot serve `ws` endpoints (the JDK has no WebSocket server); use framework = \"spring\" or serve this contract from another generator",
        )]);
    }
    Ok(())
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
        // Invariants are enforced (`spec/targets/java.md` section 8); the other rules features are
        // not implemented yet: `E0801` (`spec/rules.md` section 8).
        Capabilities::all_except(&[
            Feature::RulesEndpoint,
            Feature::RulesTransitions,
            Feature::RulesPolicies,
        ])
    }

    fn warnings(&self, req: &GenRequest<'_>, opts: &Options) -> Vec<GenDiagnostic> {
        validation_off_warnings(req, opts)
    }

    fn generate(
        &self,
        req: &GenRequest<'_>,
        opts: &Options,
        profile: &Profile,
    ) -> Result<FileSet, Vec<GenDiagnostic>> {
        precheck(req, &self.capabilities(req.role))?;
        check_framework_features(req, profile)?;
        let base = base_package(req, opts)?;
        let runtime_package = runtime::package(&base);
        let mut files = FileSet::new();
        let mut used = req.services.clone();
        used.extend(req.consumes.iter().copied());
        let features = req.ir.features_used(&used);
        let streaming = runtime::Streaming::of(&features);
        let tree = Tree::new(req.ir, &base, &runtime_package, req.services.clone())
            .with_validate(opts.validate != Some(false));
        let rules = features.contains(&Feature::RulesInvariants)
            || declares_rules(&tree)
            || (req.role == Role::Server
                && req.consumes.iter().any(|sid| {
                    declares_rules(&Tree::new(req.ir, &base, &runtime_package, vec![*sid]))
                }));
        runtime::emit(&mut files, &base, streaming, rules)?;
        let tree = tree.with_rules(rules);
        models::emit(&mut files, &tree)?;
        match req.role {
            Role::Client => client::emit(&mut files, &tree)?,
            Role::Server => {
                let model = frameworks::ServerModel {
                    base: base.clone(),
                    services: server::service_models(&tree),
                };
                frameworks::emit(&mut files, &model, profile)?;
                server::emit(&mut files, &tree)?;
                emit_consumed(
                    &mut files,
                    req,
                    &base,
                    &runtime_package,
                    opts.validate != Some(false),
                    rules,
                )?;
            }
        }
        readme::emit(&mut files, &base, req)?;
        Ok(files)
    }
}

/// `W0904` (`spec/rules.md` section 5.3): with `validate = false` request validation is off, and
/// with it the rules on what callers send. Reported when the target emits a declaration with
/// rules or an endpoint with rules, naming the first one. Empty when validation is on.
fn validation_off_warnings(req: &GenRequest<'_>, opts: &Options) -> Vec<GenDiagnostic> {
    if opts.validate != Some(false) {
        return Vec::new();
    }
    let Ok(base) = base_package(req, opts) else {
        return Vec::new();
    };
    let runtime_package = runtime::package(&base);
    let mut ruled: Vec<(String, veld_ir::Origin)> = Vec::new();
    for sid in &req.services {
        let svc = req.ir.service(*sid);
        for ep in svc.endpoints.iter().filter(|e| !e.rules.is_empty()) {
            if let Some(origin) = ep.rules.first_origin() {
                ruled.push((format!("{}.{}", svc.name, ep.name), origin.clone()));
            }
        }
    }
    for group in [&req.services, &req.consumes] {
        if group.is_empty() {
            continue;
        }
        let tree = Tree::new(req.ir, &base, &runtime_package, group.clone());
        for id in tree.decls() {
            let d = req.ir.decl(*id);
            if let Some(rule) = d.rules().first()
                && !ruled.iter().any(|(name, _)| *name == d.qualified)
            {
                ruled.push((d.qualified.clone(), rule.origin.clone()));
            }
        }
    }
    let Some((first, origin)) = ruled.first() else {
        return Vec::new();
    };
    let more = match ruled.len() {
        1 => String::new(),
        n => format!(" and {} more", n - 1),
    };
    vec![GenDiagnostic {
        code: "W0904",
        message: format!(
            "request validation is off (`validate = false`), so the rules of `{first}`{more} are not enforced on requests, WebSocket messages or by the client's local check"
        ),
        origin: Some(origin.clone()),
    }]
}

/// Whether a declaration the tree emits has invariants, reachable from a service or not.
fn declares_rules(tree: &Tree<'_>) -> bool {
    tree.decls()
        .iter()
        .any(|id| rules::has_rules(tree.ir.decl(*id)))
}

/// Emits a client tree under `<base>.clients.<service>` for each consumed service.
fn emit_consumed(
    files: &mut FileSet,
    req: &GenRequest<'_>,
    base: &str,
    runtime_package: &str,
    validate: bool,
    rules: bool,
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
        )
        .with_validate(validate)
        .with_rules(rules);
        models::emit(files, &tree)?;
        client::emit(files, &tree)?;
    }
    Ok(())
}
