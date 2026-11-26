//! The `rust` generator (server and client): serde models, typed errors, a framework-agnostic
//! server core with an axum adapter, and a client generic over its HTTP transport.
//!
//! See `spec/targets/rust.md`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod block;
pub mod brokers;
pub mod client;
pub mod conditional;
pub mod config;
pub mod endpoints;
pub mod events;
pub mod guards;
pub mod models;
pub mod package;
pub mod policies;
pub mod rules;
pub mod runtime;
pub mod server;
pub mod testing;
pub mod timed;
pub mod tree;
pub mod types;
pub mod util;
pub mod warn;

use std::collections::BTreeMap;

use serde::Deserialize;
use veld_codegen::{
    Capabilities, FileSet, FrameworkInfo, GenDiagnostic, GenRequest, Generator, LanguageInfo,
    Profile, Role, check_capabilities, naming, runtime_config::RuntimeModel,
};
use veld_ir::{Feature, ServiceId};

use crate::client::Transports;
use crate::runtime::Features;
use crate::tree::Tree;

/// The server framework of a target that names none.
pub const DEFAULT_SERVER_FRAMEWORK: &str = "axum";
/// The client transport of a target that names none.
pub const DEFAULT_CLIENT_FRAMEWORK: &str = "reqwest";

/// Options of the `rust` generator.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Options {
    /// The name of the generated crate. Default: `<package of the first service>-<role>`.
    pub crate_name: Option<String>,
    /// The HTTP transport of the clients inside a server (`consumes`): `reqwest` (default),
    /// `ureq` or `none` (bring your own `veld::client::Transport`).
    pub transport: Option<String>,
    /// `false` skips run-time request validation by default (the types are unchanged).
    pub validate: Option<bool>,
    /// `true` adds the test handler (`spec/rules.md` section 12.4) next to every service that has
    /// endpoint rules: a second entry point, in a file of its own, for `veld test --live`. Server
    /// role only. Never mount it in production.
    pub testing: Option<bool>,
    /// The broker adapters of events phase E2 (`spec/events.md` section 10.7): `"kafka"`,
    /// `"rabbitmq"` or a list of both. Server role only; a target none of whose services
    /// publishes or subscribes cannot have it.
    pub messaging: Option<brokers::Messaging>,
}

/// The Rust generator (roles `server` and `client`).
#[derive(Debug, Default, Clone, Copy)]
pub struct Rust;

fn server_frameworks() -> Vec<FrameworkInfo> {
    vec![FrameworkInfo {
        id: "axum".into(),
        versions: vec!["0.8".into()],
        min_language: None,
    }]
}

fn client_frameworks() -> Vec<FrameworkInfo> {
    vec![
        FrameworkInfo {
            id: "reqwest".into(),
            versions: vec!["0.13".into()],
            min_language: None,
        },
        FrameworkInfo {
            id: "ureq".into(),
            versions: vec!["2".into()],
            min_language: None,
        },
    ]
}

/// What the generator supports: everything. A server generates events (phase E1); a client
/// supports the feature by generating nothing for it (`spec/events.md` section 14).
fn all_capabilities() -> Capabilities {
    Capabilities::all_except(&[])
        .with(Feature::RulesRollout)
        .with(Feature::RulesVersion)
        .with(Feature::Idempotency)
        .with(Feature::RulesDerived)
        .with(Feature::EventsDelivery)
        .with(Feature::RulesTimed)
}

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

fn crate_name(req: &GenRequest<'_>, opts: &Options) -> Result<String, Vec<GenDiagnostic>> {
    if let Some(n) = &opts.crate_name {
        let ok = !n.is_empty()
            && n.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            && n.starts_with(|c: char| c.is_ascii_alphabetic());
        return if ok {
            Ok(n.clone())
        } else {
            Err(vec![GenDiagnostic::new(
                "E0802",
                format!("invalid options for generator `rust`: `{n}` is not a valid crate name"),
            )])
        };
    }
    let home = req.ir.package(req.ir.service(req.services[0]).package);
    let pkg = naming::kebab(home.name.trim_start_matches('@').replace('/', "-").as_str());
    Ok(format!(
        "{}-{}",
        if pkg.is_empty() { "veld" } else { pkg.as_str() },
        req.role.name()
    ))
}

