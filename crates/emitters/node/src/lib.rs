//! The `node` generator (role `server`): service interfaces, the router-core handler and
//! framework adapters, plus typed clients for every consumed service.
//!
//! Models, runtime, validation and clients come from `veld-emit-typescript`, so the same
//! contract produces byte-identical `runtime/` and `models/` files in both roles.
//! See `spec/targets/typescript.md`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod brokers;
mod events;
mod frameworks;
mod guards;
mod outbox;
mod retention;
mod rules;
mod server;
mod testing;

pub use brokers::Messaging;
pub use frameworks::{ADAPTERS, EndpointModel, RuntimeRef, ServerModel, ServiceModel};

use serde::Deserialize;
use veld_codegen::naming::{self, Lang};
use veld_codegen::runtime_config::{RULES_OUTPUT, RuntimeModel};
use veld_codegen::{
    Capabilities, FileSet, FrameworkInfo, GenDiagnostic, GenRequest, Generator, LanguageInfo,
    Profile, Role,
};
use veld_emit_typescript::runtime_config::{self as runtime_ts, ClientEntry};
use veld_emit_typescript::tree::Tree;
use veld_emit_typescript::{barrel, clients, emit_client_tree, models, package, precheck, runtime};

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
    /// The broker adapters to generate: `"kafka"`, `"rabbitmq"` or a list of both
    /// (`spec/events.md` section 10.7). Default: none, the memory transport only.
    pub messaging: Option<Messaging>,
    /// `true` adds a `capture` of the request to the event of `onRuleViolation`: the parameters,
    /// the facts, the principal and the clock in wire form, without the values of `@pii` and
    /// `@secret` fields (`spec/rules.md` section 17.3). A capture contains whatever the caller
    /// sent: leave it off unless the hook sends events somewhere such data may go. Default: `false`.
    pub capture_violations: Option<bool>,
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
        Capabilities::all_except(&[])
            .with(veld_ir::Feature::RulesRollout)
            .with(veld_ir::Feature::RulesVersion)
            .with(veld_ir::Feature::Idempotency)
            .with(veld_ir::Feature::RulesDerived)
            .with(veld_ir::Feature::RulesTimed)
            .with(veld_ir::Feature::RulesFlags)
            .with(veld_ir::Feature::Timeouts)
            .with(veld_ir::Feature::Slo)
            .with(veld_ir::Feature::EventsDelivery)
            .with(veld_ir::Feature::EventsOutbox)
            .with(veld_ir::Feature::Visibility)
            .with(veld_ir::Feature::Tenancy)
            .with(veld_ir::Feature::Audit)
            .with(veld_ir::Feature::Limits)
            .with(veld_ir::Feature::Messages)
            .with(veld_ir::Feature::Retention)
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
        let runtime_model = RuntimeModel::for_request(req, profile).map_err(|d| vec![d])?;
        let mut files = FileSet::new();
        let home = req.ir.service(req.services[0]).package;
        let tree = Tree::new(
            req.ir,
            "",
            Some(home),
            runtime::MODULE,
            req.services.clone(),
        )
        .with_validate(opts.validate != Some(false))
        .with_events()
        .with_capture(opts.capture_violations == Some(true));
        let brokers = brokers::choose(opts.messaging.as_ref(), &tree)?;
        let model = ServerModel {
            adapters_path: "server/adapters.ts".to_string(),
            services: server::models(&tree),
            runtime: runtime_model.as_ref().map(frameworks::RuntimeRef::of),
        };
        frameworks::emit(&mut files, &model, profile)?;
        let mut modules = models::emit(&mut files, &tree)?;
        modules.extend(veld_emit_typescript::policy::emit(&mut files, &tree)?);
        modules.extend(server::emit(&mut files, &tree, runtime_model.as_ref())?);
        veld_emit_typescript::messages::emit(&mut files, &tree)?;
        modules.extend(events::emit(&mut files, &tree)?);
        modules.extend(retention::emit(&mut files, &tree, profile)?);
        modules.extend(brokers::emit(&mut files, &brokers)?);
        modules.push("server/adapters".to_string());
        let ws = req
            .ir
            .features_used(&req.services)
            .contains(&veld_ir::Feature::StreamingWs);
        if ws {
            frameworks::emit_ws(&mut files)?;
            modules.push("server/ws".to_string());
        }
        let (extra, consumed, entries) =
            emit_consumed(&mut files, req, opts.validate != Some(false))?;
        let mut gates = tree.gates().union(consumed);
        if let Some(model) = &runtime_model {
            // `rules.output` is typed with the runtime's output mode.
            gates.rules |= model.setting(RULES_OUTPUT).is_some();
            // The runtime is the same for the server and its clients (`ClientCore`).
            gates.runtime_config = true;
            modules.push(runtime_ts::emit(&mut files, &tree, model, &entries)?);
        }
        runtime::emit_gated(&mut files, gates)?;
        testing::emit(&mut files, &tree, opts.testing == Some(true))?;
        modules.sort();
        let mut extra = extra;
        extra.extend(veld_emit_typescript::messages::exports(&tree, ""));
        if tree.uses_events() {
            extra.push("export * as veldEvents from './runtime/events';".to_string());
        }
        barrel::write(&mut files, "", &modules, &extra, true)?;
        // The `ws` package is a peer dependency of a server that hosts `ws` endpoints.
        let mut peers: Vec<(&str, &str)> = if ws { vec![("ws", "^8.0.0")] } else { vec![] };
        peers.extend(brokers::peers(&brokers));
        package::emit_with(
            &mut files,
            opts.package_name.as_deref(),
            profile.framework(),
            &peers,
        )?;
        let handlers: Vec<(String, String)> = req
            .services
            .iter()
            .filter(|s| events::serves_http(req.ir.service(**s)))
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

