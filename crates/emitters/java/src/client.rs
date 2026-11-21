//! `<package>.client`: request records and one `<Service>Client` class per service.

use veld_codegen::naming;
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_ir::{Endpoint, Service};

use crate::spec::{
    RequestNames, emit_requests, endpoint_docs, endpoint_docs_note, is_sse, is_ws, method_name,
    spec_const, write_spec_const, ws_incoming,
};
use crate::tree::{FileCtx, Tree, java_path};
use crate::types::{codec_expr, java_type};
use crate::util::{add_file, doc_comment, end_call, java_str, open_call};

/// Emits the client packages of every package of the tree's services.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<(), Vec<GenDiagnostic>> {
    let names = RequestNames::new(tree.ir, &tree.services);
    for pkg in tree.packages() {
        let package = tree.api_package(pkg, "client");
        for sid in tree
            .services
            .iter()
            .filter(|s| tree.ir.service(**s).package == pkg)
        {
            let svc = tree.ir.service(*sid);
            emit_requests(files, tree, &package, &names, svc)?;
            let class = class_name(svc);
            let mut fc = FileCtx::new(tree, &package, &class);
            let mut w = CodeWriter::new();
            write_client(&mut w, &mut fc, &names, svc, &class);
            add_file(files, &java_path(&package, &class), fc.finish(&w.finish()))?;
        }
    }
    Ok(())
}

fn class_name(svc: &Service) -> String {
    format!("{}Client", naming::pascal(&svc.name))
}

fn write_client(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    class: &str,
) {
    let core = fc.rt("ClientCore");
    let options = fc.rt("ClientOptions");
    let env = format!("VELD_{}_URL", naming::screaming_snake(&svc.name));
    let docs = svc
        .docs
        .clone()
        .unwrap_or_else(|| format!("A client for the `{}` service.", svc.name));
    doc_comment(
        w,
        Some(&format!(
            "{docs}\n\nBlocking. The base URL is the `baseUrl` option, else the environment variable `{env}`, else the contract's `base-url`."
        )),
        &[],
        None,
    );
    w.line("@SuppressWarnings(\"deprecation\")");
    w.open(format!("public final class {class}"));
    for ep in &svc.endpoints {
        write_spec_const(w, fc, ep);
        w.blank();
    }
    w.line(format!("private final {core} core;"));
    w.blank();
    w.line("/** Creates a client with default options. */");
    w.open(format!("public {class}()"));
    w.line(format!("this({options}.defaults());"));
    w.close();
    w.blank();
    w.line("/** Creates a client. */");
    w.open(format!("public {class}({options} options)"));
    write_constructor_body(w, fc, svc, &env);
    w.close();
    for ep in &svc.endpoints {
        w.blank();
        write_method(w, fc, &names.get(svc.id, &ep.name), ep);
    }
    w.close();
}

fn write_constructor_body(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service, env: &str) {
    let core = fc.rt("ClientCore");
    let defaults = fc.rt("ClientDefaults");
    let router = fc.rt("Router");
    let base = svc
        .base_url
        .as_deref()
        .map_or_else(|| "null".to_string(), java_str);
    let validation = match fc.tree.prelude_decl("ValidationError") {
        Some(id) => format!(
            "{}.fromWire({router}.issuesToWire(issues), 422, null)",
            fc.decl(id)
        ),
        None => {
            let untyped = fc.rt("UntypedError");
            format!(
                "new {untyped}(\"about:blank\", \"ValidationError\", {router}.issuesToWire(issues), 422, null)"
            )
        }
    };
    open_call(w, format!("this.core = new {core}"));
    w.line("options,");
    w.line(format!(
        "new {defaults}({}, {base}, issues -> {validation}{}));",
        java_str(env),
        if fc.tree.validate { "" } else { ", false" }
    ));
    end_call(w);
}

