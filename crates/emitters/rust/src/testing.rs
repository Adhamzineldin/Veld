//! The test handler (`spec/rules.md` section 12.4): with the target option `testing = true`, a
//! second entry point, in a file of its own, for each service with endpoint rules.
//!
//! It runs the pipeline of the production handler: the same step functions, with loaders that read
//! the facts from the request, a clock that reads the request and a principal that replaces
//! authentication. Nothing in the production module or in `src/veld` reads a test header.

use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_ir::{Constraints, Service, ServiceFact};

use crate::block;
use crate::endpoints::{self, Kind};
use crate::models::with_imports;
use crate::server::{mode_in_runtime, needs, service_mod, service_type};
use crate::tree::Tree;
use crate::types::{Ctx, rust_type, schema_ref};
use crate::util::{add_file, lit, method, module, snake};

/// The module of the test handler of a service.
#[must_use]
pub fn test_mod(svc: &Service) -> String {
    format!("{}_test_server", module(&svc.name))
}

/// The warning at the top of the file.
const WARNING: &str = "//!
//! THE VELD TEST HANDLER. NEVER MOUNT IT IN PRODUCTION.
//!
//! It lets whoever sends a request choose the facts, the principal, the clock and the result of
//! that request (`Veld-Test-*` headers), and it skips authentication and middleware. It exists so
//! `veld test --live` can run the examples of a contract against this server. Creating it reports
//! that once.";

/// Writes `<pkg>/<service>_test_server.rs` for every service with endpoint rules, when the option
/// is on. Returns the services that got one.
///
/// # Errors
/// A path problem.
pub fn emit(
    files: &mut FileSet,
    tree: &Tree<'_>,
    testing: bool,
) -> Result<Vec<veld_ir::ServiceId>, Vec<GenDiagnostic>> {
    let mut done = Vec::new();
    if !testing {
        return Ok(done);
    }
    for sid in &tree.services {
        let svc = tree.ir.service(*sid);
        let blocks = block::has_block(svc) || crate::conditional::svc_idempotent(svc);
        if !blocks && svc.subscriptions.is_empty() {
            continue;
        }
        let here = format!(
            "{}::{}::{}",
            tree.root,
            tree.pkg_mod(svc.package),
            test_mod(svc)
        );
        let ctx = Ctx { tree, here: &here };
        let mut w = CodeWriter::with_unit("    ");
        w.line(format!(
            "//! The test handler of the `{}` service (`spec/rules.md` section 12.4).",
            svc.name
        ));
        w.line(WARNING).blank();
        if blocks {
            write_file(&mut w, &ctx, svc);
        } else {
            write_events_only(&mut w, &ctx, svc);
        }
        let path = format!(
            "{}/{}/{}.rs",
            tree.dir,
            tree.pkg_mod(svc.package),
            test_mod(svc)
        );
        add_file(files, &path, with_imports(&w.finish()))?;
        done.push(*sid);
    }
    Ok(done)
}

fn write_file(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service) {
    let prod = format!(
        "{}::{}::{}",
        ctx.tree.root,
        ctx.tree.pkg_mod(svc.package),
        service_mod(svc)
    );
    let name = service_type(svc);
    let steps: Vec<String> = svc
        .endpoints
        .iter()
        .filter(|e| block::ep_has_block(e))
        .map(block::step6_name)
        .collect();
    let mut imports = vec![
        format!("{name}Dispatch"),
        format!("{name}Hooks"),
        format!("{name}Service"),
    ];
    if !svc.facts.is_empty() {
        imports.push(block::loaders_name(svc));
    }
    if !needs(svc).any() {
        imports.push("NoHooks".to_string());
    }
    imports.extend(steps);
    w.line(format!("use {prod}::{{{}}};", imports.join(", ")));
    w.line("use crate::veld::server::{Body, Endpoints, Halt, Handler, Principal, Prepared, RequestContext, Response};");
    w.blank();
    helpers(w, !svc.facts.is_empty());
    loaders(w, ctx, svc);
    test_hooks(w, ctx.tree, svc, &name);
    dispatch(w, ctx, svc, &name);
    handler(w, ctx.tree, svc, &name);
    probe(w, ctx, svc, &name);
    if !svc.subscriptions.is_empty() {
        event_endpoint(w, ctx, svc);
    }
}

