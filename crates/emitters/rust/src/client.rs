//! The client of a service: generic over a `Transport`.

use veld_codegen::{CodeWriter, FileSet, GenDiagnostic, naming};
use veld_ir::{Endpoint, Service};

use crate::endpoints::{self, Kind, endpoint_statics, request_name, request_struct, std_sigs};
use crate::models::with_imports;
use crate::tree::Tree;
use crate::types::{Ctx, rust_type};
use crate::util::{add_file, bare, lit, method, module, type_name};

/// Which transport adapters the generated crate contains (and so which get a convenience
/// constructor).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Transports {
    pub reqwest: bool,
    pub ureq: bool,
}

/// The module of a service's client.
#[must_use]
pub fn client_mod(svc: &Service) -> String {
    format!("{}_client", module(&svc.name))
}

/// Emits `<pkg>/<service>_client.rs` for every service of the tree.
pub fn emit(
    files: &mut FileSet,
    tree: &Tree<'_>,
    transports: Transports,
) -> Result<(), Vec<GenDiagnostic>> {
    for sid in &tree.services {
        let svc = tree.ir.service(*sid);
        let here = format!(
            "{}::{}::{}",
            tree.root,
            tree.pkg_mod(svc.package),
            client_mod(svc)
        );
        let ctx = Ctx { tree, here: &here };
        let mut w = CodeWriter::with_unit("    ");
        service_file(&mut w, &ctx, svc, transports);
        let path = format!(
            "{}/{}/{}.rs",
            tree.dir,
            tree.pkg_mod(svc.package),
            client_mod(svc)
        );
        add_file(files, &path, with_imports(&w.finish()))?;
    }
    Ok(())
}

fn service_file(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service, transports: Transports) {
    w.line(format!("//! The client of the `{}` service.", svc.name))
        .blank();
    for ep in &svc.endpoints {
        request_struct(w, ctx, ep);
    }
    endpoint_statics(w, ctx, svc);
    std_sigs(w, ctx.tree);
    let name = format!("{}Client", type_name(&svc.name));
    client_type(w, svc, &name);
    w.open(format!("impl<T: veld::client::Transport> {name}<T>"));
    constructor(w, svc, ctx.tree.validate);
    for (i, ep) in svc.endpoints.iter().enumerate() {
        match endpoints::kind(ep) {
            Kind::Http => http_method(w, ctx, ep, i),
            Kind::Sse => sse_methods(w, ctx, ep, i),
            Kind::Ws => ws_methods(w, ctx, ep, i),
        }
    }
    w.close().blank();
    transport_constructors(w, &name, transports);
}

fn client_type(w: &mut CodeWriter, svc: &Service, name: &str) {
    crate::util::doc(w, svc.docs.as_deref());
    w.line("///").line("/// Generic over the HTTP stack: pass a `veld::client::Transport`, or use the constructor of an");
    w.line("/// adapter that this crate includes.");
    w.open(format!("pub struct {name}<T: veld::client::Transport>"));
    w.line("core: veld::client::Core<T>,");
    w.close().blank();
    w.open(format!(
        "impl<T: veld::client::Transport> Clone for {name}<T>"
    ));
    w.open("fn clone(&self) -> Self");
    w.line("Self { core: self.core.clone() }");
    w.close().close().blank();
    w.open(format!(
        "impl<T: veld::client::Transport> std::fmt::Debug for {name}<T>"
    ));
    w.open("fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result");
    w.line(format!("write!(f, \"{name}({{:?}})\", self.core)"));
    w.close().close().blank();
}

fn constructor(w: &mut CodeWriter, svc: &Service, validate: bool) {
    let env = format!("VELD_{}_URL", naming::screaming_snake(&svc.name));
    let default = svc
        .base_url
        .as_deref()
        .map_or("None".to_string(), |u| format!("Some({})", lit(u)));
    w.line("/// A client over `transport`.");
    w.line("///");
    w.line(format!(
        "/// The base URL is `options.base_url`, else the environment variable `{env}`, else the"
    ));
    w.line("/// contract's `base-url`.");
    w.line("///");
    w.line("/// # Errors");
    w.line("/// `veld::Error::Transport` when no base URL is set.");
    w.open("pub fn new(transport: T, options: veld::client::ClientOptions) -> Result<Self, veld::Error>");
    w.line(format!("Ok(Self {{ core: veld::client::Core::new(transport, {}, {default}, STD, {validate}, options)? }})", lit(&env)));
    w.close().blank();
}

