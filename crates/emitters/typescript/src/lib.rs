//! The `typescript` generator (role `client`) and the TypeScript building blocks
//! the `node` server generator reuses: models, runtime, validation and clients.
//!
//! See `spec/targets/typescript.md`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod barrel;
pub mod can;
pub mod checks;
pub mod client;
pub mod clients;
pub mod derived;
pub mod expr;
pub mod frameworks;
pub mod machine;
pub mod models;
pub mod package;
pub mod platform;
pub mod policy;
pub mod query;
pub mod readme;
pub mod runtime;
pub mod runtime_config;
pub mod tree;
pub mod types;
pub mod util;
pub mod validate;
pub mod version;

use serde::Deserialize;
use veld_codegen::runtime_config::RuntimeModel;
use veld_codegen::{
    Capabilities, FileSet, FrameworkInfo, GenDiagnostic, GenRequest, Generator, LanguageInfo,
    Profile, Role, check_capabilities,
};
use veld_ir::Feature;

use crate::util::add_file;

pub use tree::Tree;

/// Features no TypeScript generator supports: none. Invariants, endpoint rules, state machines and
/// policies (`spec/rules.md` section 8) and streaming (SSE and WebSocket) are all supported.
pub const UNSUPPORTED: [Feature; 0] = [];

/// The TypeScript language versions: ECMAScript editions, the oldest first (the default).
/// Selected with the target option `typescript = "es2022"`.
#[must_use]
pub fn language() -> LanguageInfo {
    LanguageInfo {
        id: "typescript".to_string(),
        versions: ES_EDITIONS.iter().map(|v| (*v).to_string()).collect(),
    }
}

/// Supported `typescript` option values.
pub const ES_EDITIONS: [&str; 2] = ["es2020", "es2022"];

/// Options of the `typescript` generator.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Options {
    /// The name in the generated `package.json` (default: `@veld/<target name>`).
    pub package_name: Option<String>,
    /// `false` skips runtime request validation by default (the types stay). Default: `true`.
    pub validate: Option<bool>,
    /// `true` also generates `<package>/queries.ts`: query keys, query options and mutation options as
    /// plain objects (no imports; usable with TanStack Query, SWR or your own hooks). Default: `false`.
    pub query_options: Option<bool>,
}

/// The TypeScript client generator.
#[derive(Debug, Default, Clone, Copy)]
pub struct TypeScript;