/// The test handler of a service that subscribes and has no endpoint rules: it takes no
/// application and answers the probe and `POST /__veld/test/event` (`spec/events.md` section 10.9).
fn write_events_only(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service) {
    probe_body(w, svc, &topics(ctx.tree.ir, svc));
    w.line("/// The routes of the test handler: `GET /__veld/test` and `POST /__veld/test/event`.");
    w.open("pub fn router() -> ::axum::Router");
    w.line("::axum::Router::new().route(\"/__veld/test\", ::axum::routing::get(|| async { ([(::axum::http::header::CONTENT_TYPE, \"application/json\")], probe()) })).route(\"/__veld/test/event\", ::axum::routing::post(event))");
    w.close().blank();
    event_endpoint(w, ctx, svc);
}

fn helpers(w: &mut CodeWriter, facts: bool) {
    w.line("/// Standard or url-safe base64, padded or not.");
    w.open("fn b64(text: &str) -> Option<Vec<u8>>");
    w.line("let (mut out, mut acc, mut bits) = (Vec::new(), 0u32, 0u32);");
    w.open("for c in text.bytes()");
    w.open("let v = match c");
    w.line("b'A'..=b'Z' => c - b'A',");
    w.line("b'a'..=b'z' => c - b'a' + 26,");
    w.line("b'0'..=b'9' => c - b'0' + 52,");
    w.line("b'+' | b'-' => 62,");
    w.line("b'/' | b'_' => 63,");
    w.line("b'=' => continue,");
    w.line("_ => return None,");
    w.close_with("};");
    w.line("acc = (acc << 6) | u32::from(v);");
    w.line("bits += 6;");
    w.open("if bits >= 8");
    w.line("bits -= 8;");
    w.line("out.push(u8::try_from((acc >> bits) & 0xff).ok()?);");
    w.close().close();
    w.line("Some(out)");
    w.close().blank();
    w.line("/// The JSON of a header that carries base64url JSON; `None` when it is absent.");
    w.open("fn header_json(ctx: &RequestContext, name: &str) -> Result<Option<serde_json::Value>, veld::Error>");
    w.line("let Some(text) = ctx.header(name) else { return Ok(None) };");
    w.line("let bytes = b64(text).ok_or_else(|| veld::Error::other(format!(\"the header {name} is not base64url\")))?;");
    w.line("serde_json::from_slice(&bytes).map(Some).map_err(|e| veld::Error::other(format!(\"the header {name} is not JSON: {e}\")))");
    w.close().blank();
    if facts {
        w.line("/// A fact of `Veld-Test-Facts`: absent when the object has none or it is `null`.");
        w.open("fn fact<T: serde::de::DeserializeOwned>(ctx: &RequestContext, name: &str, schema: &'static veld::schema::Schema) -> Result<Option<T>, veld::Error>");
        w.line("let Some(serde_json::Value::Object(facts)) = header_json(ctx, \"veld-test-facts\")? else { return Ok(None) };");
        w.open("match facts.get(name)");
        w.line("None | Some(serde_json::Value::Null) => Ok(None),");
        w.line("Some(wire) => serde_json::from_value(schema.from_wire(wire, false)).map(Some).map_err(veld::Error::other),");
        w.close().close().blank();
    }
}

fn fact_schema(ctx: &Ctx<'_, '_>, f: &ServiceFact) -> String {
    schema_ref(ctx, &f.ty, &Constraints::default())
}

