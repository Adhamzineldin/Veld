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
use crate::server::{needs, service_mod, service_type};
use crate::tree::Tree;
use crate::types::{Ctx, rust_type, schema_ref};
use crate::util::{add_file, lit, method, module};

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
        if !block::has_block(svc) && !crate::conditional::svc_idempotent(svc) {
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
        write_file(&mut w, &ctx, svc);
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
    dispatch(w, ctx, svc, &name);
    handler(w, svc, &name);
    probe(w, svc, &name);
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
        w.line("let _ = (endpoint, enforce, &args);");
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

fn handler(w: &mut CodeWriter, svc: &Service, name: &str) {
    let loaders = !svc.facts.is_empty();
    let n = needs(svc);
    let inner = if loaders {
        format!("{name}Dispatch {{ service, hooks, loaders: HeaderFacts }}")
    } else {
        format!("{name}Dispatch {{ service, hooks }}")
    };
    w.line(
        "/// Builds the test handler of the service. Creating it says so once: never mount it in",
    );
    w.line("/// production.");
    w.line("#[derive(Debug, Clone, Copy)]");
    w.line(format!("pub struct {name}TestHandler;")).blank();
    w.open(format!("impl {name}TestHandler"));
    w.line("/// The test handler for `service`, with `hooks` (their authentication and middleware are not called).");
    w.open(format!(
        "pub fn create<S: {name}Service, H: {name}Hooks>(service: S, hooks: H) -> Handler<{name}TestDispatch<S, H>>"
    ));
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

fn probe(w: &mut CodeWriter, svc: &Service, name: &str) {
    let digest = svc
        .rules_digest
        .as_ref()
        .map_or("null".to_string(), |d| format!("\"{d}\""));
    w.line(
        "/// The body of `GET /__veld/test`: it tells a test handler from a production server and",
    );
    w.line("/// holds the rules digests of its service.");
    w.open("pub fn probe() -> String");
    w.line(format!(
        "{}.to_string()",
        lit(&format!(
            "{{\"veldTestHandler\":1,\"digests\":{{\"{}\":{digest}}}}}",
            svc.name
        ))
    ));
    w.close().blank();
    w.line("/// The routes of the test handler: the endpoints and `GET /__veld/test`.");
    w.open(format!(
        "pub fn router<S: {name}Service, H: {name}Hooks>(handler: Handler<{name}TestDispatch<S, H>>) -> ::axum::Router"
    ));
    w.line("crate::adapters::axum::router(handler).route(\"/__veld/test\", ::axum::routing::get(|| async { ([(::axum::http::header::CONTENT_TYPE, \"application/json\")], probe()) }))");
    w.close().blank();
}
