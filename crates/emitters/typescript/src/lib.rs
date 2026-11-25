//! The `typescript` generator (role `client`) and the TypeScript building blocks
//! the `node` server generator reuses: models, runtime, validation and clients.
//!
//! See `spec/targets/typescript.md`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod barrel;
pub mod checks;
pub mod client;
pub mod clients;
pub mod expr;
pub mod frameworks;
pub mod models;
pub mod package;
pub mod query;
pub mod readme;
pub mod runtime;
pub mod tree;
pub mod types;
pub mod util;
pub mod validate;

use serde::Deserialize;
use veld_codegen::{
    Capabilities, FileSet, FrameworkInfo, GenDiagnostic, GenRequest, Generator, LanguageInfo,
    Profile, Role, check_capabilities,
};
use veld_ir::Feature;

use crate::util::add_file;

pub use tree::Tree;

/// Features no TypeScript generator supports yet: the rules features other than invariants
/// (`spec/rules.md` section 8), reported as `E0801` until each is implemented. Streaming (SSE and
/// WebSocket) and invariants are supported.
pub const UNSUPPORTED: [Feature; 3] = [
    Feature::RulesEndpoint,
    Feature::RulesTransitions,
    Feature::RulesPolicies,
];

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

/// Emits the files of a client tree (models, errors, prelude, `client.ts`) and returns its module paths.
pub fn emit_client_tree(
    files: &mut FileSet,
    tree: &Tree<'_>,
) -> Result<Vec<String>, Vec<GenDiagnostic>> {
    let mut modules = models::emit(files, tree)?;
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
        Capabilities::all_except(&UNSUPPORTED)
    }

    fn generate(
        &self,
        req: &GenRequest<'_>,
        opts: &Options,
        profile: &Profile,
    ) -> Result<FileSet, Vec<GenDiagnostic>> {
        precheck(req, Role::Client, &self.capabilities(Role::Client))?;
        let mut files = FileSet::new();
        let tree = Tree::new(req.ir, "", None, runtime::MODULE, req.services.clone())
            .with_validate(opts.validate != Some(false));
        runtime::emit(&mut files, tree.uses_rules())?;
        let mut modules = emit_client_tree(&mut files, &tree)?;
        modules.push(clients::emit(&mut files, &tree)?);
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