fn loaders(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service) {
    if svc.facts.is_empty() {
        return;
    }
    w.line("/// Loaders that read the facts from the request instead of the application.");
    w.line("#[derive(Debug, Clone, Copy)]");
    w.line("pub struct HeaderFacts;").blank();
    w.open(format!("impl {} for HeaderFacts", block::loaders_name(svc)));
    for f in &svc.facts {
        let key = f.key.as_ref().map_or(String::new(), |k| {
            format!("_key: {}, ", rust_type(ctx, k, false))
        });
        w.open(format!(
            "fn {}(&self, {key}ctx: &RequestContext) -> impl std::future::Future<Output = Result<Option<{}>, veld::Error>> + Send",
            method(&f.name),
            rust_type(ctx, &f.ty, false)
        ));
        w.line(format!(
            "std::future::ready(fact(ctx, {}, {}))",
            lit(&f.name),
            fact_schema(ctx, f)
        ));
        w.close().blank();
    }
    w.close().blank();
}

fn dispatch(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service, name: &str) {
    let loaders = !svc.facts.is_empty();
    let inner = if loaders {
        format!("{name}Dispatch<S, H, HeaderFacts>")
    } else {
        format!("{name}Dispatch<S, H>")
    };
    w.line("/// The dispatch of the test handler: the production dispatch, with the test steps in front.");
    w.open(format!("pub struct {name}TestDispatch<S, H>"));
    w.line(format!("inner: {inner},"));
    w.close().blank();
    w.open(format!(
        "impl<S: {name}Service, H: {name}Hooks> Endpoints for {name}TestDispatch<S, H>"
    ));
    delegates(w);
    conditional(w, ctx, svc);
    audited(w, ctx, svc);
    prepare(w, svc);
    trusted(w, ctx, svc);
    call(w, svc);
    w.close().blank();
}

/// The audited rules of a test handler (`@rollout(audit)`, `spec/rules.md` sections 12.4 and 17.1):
/// it answers with `Veld-Test-Audited`, and runs the audited `ensure` statements of the dispatch.
fn audited(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service) {
    if !ctx.tree.rollout {
        return;
    }
    w.open("fn test_audited(&self) -> bool");
    w.line("true");
    w.close().blank();
    if svc.endpoints.iter().any(block::has_audited_ensure) {
        w.open("fn ensure_audited(&self, prepared: &Prepared, result: &serde_json::Value) -> Vec<veld::rules::Violation>");
        w.line("self.inner.ensure_audited(prepared, result)");
        w.close().blank();
    }
}

/// `@version` and `@idempotent` in the test handler: what each endpoint owes is the production
/// dispatch's; the store is a memory store of the request, filled from `Veld-Test-Idempotency`, and
/// the caller text is the one of section 11.10 (`spec/rules.md`).
fn conditional(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service) {
    let any = svc.endpoints.iter().any(|e| {
        crate::conditional::serves_version(ctx.tree.ir, e) || crate::conditional::is_idempotent(e)
    });
    if !any {
        return;
    }
    w.open("fn conditional(&self, endpoint: usize) -> veld::server::Conditional");
    w.line("self.inner.conditional(endpoint)");
    w.close().blank();
    if !crate::conditional::svc_idempotent(svc) {
        return;
    }
    w.open("fn idem_caller(&self, ctx: &RequestContext) -> String");
    w.line("match header_json(ctx, \"veld-test-principal\") {");
    w.line("    Ok(Some(principal)) => veld::canonical::canonical(&principal),");
    w.line("    Ok(None) | Err(_) => \"caller\".to_string(),");
    w.line("}");
    w.close().blank();
    w.open("fn idem_begin<'a>(&'a self, endpoint: usize, scope: &'a str, key: &'a str, fingerprint: &'a str, ctx: &'a RequestContext) -> impl std::future::Future<Output = Result<veld::idempotency::Begin, veld::Error>> + Send + 'a");
    w.line("let etag = self.inner.conditional(endpoint).etag;");
    w.open("let begun = match self.inner.endpoints().get(endpoint)");
    w.line("Some(ep) => veld::idempotency::seeded_begin(ep, etag, ctx.header(\"veld-test-idempotency\"), (scope, key, fingerprint)),");
    w.line("None => Err(veld::Error::other(\"not an endpoint\")),");
    w.close_with("};");
    w.line("std::future::ready(begun)");
    w.close().blank();
    w.open("fn idem_complete<'a>(&'a self, _scope: &'a str, _key: &'a str, _answer: veld::idempotency::StoredAnswer) -> impl std::future::Future<Output = Result<(), veld::Error>> + Send + 'a");
    w.line("std::future::ready(Ok(()))");
    w.close().blank();
    w.open("fn idem_release<'a>(&'a self, _scope: &'a str, _key: &'a str) -> impl std::future::Future<Output = Result<(), veld::Error>> + Send + 'a");
    w.line("std::future::ready(Ok(()))");
    w.close().blank();
}

