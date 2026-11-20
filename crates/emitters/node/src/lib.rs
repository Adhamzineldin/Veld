//! The `node` generator (role `server`): service interfaces, the router-core handler and
//! framework adapters, plus typed clients for every consumed service.
//!
//! Models, runtime, validation and clients come from `veld-emit-typescript`, so the same
//! contract produces byte-identical `runtime/` and `models/` files in both roles.
//! See `spec/targets/typescript.md`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod frameworks;
mod server;

pub use frameworks::{ADAPTERS, EndpointModel, ServerModel, ServiceModel};

use serde::Deserialize;
use veld_codegen::naming::{self, Lang};
use veld_codegen::{
    Capabilities, FileSet, FrameworkInfo, GenDiagnostic, GenRequest, Generator, LanguageInfo,
    Profile, Role,
};
use veld_emit_typescript::tree::Tree;
use veld_emit_typescript::{
    UNSUPPORTED, barrel, emit_client_tree, models, package, precheck, runtime,
};

/// Options of the `node` generator.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Options {
    /// The name in the generated `package.json` (default: `@veld/<target name>`).
    pub package_name: Option<String>,
}

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

    fn frameworks(&self, _role: Role) -> Vec<FrameworkInfo> {
        frameworks::infos()
    }

    fn language(&self) -> Option<LanguageInfo> {
        Some(veld_emit_typescript::language())
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
        precheck(req, Role::Server, &self.capabilities(Role::Server))?;
        let mut files = FileSet::new();
        runtime::emit(&mut files)?;
        let home = req.ir.service(req.services[0]).package;
        let tree = Tree::new(
            req.ir,
            "",
            Some(home),
            runtime::MODULE,
            req.services.clone(),
        );
        let model = ServerModel {
            adapters_path: "server/adapters.ts".to_string(),
            services: server::models(&tree),
        };
        frameworks::emit(&mut files, &model, profile)?;
        let mut modules = models::emit(&mut files, &tree)?;
        modules.extend(server::emit(&mut files, &tree)?);
        modules.push("server/adapters".to_string());
        let extra = emit_consumed(&mut files, req)?;
        modules.sort();
        barrel::write(&mut files, "", &modules, &extra, true)?;
        package::emit(
            &mut files,
            opts.package_name.as_deref(),
            profile.framework(),
        )?;
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