/// What the consumed clients add: the barrel lines, the runtime regions their code needs, and the
/// entries `createClients` of `runtime.ts` builds.
type Consumed = (Vec<String>, runtime::Gates, Vec<ClientEntry>);

/// Emits a client tree under `clients/<service>/` for each consumed service.
fn emit_consumed(
    files: &mut FileSet,
    req: &GenRequest<'_>,
    validate: bool,
) -> Result<Consumed, Vec<GenDiagnostic>> {
    let mut lines = Vec::new();
    let mut gates = runtime::Gates::default();
    let mut entries = Vec::new();
    for sid in &req.consumes {
        let svc = req.ir.service(*sid);
        let dir = naming::kebab(&svc.name);
        let root = format!("clients/{dir}/");
        let tree = Tree::new(req.ir, &root, None, runtime::MODULE, vec![*sid])
            .with_validate(validate)
            .with_runtime_config(true);
        gates = gates.union(tree.gates());
        let modules = emit_client_tree(files, &tree)?;
        entries.extend(clients::entries(&tree));
        let extra = veld_emit_typescript::messages::exports(&tree, &root);
        barrel::write(files, &root, &modules, &extra, false)?;
        let ns = naming::escape(Lang::Ts, &naming::camel(&svc.name));
        lines.push(format!("export * as {ns} from './clients/{dir}/index';"));
    }
    lines.sort();
    qualify_duplicates(req.ir, &mut entries);
    Ok((lines, gates, entries))
}

/// Two consumed services of one name (in two packages) get their package in the key, as the
/// clients of a client target do.
fn qualify_duplicates(ir: &veld_ir::Ir, entries: &mut [ClientEntry]) {
    let keys: Vec<String> = entries.iter().map(|e| e.key.clone()).collect();
    for e in entries.iter_mut() {
        if keys.iter().filter(|k| **k == e.key).count() > 1 {
            let pkg = ir.package(ir.service(e.service).package).name.clone();
            let pkg = pkg.trim_start_matches('@').replace('/', "-");
            e.key = format!("{}{}", naming::camel(&pkg), naming::pascal(&e.key));
        }
    }
}