fn delegates(w: &mut CodeWriter) {
    w.open("fn endpoints(&self) -> &'static [veld::spec::EndpointSpec]");
    w.line("self.inner.endpoints()");
    w.close().blank();
    w.open("fn std_sigs(&self) -> veld::spec::StdSigs");
    w.line("self.inner.std_sigs()");
    w.close().blank();
    w.open("fn validate_requests(&self) -> bool");
    w.line("self.inner.validate_requests()");
    w.close().blank();
    w.open("fn validate_responses(&self) -> bool");
    w.line("self.inner.validate_responses()");
    w.close().blank();
    w.open("fn rules_output(&self) -> veld::rules::RuleOutput");
    w.line("self.inner.rules_output()");
    w.close().blank();
    w.open("fn on_rule_violation(&self, event: &veld::rules::RuleEvent)");
    w.line("self.inner.on_rule_violation(event);");
    w.close().blank();
    w.open("fn reaches_rules(&self) -> bool");
    w.line("self.inner.reaches_rules()");
    w.close().blank();
    w.open("fn digest(&self) -> Option<&'static str>");
    w.line("self.inner.digest()");
    w.close().blank();
    w.open("fn on_error(&self, err: &veld::Error, ctx: &RequestContext)");
    w.line("self.inner.on_error(err, ctx);");
    w.close().blank();
    w.open("fn ensure(&self, prepared: &Prepared, result: &serde_json::Value) -> Vec<veld::rules::Violation>");
    w.line("self.inner.ensure(prepared, result)");
    w.close().blank();
}

/// Step 6 with the facts, the clock and the dry run of the request.
fn prepare(w: &mut CodeWriter, svc: &Service) {
    let loaders = !svc.facts.is_empty();
    let pass = if loaders { "&HeaderFacts, " } else { "" };
    w.open("async fn prepare(&self, endpoint: usize, args: &mut serde_json::Map<String, serde_json::Value>, ctx: &RequestContext) -> Result<Prepared, Halt>");
    w.line("let enforce = self.inner.hooks.validate_requests();");
    w.open("let now = match ctx.header(\"veld-test-now\")");
    w.line("Some(text) => veld::rules::instant::parse_datetime(text).ok_or_else(|| Halt::Error(veld::Error::other(\"the header Veld-Test-Now is not an RFC 3339 time\")))?,");
    // A service without endpoint rules has no clock hook.
    w.line(if block::has_block(svc) {
        "None => veld::rules::clock_ms(self.inner.hooks.clock(ctx)),"
    } else {
        "None => veld::rules::clock_ms(std::time::SystemTime::now()),"
    });
    w.close_with("};");
    if block::has_block(svc) {
        w.open("let prepared = match endpoint");
        for (i, ep) in svc.endpoints.iter().enumerate() {
            if block::ep_has_block(ep) {
                w.line(format!(
                    "{i} => {}({pass}now, enforce, args, ctx).await?,",
                    block::step6_name(ep)
                ));
            }
        }
        w.line("_ => Prepared::none(),");
        w.close_with("};");
    } else {
        w.line("let _ = (endpoint, enforce, now, &args);");
        w.line("let prepared = Prepared::none();");
    }
    w.open("if ctx.header(\"veld-test-dry-run\") == Some(\"1\")");
    w.line("return Err(Halt::Early(Response { status: 204, headers: vec![(\"veld-test-passed\".to_string(), \"1\".to_string())], body: Body::Empty }));");
    w.close();
    w.line("Ok(prepared)");
    w.close().blank();
}

