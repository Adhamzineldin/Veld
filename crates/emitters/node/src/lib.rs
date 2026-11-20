//! The `node` generator (role `server`): service interfaces, the router-core handler and
//! framework adapters, plus typed clients for every consumed service.
//!
//! Models, runtime, validation and clients come from `veld-emit-typescript`, so the same
//! contract produces byte-identical `runtime/` and `models/` files in both roles.
//! See `spec/targets/typescript.md`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod server;

use serde::Deserialize;
use veld_codegen::naming::{self, Lang};
use veld_codegen::{Capabilities, FileSet, GenDiagnostic, GenRequest, Generator, Role};
use veld_emit_typescript::tree::Tree;
use veld_emit_typescript::util::add_file;
use veld_emit_typescript::{UNSUPPORTED, barrel, emit_client_tree, models, precheck, runtime};

/// Frameworks the adapters cover. All four adapters are always emitted.
const FRAMEWORKS: &[&str] = &["express", "fastify", "hono", "node-http"];

/// Source of `server/adapters.ts`.
pub const ADAPTERS_SOURCE: &str = include_str!("../runtime/adapters.ts");

/// Options of the `node` generator (none yet).
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {}

/// The Node.js server generator.
#[derive(Debug, Default, Clone, Copy)]
pub struct Node;

impl Generator for Node {
    type Options = Options;

    fn id(&self) -> &'static str {
        "node"
    }

    fn roles(&self) -> &'static [Role] {
        &[Role::Server]
    }

    fn frameworks(&self, _role: Role) -> &'static [&'static str] {
        FRAMEWORKS
    }

    fn capabilities(&self, _role: Role) -> Capabilities {
        Capabilities::all_except(&UNSUPPORTED)
    }

    fn generate(
        &self,
        req: &GenRequest<'_>,
        _opts: &Options,
    ) -> Result<FileSet, Vec<GenDiagnostic>> {
        precheck(req, Role::Server, &self.capabilities(Role::Server))?;
        if let Some(f) = &req.framework
            && !FRAMEWORKS.contains(&f.as_str())
        {
            return Err(vec![GenDiagnostic::new(
                "E0803",
                format!("unknown framework `{f}`; expected one of {FRAMEWORKS:?}"),
            )]);
        }
        let mut files = FileSet::new();
        runtime::emit(&mut files)?;
        add_file(
            &mut files,
            "server/adapters.ts",
            ADAPTERS_SOURCE.to_string(),
        )?;
        let home = req.ir.service(req.services[0]).package;
        let tree = Tree::new(
            req.ir,
            "",
            Some(home),
            runtime::MODULE,
            req.services.clone(),
        );
        let mut modules = models::emit(&mut files, &tree)?;
        modules.extend(server::emit(&mut files, &tree)?);
        modules.push("server/adapters".to_string());
        let extra = emit_consumed(&mut files, req)?;
        modules.sort();
        barrel::write(&mut files, "", &modules, &extra, true)?;
        Ok(files)
    }
}

/// Emits a client tree under `clients/<service>/` for each consumed service; returns barrel lines.
fn emit_consumed(
    files: &mut FileSet,
    req: &GenRequest<'_>,
) -> Result<Vec<String>, Vec<GenDiagnostic>> {
    let mut lines = Vec::new();
    for sid in &req.consumes {
        let svc = req.ir.service(*sid);
        let dir = naming::kebab(&svc.name);
        let root = format!("clients/{dir}/");
        let tree = Tree::new(req.ir, &root, None, runtime::MODULE, vec![*sid]);
        let modules = emit_client_tree(files, &tree)?;
        barrel::write(files, &root, &modules, &[], false)?;
        let ns = naming::escape(Lang::Ts, &naming::camel(&svc.name));
        lines.push(format!("export * as {ns} from './clients/{dir}/index';"));
    }
    lines.sort();
    Ok(lines)
}
