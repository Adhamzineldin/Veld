//! The `rust` generator (server and client): serde models, typed errors, a framework-agnostic
//! server core with an axum adapter, and a client generic over its HTTP transport.
//!
//! See `spec/targets/rust.md`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod client;
pub mod endpoints;
pub mod models;
pub mod package;
pub mod runtime;
pub mod server;
pub mod tree;
pub mod types;
pub mod util;

use std::collections::BTreeMap;

use serde::Deserialize;
use veld_codegen::{
    Capabilities, FileSet, FrameworkInfo, GenDiagnostic, GenRequest, Generator, LanguageInfo,
    Profile, Role, check_capabilities, naming,
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
        // Everything is supported. The one limit is framework dependent and reported as `E0801` by
        // `generate`: the blocking `ureq` transport cannot open WebSockets.
        Capabilities::all_except(&[])
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
        let name = crate_name(req, opts)?;
        let tr = transports(req, opts, profile)?;
        let mut used = req.services.clone();
        used.extend(req.consumes.iter().copied());
        let used_features = req.ir.features_used(&used);
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
        let f = Features {
            server,
            client: !server || !req.consumes.is_empty(),
            sse: streaming,
            ws_server: server
                && req
                    .ir
                    .features_used(&req.services)
                    .contains(&Feature::StreamingWs),
            ws_client: client_ws,
            axum: server && profile.framework().unwrap_or(DEFAULT_SERVER_FRAMEWORK) == "axum",
            reqwest: tr.reqwest,
            ureq: tr.ureq,
        };
        let mut files = FileSet::new();
        runtime::emit(&mut files, &f)?;
        package::emit_cargo(&mut files, &name, &f)?;
        package::emit_readme(&mut files, &name, req.role, &f)?;
        let validate = opts.validate != Some(false);
        let main =
            Tree::new(req.ir, "crate", "src", req.services.clone(), true).with_validate(validate);
        models::emit(&mut files, &main)?;
        if !files.contains("src/prelude.rs") {
            util::add_file(&mut files, "src/prelude.rs", format!("{}\n", util::HEADER))?;
        }
        let mut mods: BTreeMap<ServiceId, Vec<String>> = BTreeMap::new();
        for sid in &req.services {
            let svc = req.ir.service(*sid);
            let m = if server {
                server::service_mod(svc)
            } else {
                client::client_mod(svc)
            };
            mods.entry(*sid).or_default().push(m);
        }
        if server {
            server::emit(&mut files, &main)?;
        } else {
            client::emit(&mut files, &main, tr)?;
        }
        let packages = package::emit_modules(&mut files, &main, &mods)?;
        let has_clients = emit_consumed(&mut files, req, tr, validate)?;
        package::emit_lib(&mut files, &packages, has_clients, &f, req.role)?;
        Ok(files)
    }
}

/// A client tree under `src/clients/<service>` for each consumed service of a server.
fn emit_consumed(
    files: &mut FileSet,
    req: &GenRequest<'_>,
    tr: Transports,
    validate: bool,
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
        let tree = Tree::new(req.ir, &root, &dir, vec![*sid], false).with_validate(validate);
        models::emit(files, &tree)?;
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