fn trusted(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service) {
    w.open("fn trusted_principal(&self, ctx: &RequestContext) -> Option<Principal>");
    match crate::server::typed_principal(ctx, svc) {
        // The hooks of this service return the model: the header is its wire JSON, decoded the
        // way a fact is. Without the header there is no caller, and a statement that reads it
        // answers `500`.
        Some(ty) => {
            let schema = svc
                .principal
                .as_ref()
                .map(|p| schema_ref(ctx, &veld_ir::Type::Decl(p.decl), &Constraints::default()))
                .unwrap_or_default();
            w.line("let wire = header_json(ctx, \"veld-test-principal\").ok().flatten()?;");
            w.line(format!(
                "let principal: {ty} = serde_json::from_value(vs::Schema::from_wire({schema}, &wire, false)).ok()?;"
            ));
            w.line("Some(Principal::new(principal))");
        }
        None => {
            w.line("Some(Principal::new(header_json(ctx, \"veld-test-principal\").ok().flatten().unwrap_or(serde_json::Value::Null)))");
        }
    }
    w.close().blank();
}

fn call(w: &mut CodeWriter, svc: &Service) {
    let args = "args: serde_json::Map<String, serde_json::Value>";
    w.open(format!("async fn call(&self, endpoint: usize, {args}, ctx: &RequestContext) -> Result<serde_json::Value, veld::Error>"));
    w.open("if let Some(wire) = header_json(ctx, \"veld-test-result\")?");
    w.line("let schema = self.inner.endpoints().get(endpoint).and_then(|e| e.success);");
    w.line("return Ok(schema.map_or(serde_json::Value::Null, |s| s.from_wire(&wire, false)));");
    w.close();
    w.line("self.inner.call(endpoint, args, ctx).await");
    w.close().blank();
    if svc.endpoints.iter().any(|e| endpoints::kind(e) == Kind::Ws) {
        w.open(format!("async fn call_ws(&self, endpoint: usize, {args}, ctx: &RequestContext, io: veld::server::WsIo) -> Result<(), veld::Error>"));
        w.line("self.inner.call_ws(endpoint, args, ctx, io).await");
        w.close().blank();
    }
    if svc
        .endpoints
        .iter()
        .any(|e| endpoints::kind(e) == Kind::Sse)
    {
        w.open(format!("async fn call_sse(&self, endpoint: usize, {args}, ctx: &RequestContext) -> Result<veld::stream::EventStream<serde_json::Value>, veld::Error>"));
        w.line("self.inner.call_sse(endpoint, args, ctx).await");
        w.close().blank();
    }
}

fn handler(w: &mut CodeWriter, tree: &Tree<'_>, svc: &Service, name: &str) {
    let loaders = !svc.facts.is_empty();
    let n = needs(svc);
    // The output mode is a member of the test handler's own hooks (`spec/runtime-config.md` 4).
    let member = if mode_in_runtime(tree) {
        ", output"
    } else {
        ""
    };
    let bound = if mode_in_runtime(tree) {
        format!("{name}TestHooks")
    } else {
        format!("{name}Hooks")
    };
    let inner = if loaders {
        format!("{name}Dispatch {{ service, hooks, loaders: HeaderFacts{member} }}")
    } else {
        format!("{name}Dispatch {{ service, hooks{member} }}")
    };
    w.line(
        "/// Builds the test handler of the service. Creating it says so once: never mount it in",
    );
    w.line("/// production. It takes no runtime config: it calls no consumed service.");
    w.line("#[derive(Debug, Clone, Copy)]");
    w.line(format!("pub struct {name}TestHandler;")).blank();
    w.open(format!("impl {name}TestHandler"));
    w.line("/// The test handler for `service`, with `hooks` (their authentication and middleware are not called).");
    w.open(format!(
        "pub fn create<S: {name}Service, H: {bound}>(service: S, hooks: H) -> Handler<{name}TestDispatch<S, H>>"
    ));
    if mode_in_runtime(tree) {
        w.line("let output = hooks.rules_output();");
    }
    w.line(format!(
        "eprintln!(\"veld: the test handler of `{}` is running: anyone can choose the facts, the principal and the clock of a request. Never mount it in production.\");",
        svc.name
    ));
    w.line(format!(
        "Handler::new({name}TestDispatch {{ inner: {inner} }})"
    ));
    w.close();
    if !n.any() {
        w.blank();
        w.line("/// The test handler for `service` with the default hooks.");
        w.open(format!(
            "pub fn plain<S: {name}Service>(service: S) -> Handler<{name}TestDispatch<S, NoHooks>>"
        ));
        w.line("Self::create(service, NoHooks)");
        w.close();
    }
    w.close().blank();
}