/// Checks the request is well formed for a generator that supports `role`:
/// the role matches, services exist, every used feature is supported.
pub fn precheck(
    req: &GenRequest<'_>,
    role: Role,
    caps: &Capabilities,
) -> Result<(), Vec<GenDiagnostic>> {
    if req.role != role {
        return Err(vec![GenDiagnostic::new(
            "E0803",
            format!("this generator only produces the `{}` role", role.name()),
        )]);
    }
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

/// `W0904` (`spec/rules.md` section 5.3): the target option `validate = false` turns request
/// validation off, and with it the rules on what callers send. Reported when the target emits
/// a declaration with rules (the same predicate that adds `runtime/rules.ts`), naming the first
/// one and pointing at its first rule. Empty when validation is on or the contract has no rules.
#[must_use]
pub fn validation_off_warnings(req: &GenRequest<'_>, validate: Option<bool>) -> Vec<GenDiagnostic> {
    if validate != Some(false) {
        return Vec::new();
    }
    let mut ruled: Vec<(String, veld_ir::Origin)> = Vec::new();
    for group in std::iter::once(&req.services).chain(std::iter::once(&req.consumes)) {
        if group.is_empty() {
            continue;
        }
        let tree = Tree::new(req.ir, "", None, runtime::MODULE, group.clone());
        for id in tree.decls() {
            let d = req.ir.decl(*id);
            if let Some(rule) = d.rules().first()
                && !ruled.iter().any(|(name, _)| *name == d.qualified)
            {
                ruled.push((d.qualified.clone(), rule.origin.clone()));
            }
        }
        for sid in group {
            let svc = req.ir.service(*sid);
            for ep in &svc.endpoints {
                if let Some(origin) = ep.rules.first_origin() {
                    ruled.push((format!("{}.{}", svc.name, ep.name), origin.clone()));
                }
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

/// Emits the files of a client tree (models, errors, prelude, `client.ts`) and returns its module paths.
pub fn emit_client_tree(
    files: &mut FileSet,
    tree: &Tree<'_>,
) -> Result<Vec<String>, Vec<GenDiagnostic>> {
    let mut modules = models::emit(files, tree)?;
    modules.extend(policy::emit(files, tree)?);
    modules.extend(client::emit(files, tree)?);
    modules.sort();
    Ok(modules)
}

impl Generator for TypeScript {
    type Options = Options;

    fn id(&self) -> &'static str {
        "typescript"
    }

    fn roles(&self) -> &'static [Role] {
        &[Role::Client]
    }

    fn frameworks(&self, role: Role) -> Vec<FrameworkInfo> {
        match role {
            Role::Client => frameworks::infos(),
            Role::Server => Vec::new(),
        }
    }

    fn language(&self) -> Option<LanguageInfo> {
        Some(language())
    }

    fn capabilities(&self, _role: Role) -> Capabilities {
        // A client skips audited rules in the local check and in `partial` (`spec/rules.md` 17.1).
        Capabilities::all_except(&UNSUPPORTED)
            .with(veld_ir::Feature::RulesRollout)
            .with(veld_ir::Feature::RulesVersion)
            .with(veld_ir::Feature::Idempotency)
            .with(veld_ir::Feature::RulesDerived)
            .with(veld_ir::Feature::RulesTimed)
            .with(veld_ir::Feature::RulesFlags)
            .with(veld_ir::Feature::Timeouts)
            .with(veld_ir::Feature::Slo)
    }

    fn warnings(&self, req: &GenRequest<'_>, opts: &Options) -> Vec<GenDiagnostic> {
        validation_off_warnings(req, opts.validate)
    }

    fn generate(
        &self,
        req: &GenRequest<'_>,
        opts: &Options,
        profile: &Profile,
    ) -> Result<FileSet, Vec<GenDiagnostic>> {
        precheck(req, Role::Client, &self.capabilities(Role::Client))?;
        let runtime = RuntimeModel::for_request(req, profile).map_err(|d| vec![d])?;
        let mut files = FileSet::new();
        let tree = Tree::new(req.ir, "", None, runtime::MODULE, req.services.clone())
            .with_validate(opts.validate != Some(false))
            .with_runtime_config(runtime.is_some());
        runtime::emit_gated(&mut files, tree.gates())?;
        let mut modules = emit_client_tree(&mut files, &tree)?;
        if let Some(model) = &runtime {
            modules.push(runtime_config::emit(
                &mut files,
                &tree,
                model,
                &clients::entries(&tree),
            )?);
        }
        if opts.query_options == Some(true) {
            modules.extend(query::emit(&mut files, &tree)?);
        }
        modules.sort();
        let model = frameworks::model(&tree);
        for adapter in frameworks::ADAPTERS {
            for (path, bytes) in adapter.files(&model, profile)?.iter() {
                add_file(
                    &mut files,
                    path,
                    String::from_utf8_lossy(bytes).into_owned(),
                )?;
            }
        }
        barrel::write(&mut files, "", &modules, &[], true)?;
        package::emit(
            &mut files,
            opts.package_name.as_deref(),
            profile.framework(),
        )?;
        readme::emit(
            &mut files,
            &readme::Readme {
                package: opts
                    .package_name
                    .as_deref()
                    .unwrap_or(package::DEFAULT_NAME),
                server: false,
                clients: &clients::keys(&tree),
                handlers: &[],
            },
        )?;
        Ok(files)
    }
}