fn transports(
    req: &GenRequest<'_>,
    opts: &Options,
    profile: &Profile,
) -> Result<Transports, Vec<GenDiagnostic>> {
    if req.role == Role::Client {
        return Ok(
            match profile.framework().unwrap_or(DEFAULT_CLIENT_FRAMEWORK) {
                "ureq" => Transports {
                    reqwest: false,
                    ureq: true,
                },
                _ => Transports {
                    reqwest: true,
                    ureq: false,
                },
            },
        );
    }
    if req.consumes.is_empty() {
        return Ok(Transports::default());
    }
    match opts.transport.as_deref().unwrap_or("reqwest") {
        "reqwest" => Ok(Transports {
            reqwest: true,
            ureq: false,
        }),
        "ureq" => Ok(Transports {
            reqwest: false,
            ureq: true,
        }),
        "none" => Ok(Transports::default()),
        other => Err(vec![GenDiagnostic::new(
            "E0802",
            format!(
                "invalid options for generator `rust`: unknown transport `{other}`; expected one of: reqwest, ureq, none"
            ),
        )]),
    }
}

impl Generator for Rust {
    type Options = Options;

    fn id(&self) -> &'static str {
        "rust"
    }

    fn roles(&self) -> &'static [Role] {
        &[Role::Server, Role::Client]
    }

    fn frameworks(&self, role: Role) -> Vec<FrameworkInfo> {
        if role == Role::Server {
            server_frameworks()
        } else if role == Role::Client {
            client_frameworks()
        } else {
            Vec::new()
        }
    }

    fn language(&self) -> Option<LanguageInfo> {
        None
    }

    fn capabilities(&self, _role: Role) -> Capabilities {
        // Everything: invariants, endpoint rules, state machines, policies and events are
        // generated (`spec/targets/rust.md` sections 10 and 13). The limits that depend on the
        // framework or the endpoint are reported as `E0801` by `generate`: the blocking `ureq`
        // transport cannot open WebSockets, and an endpoint rule on a `ws` endpoint is not
        // supported.
        all_capabilities()
    }

    fn warnings(&self, req: &GenRequest<'_>, opts: &Options) -> Vec<GenDiagnostic> {
        warn::validation_off(req, opts.validate)
    }

    fn generate(
        &self,
        req: &GenRequest<'_>,
        opts: &Options,
        profile: &Profile,
    ) -> Result<FileSet, Vec<GenDiagnostic>> {
        if req.role != Role::Server && req.role != Role::Client {
            return Err(vec![GenDiagnostic::new(
                "E0803",
                "generator `rust` supports the `server` and `client` roles only",
            )]);
        }
        precheck(req, &self.capabilities(req.role))?;
        let model = RuntimeModel::for_request(req, profile).map_err(|d| vec![d])?;
        let runtime = model.as_ref().map(config::RuntimeCfg::of);
        let name = crate_name(req, opts)?;
        let tr = transports(req, opts, profile)?;
        let mut used = req.services.clone();
        used.extend(req.consumes.iter().copied());
        let used_features = req.ir.http_features_used(&used);
        let streaming = used_features.contains(&Feature::StreamingSse)
            || used_features.contains(&Feature::StreamingWs);
        let server = req.role == Role::Server;
        // The clients are the services of a client target, or the consumed services of a server.
        let client_services = if server { &req.consumes } else { &req.services };
        let client_ws = req
            .ir
            .features_used(client_services)
            .contains(&Feature::StreamingWs);
        if client_ws && tr.ureq {
            return Err(util::unsupported(
                "generator `rust` cannot open WebSockets on the `ureq` transport (it is blocking and has no WebSocket client); use the `reqwest` framework, or serve this contract from another generator",
            ));
        }
        refuse_ws_blocks(req)?;
        conditional::check(req.ir, &used)?;
        if opts.testing == Some(true) && req.role != Role::Server {
            return Err(vec![GenDiagnostic::new(
                "E0802",
                "invalid options for generator `rust`: `testing` is an option of the server role",
            )]);
        }
        let testing = opts.testing == Some(true);
        let chosen = brokers::choose(opts.messaging.as_ref(), req.role, &main_tree(req))?;
        let idempotency = server && conditional::any_idempotent(req.ir, &served_http(req));
        // The test handler of a service that is only idempotent runs the steps of the pipeline
        // too: it needs the endpoint rules runtime.
        let endpoint_rules =
            server && (block::served_has_blocks(req.ir, &req.services) || (testing && idempotency));
        // A policy is generated, in either role, for an `allow` of a generated service: its
        // functions use the rules runtime.
        let policies = used.iter().any(|s| {
            req.ir
                .service(*s)
                .endpoints
                .iter()
                .any(|e| !e.rules.allows.is_empty())
        });
        let guards = server
            && req.services.iter().any(|s| {
                let svc = req.ir.service(*s);
                guards::svc_has_guards(svc) || svc.endpoints.iter().any(conditional::has_guard)
            });
        let timed = server
            && served_http(req).iter().any(|s| {
                req.ir
                    .service(*s)
                    .endpoints
                    .iter()
                    .any(|e| e.rules.transition.as_ref().is_some_and(|t| t.is_timed()))
            });
        let etag = server && conditional::any_version(req.ir, &served_http(req));
        let version = etag
            || conditional::any_guard(req.ir, if server { &req.consumes } else { &req.services });
        // A declaration that no endpoint reaches is emitted too, and it calls `vr::audit` for its
        // audited rules: the rollout regions are kept for every audited rule the crate has.
        let rollout = used_features.contains(&Feature::RulesRollout) || crate_has_audited(req);
        // A runtime config with `rules.output` needs the rules runtime to carry the mode.
        let derived = crate_has_derived(req);
        let rules = crate_has_rules(req)
            || derived
            || endpoint_rules
            || policies
            || rollout
            || runtime.as_ref().is_some_and(|r| r.output);
        // A server whose services only publish or subscribe has no HTTP surface at all: no server
        // core, no adapter, no framework dependency.
        let serving = server
            && req
                .services
                .iter()
                .any(|s| events::serves_http(req.ir.service(*s)));
        let f = Features {
            events: main_tree(req).uses_events(),
            version,
            etag,
            idempotency,
            conditional: etag || idempotency,
            rules,
            endpoint_rules,
            guards,
            timed,
            policies,
            rollout,
            server: serving,
            client: !server || !req.consumes.is_empty(),
            sse: streaming,
            ws_server: serving
                && req
                    .ir
                    .features_used(&req.services)
                    .contains(&Feature::StreamingWs),
            ws_client: client_ws,
            axum: serving && profile.framework().unwrap_or(DEFAULT_SERVER_FRAMEWORK) == "axum",
            reqwest: tr.reqwest,
            ureq: tr.ureq,
            config: model.is_some(),
            derived,
            test_events: server
                && testing
                && req
                    .services
                    .iter()
                    .any(|s| !req.ir.service(*s).subscriptions.is_empty()),
            kafka: chosen.contains(&brokers::Broker::Kafka),
            rabbitmq: chosen.contains(&brokers::Broker::RabbitMq),
        };
        let mut files = FileSet::new();
        runtime::emit(&mut files, &f)?;
        package::emit_cargo(&mut files, &name, &f)?;
        let runtime_type = model.as_ref().map(RuntimeModel::type_name);
        package::emit_readme(&mut files, &name, req.role, &f, runtime_type.as_deref())?;
        let validate = opts.validate != Some(false);
        let main = main_tree(req)
            .with_validate(validate)
            .with_rules(rules)
            .with_rollout(rollout)
            .with_derived(derived)
            .with_idempotency(idempotency)
            .with_timed(timed)
            .with_runtime(runtime);
        if let Some(model) = &model {
            config::emit(&mut files, model, req.ir, req.role, tr)?;
        }
        models::emit(&mut files, &main)?;
        policies::emit(&mut files, &main)?;
        if !files.contains("src/prelude.rs") {
            util::add_file(&mut files, "src/prelude.rs", format!("{}\n", util::HEADER))?;
        }
        let mut mods: BTreeMap<ServiceId, Vec<String>> = BTreeMap::new();
        for sid in &req.services {
            let svc = req.ir.service(*sid);
            let m = if !server {
                client::client_mod(svc)
            } else if events::serves_http(svc) {
                server::service_mod(svc)
            } else {
                continue;
            };
            mods.entry(*sid).or_default().push(m);
        }
        if server {
            server::emit(&mut files, &main)?;
            for sid in events::emit(&mut files, &main)? {
                mods.entry(sid)
                    .or_default()
                    .push(events::events_mod(req.ir.service(sid)));
            }
            for sid in testing::emit(&mut files, &main, opts.testing == Some(true))? {
                mods.entry(sid)
                    .or_default()
                    .push(testing::test_mod(req.ir.service(sid)));
            }
        } else {
            client::emit(&mut files, &main, tr)?;
        }
        let packages = package::emit_modules(&mut files, &main, &mods)?;
        let has_clients = emit_consumed(&mut files, req, tr, (validate, rules, derived))?;
        package::emit_lib(&mut files, &packages, has_clients, &f, req.role)?;
        Ok(files)
    }
}