/// The hooks trait of the test handler: the production hooks and the output mode.
fn test_hooks(w: &mut CodeWriter, tree: &Tree<'_>, svc: &Service, name: &str) {
    if !mode_in_runtime(tree) {
        return;
    }
    w.line(
        "/// The hooks of the test handler: the hooks of the service, and the rule output mode the",
    );
    w.line("/// test handler runs in (`spec/runtime-config.md` section 4). Implement it with an empty body");
    w.line("/// for the default, `Strict`.");
    w.open(format!("pub trait {name}TestHooks: {name}Hooks"));
    w.line("/// `Strict` (the default) answers `500 Internal` for a violated invariant of a result, `Audit`");
    w.line("/// sends it and reports, `Off` does not check.");
    w.open("fn rules_output(&self) -> veld::rules::RuleOutput");
    w.line("veld::rules::RuleOutput::Strict");
    w.close();
    w.close().blank();
    if !needs(svc).any() {
        w.line(format!("impl {name}TestHooks for NoHooks {{}}"))
            .blank();
    }
}

/// The topics the service subscribes to, sorted.
fn topics(ctx_ir: &veld_ir::Ir, svc: &Service) -> Vec<String> {
    let mut out: Vec<String> = svc
        .subscriptions
        .iter()
        .map(|s| ctx_ir.channel(s.channel).topic.clone())
        .collect();
    out.sort();
    out.dedup();
    out
}

fn probe_body(w: &mut CodeWriter, svc: &Service, topics: &[String]) {
    let digest = svc
        .rules_digest
        .as_ref()
        .map_or("null".to_string(), |d| format!("\"{d}\""));
    w.line(
        "/// The body of `GET /__veld/test`: it tells a test handler from a production server and",
    );
    w.line(if svc.subscriptions.is_empty() {
        "/// holds the rules digests of its service."
    } else {
        "/// holds the rules digests of its service, and the topics it subscribes to."
    });
    w.open("pub fn probe() -> String");
    let subscribes = if svc.subscriptions.is_empty() {
        String::new()
    } else {
        let list: Vec<String> = topics.iter().map(|x| format!("\"{x}\"")).collect();
        format!(",\"subscribes\":{{\"{}\":[{}]}}", svc.name, list.join(","))
    };
    w.line(format!(
        "{}.to_string()",
        lit(&format!(
            "{{\"veldTestHandler\":1,\"digests\":{{\"{}\":{digest}}}{subscribes}}}",
            svc.name
        ))
    ));
    w.close().blank();
}

fn probe(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service, name: &str) {
    probe_body(w, svc, &topics(ctx.tree.ir, svc));
    w.line("/// The routes of the test handler: the endpoints and `GET /__veld/test`.");
    w.open(format!(
        "pub fn router<S: {name}Service, H: {name}Hooks>(handler: Handler<{name}TestDispatch<S, H>>) -> ::axum::Router"
    ));
    let event = if svc.subscriptions.is_empty() {
        ""
    } else {
        ".route(\"/__veld/test/event\", ::axum::routing::post(event))"
    };
    w.line(format!("crate::adapters::axum::router(handler).route(\"/__veld/test\", ::axum::routing::get(|| async {{ ([(::axum::http::header::CONTENT_TYPE, \"application/json\")], probe()) }})){event}"));
    w.close().blank();
}

