//! The `typescript` generator (role `client`) and the TypeScript building blocks
//! the `node` server generator reuses: models, runtime, validation and clients.
//!
//! See `spec/targets/typescript.md`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod barrel;
pub mod client;
pub mod frameworks;
pub mod models;
pub mod package;
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

/// Features no TypeScript generator supports yet.
pub const UNSUPPORTED: [Feature; 2] = [Feature::StreamingSse, Feature::StreamingWs];

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
        runtime::emit(&mut files)?;
        let tree = Tree::new(req.ir, "", None, runtime::MODULE, req.services.clone());
        let modules = emit_client_tree(&mut files, &tree)?;
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
        Ok(files)
    }
}