/// The services of a server that have an HTTP surface.
fn served_http(req: &GenRequest<'_>) -> Vec<ServiceId> {
    req.services
        .iter()
        .copied()
        .filter(|s| events::serves_http(req.ir.service(*s)))
        .collect()
}

/// `E0801` for an endpoint block on a `ws` endpoint that a server serves: a conversation has no
/// single result and no request/response pair to apply steps 6 and 9 to. A client enforces no
/// block, so it generates.
fn refuse_ws_blocks(req: &GenRequest<'_>) -> Result<(), Vec<GenDiagnostic>> {
    if req.role != Role::Server {
        return Ok(());
    }
    for sid in &req.services {
        let svc = req.ir.service(*sid);
        for ep in &svc.endpoints {
            if endpoints::kind(ep) == endpoints::Kind::Ws && !ep.rules.is_empty() {
                return Err(util::unsupported(format!(
                    "generator `rust` does not support endpoint rules on the `ws` endpoint `{}.{}`",
                    svc.name, ep.name
                )));
            }
        }
    }
    Ok(())
}

/// The tree of the crate's own services. A server also has the events of its channels
/// (`spec/events.md` section 14); a client never has them.
fn main_tree<'a>(req: &GenRequest<'a>) -> Tree<'a> {
    let tree = Tree::new(req.ir, "crate", "src", req.services.clone(), true);
    if req.role == Role::Server {
        tree.with_events()
    } else {
        tree
    }
}