/// `POST /__veld/test/event` (`spec/events.md` section 10.9): the service's own subscriber, over a
/// fresh memory transport and a fresh memory inbox, with a `sleep` that does not wait and handlers
/// that only count. The application's handlers are not called.
fn event_endpoint(w: &mut CodeWriter, ctx: &Ctx<'_, '_>, svc: &Service) {
    let name = crate::util::type_name(&svc.name);
    let events = format!(
        "{}::{}::{}",
        ctx.tree.root,
        ctx.tree.pkg_mod(svc.package),
        crate::events::events_mod(svc)
    );
    let handlers: Vec<&veld_ir::Handler> = svc
        .subscriptions
        .iter()
        .flat_map(|s| s.handlers.iter())
        .collect();
    w.line(
        "/// What the test handler answers to an event: the JSON of `spec/events.md` section 10.9.",
    );
    w.line("#[derive(Debug, Clone, PartialEq)]");
    w.open("pub struct EventOutcome");
    w.line("/// `handled`, `ignored` or `rejected`.");
    w.line("pub outcome: &'static str,");
    w.line("/// `malformed`, `shape` or `rule` for a rejected message.");
    w.line("pub reason: Option<&'static str>,");
    w.line("/// How often each `Service.handler` ran.");
    w.line("pub called: std::collections::BTreeMap<String, u64>,");
    w.line("/// The violations of a rejected message: `rule`, `path`, `fields`, `message`.");
    w.line("pub violations: Vec<serde_json::Value>,");
    w.line("/// The copies published to `<topic>.dlq`.");
    w.line("pub dead_letters: u64,");
    w.close().blank();
    w.open("impl EventOutcome");
    w.line("/// The body of the answer.");
    w.open("pub fn to_json(&self) -> serde_json::Value");
    w.line("serde_json::json!({ \"outcome\": self.outcome, \"reason\": self.reason, \"called\": self.called, \"violations\": self.violations, \"deadLetters\": self.dead_letters })");
    w.close();
    w.close().blank();
    w.line("/// The handlers of the test handler: each one counts its calls and returns.");
    w.line("struct Counting(std::sync::Arc<std::sync::Mutex<std::collections::BTreeMap<String, u64>>>);");
    w.blank();
    w.open("impl Counting");
    w.open("fn count(&self, handler: &str)");
    w.line("let mut calls = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);");
    w.line("*calls.entry(handler.to_string()).or_insert(0) += 1;");
    w.close();
    w.close().blank();
    w.open(format!("impl {events}::{name}EventHandlers for Counting"));
    for (i, h) in handlers.iter().enumerate() {
        if i > 0 {
            w.blank();
        }
        w.open(format!(
            "fn {}(&self, _event: {}, _meta: veld::events::EventMeta) -> impl std::future::Future<Output = Result<(), veld::Error>> + Send",
            snake(&h.name),
            ctx.tree.type_path(h.event, ctx.here)
        ));
        w.line(format!(
            "self.count({});",
            lit(&format!("{}.{}", svc.name, h.name))
        ));
        w.line("std::future::ready(Ok(()))");
        w.close();
    }
    w.close().blank();
    w.line("/// Every handler of the service, as `Service.handler`.");
    w.open_with("const HANDLERS: &[&str] = &[", "");
    for h in &handlers {
        w.line(format!("{},", lit(&format!("{}.{}", svc.name, h.name))));
    }
    w.close_with("];");
    w.blank();
    w.line("/// Delivers `bytes` to the subscriber of this service `deliveries` times (1 or 2), over a fresh");
    w.line("/// memory transport with a fresh inbox, and says what happened.");
    w.line("///");
    w.line("/// # Errors");
    w.line("/// The transport's own error.");
    w.open("pub async fn deliver_event(topic: &str, key: Option<&str>, deliveries: u8, bytes: &[u8]) -> Result<EventOutcome, veld::Error>");
    w.line("use std::sync::{Arc, Mutex, PoisonError};");
    w.line("let transport = Arc::new(veld::events::MemoryTransport::new());");
    w.line("let calls = Arc::new(Mutex::new(std::collections::BTreeMap::new()));");
    w.line("let rejected: Arc<Mutex<Option<veld::events::EventRejected>>> = Arc::new(Mutex::new(None));");
    w.line("let copies = Arc::new(std::sync::atomic::AtomicU64::new(0));");
    w.line("for handler in HANDLERS {");
    w.line("    calls.lock().unwrap_or_else(PoisonError::into_inner).insert((*handler).to_string(), 0u64);");
    w.line("}");
    w.line("let seen = Arc::clone(&rejected);");
    w.line("let options = veld::events::ConsumerOptions::new()");
    w.line("    .inbox(Arc::new(veld::events::MemoryInbox::new()))");
    w.line("    .sleep(|_| Box::pin(std::future::ready(())))");
    w.line("    .on_event_rejected(move |e| { let mut first = seen.lock().unwrap_or_else(PoisonError::into_inner); if first.is_none() { *first = Some(e.clone()); } });");
    w.line(format!(
        "let handle = {events}::subscribe_{}_events(transport.clone(), Arc::new(Counting(Arc::clone(&calls))), options).await?;",
        snake(&svc.name)
    ));
    w.line("let counted = Arc::clone(&copies);");
    w.line("let dead: veld::events::Deliver = Arc::new(move |_| { counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst); Box::pin(std::future::ready(veld::events::Outcome::Done)) });");
    w.line("use veld::events::Transport;");
    w.line("let listener = transport.subscribe(&format!(\"{topic}.dlq\"), \"veld-test\", dead, veld::events::ChannelOptions::NONE).await?;");
    w.open("for _ in 0..deliveries.clamp(1, 2)");
    w.line("transport.publish(topic, key, bytes, veld::events::ChannelOptions::NONE).await?;");
    w.close();
    w.line("transport.idle().await;");
    w.line("handle.close().await;");
    w.line("listener.close().await;");
    w.line("let called = calls.lock().unwrap_or_else(PoisonError::into_inner).clone();");
    w.line("let first = rejected.lock().unwrap_or_else(PoisonError::into_inner).take();");
    w.line("let (outcome, reason, violations) = match first {");
    w.line("    Some(e) => (\"rejected\", Some(e.reason.as_str()), e.violations.iter().map(|v| serde_json::json!({ \"rule\": v.rule, \"path\": v.path, \"fields\": v.fields, \"message\": v.message })).collect()),");
    w.line("    None if called.values().any(|n| *n > 0) => (\"handled\", None, Vec::new()),");
    w.line("    None => (\"ignored\", None, Vec::new()),");
    w.line("};");
    w.line("Ok(EventOutcome { outcome, reason, called, violations, dead_letters: copies.load(std::sync::atomic::Ordering::SeqCst) })");
    w.close().blank();
    w.line("/// `POST /__veld/test/event`: the body is the message, the headers are `Veld-Test-Topic`,");
    w.line("/// `Veld-Test-Key` and `Veld-Test-Deliveries`. A request without a topic is `400`.");
    w.open("async fn event(headers: ::axum::http::HeaderMap, body: ::axum::body::Bytes) -> ::axum::response::Response");
    w.line("use ::axum::response::IntoResponse;");
    w.line("let text = |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).map(str::to_string);");
    w.line("let Some(topic) = text(\"veld-test-topic\") else { return (::axum::http::StatusCode::BAD_REQUEST, \"Veld-Test-Topic is missing\").into_response() };");
    w.line("let deliveries = text(\"veld-test-deliveries\").and_then(|n| n.parse::<u8>().ok()).unwrap_or(1);");
    w.open(
        "match deliver_event(&topic, text(\"veld-test-key\").as_deref(), deliveries, &body).await",
    );
    w.line("Ok(outcome) => ([(::axum::http::header::CONTENT_TYPE, \"application/json\")], outcome.to_json().to_string()).into_response(),");
    w.line("Err(e) => (::axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),");
    w.close();
    w.close().blank();
}