fn http_method(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, ep: &Endpoint, index: usize) {
    endpoints::method_docs(w, ep);
    let params = endpoints::request_params(ep).count() > 0;
    let arg = if params {
        format!(", request: {}", request_name(ep))
    } else {
        String::new()
    };
    let req = if params { "&request" } else { "&()" };
    let name = method(&ep.name);
    match &ep.success.ty {
        Some(t) => {
            w.open(format!(
                "pub async fn {name}(&self{arg}) -> Result<{}, veld::Error>",
                rust_type(ctx, t, false)
            ));
            w.line(format!("self.core.call(&ENDPOINTS[{index}], {req}).await"));
        }
        None => {
            w.open(format!(
                "pub async fn {name}(&self{arg}) -> Result<(), veld::Error>"
            ));
            w.line(format!(
                "self.core.call_unit(&ENDPOINTS[{index}], {req}).await"
            ));
        }
    }
    w.close().blank();
}

fn sse_methods(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, ep: &Endpoint, index: usize) {
    let params = endpoints::request_params(ep).count() > 0;
    let arg = if params {
        format!(", request: {}", request_name(ep))
    } else {
        String::new()
    };
    let pass = if params { "request, " } else { "" };
    let req = if params { "&request" } else { "&()" };
    let item = ep
        .success
        .ty
        .as_ref()
        .map_or("()".to_string(), |t| rust_type(ctx, t, false));
    let ret = format!("Result<veld::stream::BoxStream<Result<{item}, veld::Error>>, veld::Error>");
    let name = method(&ep.name);
    endpoints::method_docs(w, ep);
    w.line("///").line("/// The HTTP call is eager: a failed response is the error of this method. Errors after the");
    w.line("/// first event are the last item of the stream.");
    w.open(format!("pub async fn {name}(&self{arg}) -> {ret}"));
    w.line(format!(
        "self.{}_with({pass}veld::client::SseOptions::default()).await",
        bare(&name)
    ));
    w.close().blank();
    w.line(format!(
        "/// Like [`{}`](Self::{}), with options: resume after an event id, report ids, reconnect.",
        bare(&name),
        bare(&name)
    ));
    w.open(format!(
        "pub async fn {}_with(&self{arg}, options: veld::client::SseOptions) -> {ret}",
        bare(&name)
    ));
    w.line(format!(
        "self.core.stream(&ENDPOINTS[{index}], {req}, options).await"
    ));
    w.close().blank();
}

fn transport_constructors(w: &mut CodeWriter, name: &str, t: Transports) {
    if t.reqwest {
        w.open(format!(
            "impl {name}<veld::transport_reqwest::ReqwestTransport>"
        ));
        w.line("/// A client on the default `reqwest` transport.");
        w.line("///");
        w.line("/// # Errors");
        w.line("/// `veld::Error::Transport` when no base URL is set.");
        w.open("pub fn reqwest(options: veld::client::ClientOptions) -> Result<Self, veld::Error>");
        w.line("Self::new(veld::transport_reqwest::ReqwestTransport::new(), options)");
        w.close().close().blank();
    }
    if t.ureq {
        w.open(format!("impl {name}<veld::transport_ureq::UreqTransport>"));
        w.line("/// A client on the default (blocking) `ureq` transport. Drive its futures with `veld::block_on`.");
        w.line("///");
        w.line("/// # Errors");
        w.line("/// `veld::Error::Transport` when no base URL is set.");
        w.open("pub fn ureq(options: veld::client::ClientOptions) -> Result<Self, veld::Error>");
        w.line("Self::new(veld::transport_ureq::UreqTransport::new(), options)");
        w.close().close().blank();
    }
}

fn ws_methods(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, ep: &Endpoint, index: usize) {
    let params = endpoints::request_params(ep).count() > 0;
    let arg = if params {
        format!(", request: {}", request_name(ep))
    } else {
        String::new()
    };
    let pass = if params { "request, " } else { "" };
    let req = if params { "&request" } else { "&()" };
    // The channel's `In` is what the server sends, its `Out` what the client sends.
    let (sent, received) = endpoints::ws_types(ctx, ep);
    let ret = format!("Result<veld::client::WsChannel<T, {received}, {sent}>, veld::Error>");
    let name = method(&ep.name);
    endpoints::method_docs(w, ep);
    w.line("///").line(
        "/// The channel is connected when this returns. Receive with `recv`, send with `send`.",
    );
    w.open(format!("pub async fn {name}(&self{arg}) -> {ret}"));
    w.line(format!(
        "self.{}_with({pass}veld::client::WsOptions::default()).await",
        bare(&name)
    ));
    w.close().blank();
    w.line(format!(
        "/// Like [`{}`](Self::{}), with options (reconnect).",
        bare(&name),
        bare(&name)
    ));
    w.open(format!(
        "pub async fn {}_with(&self{arg}, options: veld::client::WsOptions) -> {ret}",
        bare(&name)
    ));
    w.line(format!(
        "self.core.ws(&ENDPOINTS[{index}], {req}, options).await"
    ));
    w.close().blank();
}
