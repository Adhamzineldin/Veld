//! The `node` generator (role `server`): service interfaces, the router-core handler and
//! framework adapters, plus typed clients for every consumed service.
//!
//! Models, runtime, validation and clients come from `veld-emit-typescript`, so the same
//! contract produces byte-identical `runtime/` and `models/` files in both roles.
//! See `spec/targets/typescript.md`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod frameworks;
mod rules;
mod server;
mod testing;

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
    /// `false` skips runtime request validation by default (the types stay). Default: `true`.
    pub validate: Option<bool>,
    /// `true` also writes `server/<service>-test-handler.ts` for each service with endpoint rules: a
    /// second entry point that takes its facts, principal, clock and result from request headers
    /// (`spec/rules.md` section 12.4). It must never be mounted in production. Default: `false`.
    pub testing: Option<bool>,
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

    fn warnings(&self, req: &GenRequest<'_>, opts: &Options) -> Vec<GenDiagnostic> {
        veld_emit_typescript::validation_off_warnings(req, opts.validate)
    }

    fn generate(
        &self,
        req: &GenRequest<'_>,
        opts: &Options,
        profile: &Profile,
    ) -> Result<FileSet, Vec<GenDiagnostic>> {
        precheck(req, Role::Server, &self.capabilities(Role::Server))?;
        let mut files = FileSet::new();
        let home = req.ir.service(req.services[0]).package;
        let tree = Tree::new(
            req.ir,
            "",
            Some(home),
            runtime::MODULE,
            req.services.clone(),
        )
        .with_validate(opts.validate != Some(false));
        let model = ServerModel {
            adapters_path: "server/adapters.ts".to_string(),
            services: server::models(&tree),
        };
        frameworks::emit(&mut files, &model, profile)?;
        let mut modules = models::emit(&mut files, &tree)?;
        modules.extend(server::emit(&mut files, &tree)?);
        modules.push("server/adapters".to_string());
        let ws = req
            .ir
            .features_used(&req.services)
            .contains(&veld_ir::Feature::StreamingWs);
        if ws {
            frameworks::emit_ws(&mut files)?;
            modules.push("server/ws".to_string());
        }
        let (extra, consumed) = emit_consumed(&mut files, req, opts.validate != Some(false))?;
        runtime::emit_gated(&mut files, tree.gates().union(consumed))?;
        testing::emit(&mut files, &tree, opts.testing == Some(true))?;
        modules.sort();
        barrel::write(&mut files, "", &modules, &extra, true)?;
        // The `ws` package is a peer dependency of a server that hosts `ws` endpoints.
        let peers: &[(&str, &str)] = if ws { &[("ws", "^8.0.0")] } else { &[] };
        package::emit_with(
            &mut files,
            opts.package_name.as_deref(),
            profile.framework(),
            peers,
        )?;
        let handlers: Vec<(String, String)> = req
            .services
            .iter()
            .map(|s| {
                let name = naming::pascal(&req.ir.service(*s).name);
                (name.clone(), format!("create{name}Handler"))
            })
            .collect();
        veld_emit_typescript::readme::emit(
            &mut files,
            &veld_emit_typescript::readme::Readme {
                package: opts
                    .package_name
                    .as_deref()
                    .unwrap_or(package::DEFAULT_NAME),
                server: true,
                clients: &[],
                handlers: &handlers,
            },
        )?;
        Ok(files)
    }
}

/// Emits a client tree under `clients/<service>/` for each consumed service; returns the barrel lines
/// and the runtime regions the consumed clients need.
fn emit_consumed(
    files: &mut FileSet,
    req: &GenRequest<'_>,
    validate: bool,
) -> Result<(Vec<String>, runtime::Gates), Vec<GenDiagnostic>> {
    let mut lines = Vec::new();
    let mut gates = runtime::Gates::default();
    for sid in &req.consumes {
        let svc = req.ir.service(*sid);
        let dir = naming::kebab(&svc.name);
        let root = format!("clients/{dir}/");
        let tree =
            Tree::new(req.ir, &root, None, runtime::MODULE, vec![*sid]).with_validate(validate);
        gates = gates.union(tree.gates());
        let modules = emit_client_tree(files, &tree)?;
        barrel::write(files, &root, &modules, &[], false)?;
        let ns = naming::escape(Lang::Ts, &naming::camel(&svc.name));
        lines.push(format!("export * as {ns} from './clients/{dir}/index';"));
    }
    lines.sort();
    Ok((lines, gates))
}