fn write_method(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, request: &str, ep: &Endpoint) {
    if is_sse(ep) {
        return write_sse_method(w, fc, request, ep);
    }
    if is_ws(ep) {
        return write_ws_method(w, fc, request, ep);
    }
    let method = method_name(ep);
    let konst = spec_const(ep);
    let (ret, decode) = match &ep.success.ty {
        Some(t) => {
            let ty = java_type(fc, t, true);
            let c = codec_expr(fc, t);
            (ty, Some(c))
        }
        None => ("void".to_string(), None),
    };
    endpoint_docs(w, ep);
    w.open(format!("public {ret} {method}({request} request)"));
    match decode {
        Some(c) => w.line(format!(
            "return {c}.decode(core.call({konst}, request.toWire()));"
        )),
        None => w.line(format!("core.call({konst}, request.toWire());")),
    };
    w.close();
    if ep.params.is_empty() {
        w.blank();
        w.line(format!("/** Calls `{}` without parameters. */", ep.name));
        w.open(format!("public {ret} {method}()"));
        let call = format!("{method}(new {request}())");
        w.line(if decode_is_void(&ret) {
            format!("{call};")
        } else {
            format!("return {call};")
        });
        w.close();
    }
}

fn decode_is_void(ret: &str) -> bool {
    ret == "void"
}

/// A Server-Sent Events method: an eager call that returns a closeable `Stream` of items.
fn write_sse_method(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, request: &str, ep: &Endpoint) {
    let (method, konst) = (method_name(ep), spec_const(ep));
    let (stream, options) = (fc.name("java.util.stream.Stream"), fc.rt("SseOptions"));
    let (item, codec) = match &ep.success.ty {
        Some(t) => (java_type(fc, t, true), codec_expr(fc, t)),
        None => ("Void".to_string(), "null".to_string()),
    };
    endpoint_docs_note(
        w,
        ep,
        "The call is eager: an error response throws from this method.
The returned stream reads events as they arrive; close it to cancel the request.
A final `error` event throws the typed error when it is consumed.
There is no automatic reconnect: pass `SseOptions.resumeAfter(id)` to resume
after the last event id received.",
    );
    w.open(format!(
        "public {stream}<{item}> {method}({request} request, {options} options)"
    ));
    w.line(format!(
        "return core.stream({konst}, request.toWire(), options).map({codec}::decode);"
    ));
    w.close();
    w.blank();
    w.line(format!("/** Calls `{}` without options. */", ep.name));
    w.open(format!(
        "public {stream}<{item}> {method}({request} request)"
    ));
    w.line(format!("return {method}(request, {options}.none());"));
    w.close();
    if ep.params.is_empty() {
        w.blank();
        w.line(format!("/** Calls `{}` without parameters. */", ep.name));
        w.open(format!(
            "public {stream}<{item}> {method}({options} options)"
        ));
        w.line(format!("return {method}(new {request}(), options);"));
        w.close();
        w.blank();
        w.line(format!(
            "/** Calls `{}` without parameters or options. */",
            ep.name
        ));
        w.open(format!("public {stream}<{item}> {method}()"));
        w.line(format!(
            "return {method}(new {request}(), {options}.none());"
        ));
        w.close();
    }
}

/// A `ws` method: connects and returns the channel.
fn write_ws_method(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, request: &str, ep: &Endpoint) {
    let (method, konst) = (method_name(ep), spec_const(ep));
    let channel = fc.rt("WsChannel");
    let side = |fc: &mut FileCtx<'_, '_>, ty: Option<&veld_ir::Type>| match ty {
        Some(t) => (java_type(fc, t, true), codec_expr(fc, t)),
        None => ("Void".to_string(), "null".to_string()),
    };
    let (incoming, in_codec) = side(fc, ep.success.ty.as_ref());
    let (outgoing, out_codec) = side(fc, ws_incoming(ep).map(|p| &p.ty));
    endpoint_docs_note(
        w,
        ep,
        "Opens the WebSocket; the channel is connected when this returns.
Auth is sent in the query string (`access_token` or `api_key`), as are header
parameters.",
    );
    w.open(format!(
        "public {channel}<{incoming}, {outgoing}> {method}({request} request)"
    ));
    w.line(format!(
        "return core.connect({konst}, request.toWire(), {in_codec}, {out_codec});"
    ));
    w.close();
    if ep.params.iter().all(|p| p.stream) {
        w.blank();
        w.line(format!("/** Opens `{}` without parameters. */", ep.name));
        w.open(format!(
            "public {channel}<{incoming}, {outgoing}> {method}()"
        ));
        w.line(format!("return {method}(new {request}());"));
        w.close();
    }
}