/// Whether any declaration that the crate emits has invariants. The flag is per crate: with it,
/// every schema carries its check function and the rules runtime is part of the crate.
fn crate_has_rules(req: &GenRequest<'_>) -> bool {
    let main = main_tree(req);
    main.has_rules()
        || req.consumes.iter().any(|sid| {
            Tree::new(
                req.ir,
                "crate::clients::x",
                "src/clients/x",
                vec![*sid],
                false,
            )
            .has_rules()
        })
}

/// Whether any declaration that the crate emits has a derived field. The flag is per crate: with
/// it, every schema says which of its fields are derived.
fn crate_has_derived(req: &GenRequest<'_>) -> bool {
    let main = main_tree(req);
    main.has_derived()
        || req.consumes.iter().any(|sid| {
            Tree::new(
                req.ir,
                "crate::clients::x",
                "src/clients/x",
                vec![*sid],
                false,
            )
            .has_derived()
        })
}

/// Whether any declaration that the crate emits has an audited invariant (`@rollout(audit)`).
fn crate_has_audited(req: &GenRequest<'_>) -> bool {
    let main = main_tree(req);
    main.has_audited()
        || req.consumes.iter().any(|sid| {
            Tree::new(
                req.ir,
                "crate::clients::x",
                "src/clients/x",
                vec![*sid],
                false,
            )
            .has_audited()
        })
}

/// A client tree under `src/clients/<service>` for each consumed service of a server.
fn emit_consumed(
    files: &mut FileSet,
    req: &GenRequest<'_>,
    tr: Transports,
    (validate, rules, derived): (bool, bool, bool),
) -> Result<bool, Vec<GenDiagnostic>> {
    if req.consumes.is_empty() {
        return Ok(false);
    }
    let mut names = Vec::new();
    for sid in &req.consumes {
        let svc = req.ir.service(*sid);
        let dir_name = util::module(&svc.name);
        let root = format!("crate::clients::{dir_name}");
        let dir = format!("src/clients/{dir_name}");
        let tree = Tree::new(req.ir, &root, &dir, vec![*sid], false)
            .with_validate(validate)
            .with_rules(rules)
            .with_derived(derived);
        models::emit(files, &tree)?;
        policies::emit(files, &tree)?;
        client::emit(files, &tree, tr)?;
        let mods = BTreeMap::from([(*sid, vec![client::client_mod(svc)])]);
        let packages = package::emit_modules(files, &tree, &mods)?;
        let mut text = format!("{}\n\n", util::HEADER);
        for p in packages {
            text.push_str(&format!("pub mod {p};\n"));
        }
        util::add_file(files, &format!("{dir}/mod.rs"), text)?;
        names.push(dir_name);
    }
    names.sort();
    names.dedup();
    let mut text = format!(
        "{}\n\n//! A client for each service this server consumes.\n\n",
        util::HEADER
    );
    for n in names {
        text.push_str(&format!("pub mod {n};\n"));
    }
    util::add_file(files, "src/clients/mod.rs", text)?;
    Ok(true)
}
