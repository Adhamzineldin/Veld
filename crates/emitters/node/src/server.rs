//! `server/<service>.ts`: request types, the `<Service>Service` interface, hooks and `create<Service>Handler`.

use veld_codegen::naming;
use veld_codegen::runtime_config::{RULES_OUTPUT, RuntimeModel};
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_emit_typescript::client::{RequestNames, write_request_type, write_spec_fields};
use veld_emit_typescript::derived;
use veld_emit_typescript::tree::{FileCtx, Tree};
use veld_emit_typescript::types::ts_type;
use veld_emit_typescript::util::{add_file, doc_comment, method_key, prop_key, ts_str};
use veld_emit_typescript::validate::schema_expr;
use veld_ir::{AuthKind, Constraints, DeclKind, Endpoint, Ir, Method, Service};

use crate::events;
use crate::frameworks::{EndpointModel, ServiceModel};
use crate::guards;
use crate::outbox;
use crate::rules::{self, Site};

/// Root-relative module path of a service's server file.
pub(crate) fn module_of(tree: &Tree<'_>, svc: &Service) -> String {
    format!("{}server/{}", tree.root, naming::kebab(&svc.name))
}

/// Emits one server module per service and returns their module paths.
pub fn emit(
    files: &mut FileSet,
    tree: &Tree<'_>,
    runtime: Option<&RuntimeModel>,
) -> Result<Vec<String>, Vec<GenDiagnostic>> {
    let names = RequestNames::new(tree.ir, &tree.services);
    let mut modules = Vec::new();
    let mut diags = Vec::new();
    for sid in &tree.services {
        let svc = tree.ir.service(*sid);
        // A service that only publishes or subscribes has no HTTP surface: its module is the
        // events module (`events.rs`).
        if !events::serves_http(svc) {
            continue;
        }
        let module = module_of(tree, svc);
        let mut fc = FileCtx::new(tree, &module);
        let mut w = CodeWriter::new();
        write_service(&mut w, &mut fc, &names, (svc, runtime), &mut diags);
        add_file(files, &format!("{module}.ts"), fc.finish(&w.finish()))?;
        modules.push(module);
    }
    if diags.is_empty() {
        Ok(modules)
    } else {
        Err(diags)
    }
}

fn write_service(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    (svc, runtime): (&Service, Option<&RuntimeModel>),
    diags: &mut Vec<GenDiagnostic>,
) {
    for ep in &svc.endpoints {
        veld_emit_typescript::version::reserved_names(svc, ep, diags);
        if let (true, Some(name)) = (outbox::emits(ep), outbox::clashes(ep)) {
            let mut d = GenDiagnostic::new(
                "E0801",
                format!(
                    "{}.{}: a parameter or fact called `{name}` clashes with the `emit` the handler of a move that emits receives",
                    svc.name, ep.name
                ),
            );
            d.origin = Some(ep.origin.clone());
            diags.push(d);
        }
        audited_stream(svc, ep, diags);
        let request = names.get(svc.id, &ep.name);
        write_request_type(w, fc, &request, ep);
        w.blank();
        rules::write_facts_type(w, fc, &request, ep);
        guards::write_version_type(w, fc, &request, ep);
        guards::write_transition_type(w, fc, &request, ep);
    }
    write_interface(w, fc, names, svc);
    w.blank();
    if !svc.facts.is_empty() {
        rules::write_loaders(w, fc, svc, &naming::pascal(&svc.name));
        w.blank();
    }
    write_hooks(w, fc, svc, runtime);
    w.blank();
    write_factory(w, fc, names, (svc, runtime), diags);
    if !timed_endpoints(fc.tree.ir, svc).is_empty() {
        w.blank();
        write_due_moves(w, fc, names, (svc, runtime));
    }
}

/// An audited `ws` or streaming endpoint is `E0801`: a stream has no single outcome to record
/// (`spec/platform.md` section 9.1).
fn audited_stream(svc: &Service, ep: &Endpoint, diags: &mut Vec<GenDiagnostic>) {
    if ep.audited && (ep.success.stream || ep.method == Method::Ws) {
        let mut d = GenDiagnostic::new(
            "E0801",
            format!(
                "{}.{}: an audited `{}` endpoint is not supported by the `node` generator: a stream has no single outcome to record",
                svc.name,
                ep.name,
                if ep.method == Method::Ws { "ws" } else { "sse" }
            ),
        );
        d.origin = Some(ep.origin.clone());
        diags.push(d);
    }
}

/// The endpoints of a service that take a timed edge, with the milliseconds of that edge
/// (`spec/platform.md` section 10.1).
pub(crate) fn timed_endpoints<'a>(ir: &'a Ir, svc: &'a Service) -> Vec<(&'a Endpoint, &'a str)> {
    svc.endpoints
        .iter()
        .filter_map(|ep| {
            let t = ep.rules.transition.as_ref().filter(|t| t.is_timed())?;
            let source = t.from.first()?;
            let DeclKind::Enum(machine) = &ir.decl(t.decl).kind else {
                return None;
            };
            let after = machine
                .flows
                .iter()
                .find(|f| f.from == *source && f.to == t.to)?
                .after_ms
                .as_deref()?;
            Some((ep, after))
        })
        .collect()
}

/// `<Service>DueLoaders`, `<Service>DueOptions` and `run<Service>DueMoves` (`spec/platform.md`
/// section 10.1, `spec/targets/typescript.md` section 5j).
fn write_due_moves(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    (svc, runtime): (&Service, Option<&RuntimeModel>),
) {
    let name = naming::pascal(&svc.name);
    let ctx = fc.rt("RequestContext");
    let timed = timed_endpoints(fc.tree.ir, svc);
    w.line(format!(
        "/** One loader per endpoint of `{}` that takes a timed edge: it answers the parameters of up to `limit` calls whose fact entered the source state at or before `before`. */",
        svc.name
    ));
    w.open(format!("export interface {name}DueLoaders"));
    for (ep, _) in &timed {
        let request = names.get(svc.id, &ep.name);
        w.line(format!(
            "{}(before: Date, limit: number, ctx: {ctx}): Promise<readonly {request}[]>;",
            method_key(&ep.name)
        ));
    }
    w.close();
    w.blank();
    let options = fc.rt("DueOptions");
    w.open(format!(
        "export interface {name}DueOptions extends {options}"
    ));
    w.line(format!("loaders: {name}DueLoaders;"));
    w.close();
    w.blank();
    write_due_runner(w, fc, (svc, runtime), &timed);
}

/// `run<Service>DueMoves(impl, hooks, options, runtime?)`: builds the endpoint descriptions and
/// hands the loaders to the runtime's `runDueMoves`.
fn write_due_runner(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    (svc, runtime): (&Service, Option<&RuntimeModel>),
    timed: &[(&Endpoint, &str)],
) {
    let name = naming::pascal(&svc.name);
    let report = fc.rt("DueReport");
    let runtime_param = match runtime {
        Some(m) => {
            let ty = m.type_name();
            fc.import("runtime", &ty, false);
            let optional = if m.has_required() { "" } else { "?" };
            format!(", runtime{optional}: {ty}")
        }
        None => String::new(),
    };
    doc_comment(
        w,
        Some(&format!(
            "Runs the moves of `{}` that are due, through the pipeline and in process, as the caller `hooks.timer.principal()` answers. Run it from an interval, a cron job or a queue worker. The guard is not an atomic guarantee: two runs at once are safe exactly as far as the handler's conditional write is.",
            svc.name
        )),
        None,
    );
    w.open(format!(
        "export function run{name}DueMoves(impl: {name}Service, hooks: {name}Hooks, options: {name}DueOptions{runtime_param}): Promise<{report}>"
    ));
    if runtime.is_some() {
        fc.import("runtime", "checkRuntime", true);
        if output_is_a_setting(runtime) {
            w.line("const config = checkRuntime(runtime);");
        } else {
            w.line("checkRuntime(runtime);");
        }
    }
    let loaders = if svc.facts.is_empty() {
        ""
    } else {
        ", hooks.facts"
    };
    w.line(format!(
        "const endpoints = {}(impl{loaders});",
        builder_name(svc)
    ));
    let target = fc.rt("DueTarget");
    w.open_with(format!("const targets: {target}[] ="), " [");
    for (ep, after) in timed {
        let key = method_key(&ep.name);
        w.line(format!(
            "{{ endpoint: {}, afterMs: {after}, load: (before, limit, ctx) => options.loaders.{key}(before, limit, ctx) }},",
            ts_str(&ep.name)
        ));
    }
    w.close_with("];");
    let std = std_sigs(fc);
    let run = fc.rt("runDueMoves");
    let hooks = handler_hooks(fc, runtime);
    w.line(format!(
        "return {run}(endpoints, {std}, {hooks}, targets, options);"
    ));
    w.close();
}

/// Whether the service has an endpoint with an `@slo`: its hooks take `metrics`.
pub(crate) fn service_has_slo(svc: &Service) -> bool {
    svc.endpoints.iter().any(|ep| ep.slo.is_some())
}

/// The request type of an endpoint as the implementation receives it: with its loaded facts.
fn impl_request(fc: &mut FileCtx<'_, '_>, svc: &Service, ep: &Endpoint, request: &str) -> String {
    let mut parts = vec![request.to_string()];
    if !ep.rules.facts.is_empty() {
        parts.push(rules::facts_name(request));
    }
    if ep.rules.version.is_some() {
        parts.push(guards::version_name(request));
    }
    if ep.rules.transition.is_some() {
        parts.push(guards::transition_name(request));
    }
    if crate::outbox::emits(ep) {
        let emit = crate::outbox::emit_name(request);
        fc.import(&events::module_of(fc.tree, svc), &emit, false);
        parts.push(emit);
    }
    parts.join(" & ")
}

/// Name of the function that builds the endpoint descriptions of a service with endpoint rules.
pub(crate) fn builder_name(svc: &Service) -> String {
    format!("{}Endpoints", naming::camel(&svc.name))
}

fn write_interface(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
) {
    let name = naming::pascal(&svc.name);
    let ctx = fc.rt("RequestContext");
    doc_comment(w, svc.docs.as_deref(), None);
    w.open(format!("export interface {name}Service<Ctx = {ctx}>"));
    for ep in &svc.endpoints {
        let docs = deadline_docs(ep);
        doc_comment(w, docs.as_deref(), ep.deprecated.as_deref());
        // What the handler returns: a derived field is optional, the runtime computes it.
        let ret = match &ep.success.ty {
            Some(t) => derived::shaped_type(fc, t, derived::Shape::Output),
            None => "void".to_string(),
        };
        let key = method_key(&ep.name);
        let request = names.get(svc.id, &ep.name);
        let req = impl_request(fc, svc, ep, &request);
        if ep.method == Method::Ws {
            let (incoming, outgoing) = ws_types(fc, ep);
            let session = fc.rt("WsSession");
            w.line(format!(
                "{key}(req: {req}, ctx: Ctx, ws: {session}<{incoming}, {outgoing}>): Promise<void>;"
            ));
        } else if ep.success.stream {
            let event = fc.rt("SseEvent");
            let item = format!("{ret} | {event}<{ret}>");
            w.line(format!(
                "{key}(req: {req}, ctx: Ctx): AsyncIterable<{item}> | Promise<AsyncIterable<{item}>>;"
            ));
        } else {
            w.line(format!("{key}(req: {req}, ctx: Ctx): Promise<{ret}>;"));
        }
    }
    w.close();
}

/// The message types of a `ws` endpoint: (from the client, to the client); `never` for a side
/// that has none.
fn ws_types(fc: &mut FileCtx<'_, '_>, ep: &Endpoint) -> (String, String) {
    let incoming = match ep.params.iter().find(|p| p.stream) {
        Some(p) => ts_type(fc, &p.ty),
        None => "never".to_string(),
    };
    let outgoing = match &ep.success.ty {
        Some(t) => derived::shaped_type(fc, t, derived::Shape::Output),
        None => "never".to_string(),
    };
    (incoming, outgoing)
}

/// The documentation of an endpoint, with the warning about its deadline when it has one: a
/// handler cannot be stopped, so one that writes must notice the deadline itself.
fn deadline_docs(ep: &Endpoint) -> Option<String> {
    let Some(ms) = &ep.timeout_ms else {
        return ep.docs.clone();
    };
    let note = format!(
        "Deadline: {ms} ms. When the pipeline has no answer in time the server answers `504 DeadlineExceeded` and drops what this handler answers later. The handler is not stopped: one that writes must watch `ctx.signal` or `ctx.deadline` itself."
    );
    Some(match &ep.docs {
        Some(docs) => format!("{docs}\n\n{note}"),
        None => note,
    })
}

/// The auth hook of a kind. `who` is what the hook resolves to: `unknown`, or the generated
/// type of the principal of a service that has one (`spec/rules.md` section 11.1).
fn auth_hook(kind: AuthKind, ctx: &str, who: &str) -> Option<(&'static str, String)> {
    match kind {
        AuthKind::None => None,
        AuthKind::Bearer => Some((
            "bearer",
            format!("(token: string, ctx: {ctx}) => Promise<{who} | null>"),
        )),
        AuthKind::Basic => Some((
            "basic",
            format!("(username: string, password: string, ctx: {ctx}) => Promise<{who} | null>"),
        )),
        AuthKind::ApiKey => Some((
            "apiKey",
            format!("(key: string, ctx: {ctx}) => Promise<{who} | null>"),
        )),
    }
}

/// Auth hooks and middleware names the service's endpoints need, in a stable order.
fn required_hooks(
    svc: &Service,
    ctx: &str,
    who: &str,
) -> (Vec<(&'static str, String)>, Vec<String>) {
    let mut auth: Vec<(&'static str, String)> = Vec::new();
    let mut middleware: Vec<String> = Vec::new();
    for ep in &svc.endpoints {
        if let Some(h) = auth_hook(ep.auth.kind, ctx, who)
            && !auth.iter().any(|(n, _)| *n == h.0)
        {
            auth.push(h);
        }
        for m in &ep.middleware {
            if !middleware.contains(m) {
                middleware.push(m.clone());
            }
        }
    }
    auth.sort_by_key(|(n, _)| *n);
    (auth, middleware)
}

/// Whether the output mode of the rules is a setting of the runtime config, and so no longer a
/// member of the hooks (`spec/runtime-config.md` section 5).
fn output_is_a_setting(runtime: Option<&RuntimeModel>) -> bool {
    runtime.is_some_and(|m| m.setting(RULES_OUTPUT).is_some())
}

fn write_hooks(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    svc: &Service,
    runtime: Option<&RuntimeModel>,
) {
    let ctx = fc.rt("RequestContext");
    // A service with a principal resolves its auth hooks to the principal's model, when a
    // statement reads the caller (`spec/rules.md` section 11.1).
    let who = match (&svc.principal, fc.tree.uses_policies()) {
        // What an auth hook returns: the derived fields of the principal are optional, the
        // runtime computes them.
        (Some(p), true) => {
            derived::shaped_type(fc, &veld_ir::Type::Decl(p.decl), derived::Shape::Output)
        }
        _ => "unknown".to_string(),
    };
    let (auth, middleware) = required_hooks(svc, &ctx, &who);
    w.open(format!(
        "export interface {}Hooks",
        naming::pascal(&svc.name)
    ));
    if !svc.facts.is_empty() {
        w.line("/** The loaders of the facts the endpoint rules need. */");
        w.line(format!(
            "facts: {};",
            rules::loaders_name(&naming::pascal(&svc.name))
        ));
    }
    if rules::service_has_clock(fc.tree, svc) {
        w.line("/** The server's clock, read once per request before the facts load. Default: the system clock. */");
        w.line(format!("clock?: (ctx: {ctx}) => Date;"));
    }
    if rules::service_audits(fc.tree, svc) {
        let event = fc.rt("AuditEvent");
        w.line("/** Called once for every request that matched an audited endpoint, after its outcome is known and before the answer is written (`spec/platform.md` section 4). A failing hook does not change the answer: the failure goes to `onError`. */");
        w.line(format!("audit: (event: {event}) => void | Promise<void>;"));
    }
    if rules::service_uses_flags(fc.tree, svc) {
        let provider = fc.rt("FlagProvider");
        w.line("/** Asks whether a flag is on, once per request and flag (`spec/platform.md` section 11). `MemoryFlagProvider` is one for tests. */");
        w.line(format!("flags: {provider};"));
    }
    if outbox::service_emits(svc) {
        let ns = fc.events_ns();
        w.line("/** The application's outbox, over its own store (`spec/events.md` section 11.1): the handler of a move that emits writes to it, in the transaction that changes the state. `MemoryOutbox` is one for tests. */");
        w.line(format!("outbox: {ns}.Outbox;"));
        w.line("/** The id source, the clock and the rule hook of `emit`. Default: a random UUID and the system clock. */");
        w.line(format!("events?: {ns}.PublisherOptions;"));
    }
    if !timed_endpoints(fc.tree.ir, svc).is_empty() {
        w.line("/** The principal the application gives its own timer, for `run<Service>DueMoves`. Required there when a timed endpoint has auth. */");
        w.line(format!(
            "timer?: {{ principal: () => {who} | Promise<{who}> }};"
        ));
    }
    write_idempotency_hook(w, fc, svc, (&ctx, &who));
    write_limit_hooks(w, fc, svc, (&ctx, &who));
    if !auth.is_empty() {
        w.open("auth:");
        for (name, ty) in &auth {
            w.line(format!("{name}: {ty};"));
        }
        w.close_with("};");
    }
    if !middleware.is_empty() {
        w.open("middleware:");
        for name in &middleware {
            w.line(format!(
                "{}: (ctx: {ctx}) => Promise<void>;",
                prop_key(name)
            ));
        }
        w.close_with("};");
    }
    w.line(format!("onError?: (err: unknown, ctx: {ctx}) => void;"));
    if service_has_slo(svc) {
        let metric = fc.rt("Metric");
        w.line("/** Called once per request that matched an endpoint of this server, with the answer ready (`spec/platform.md` section 13). A failing hook is ignored. */");
        w.line(format!("metrics?: (m: {metric}) => void | Promise<void>;"));
    }
    w.line("validateResponses?: boolean;");
    if fc.tree.uses_rules() && !output_is_a_setting(runtime) {
        let mode = fc.rt("RuleOutputMode");
        w.line("/** How rules on this server's own output are enforced (default `strict`). */");
        w.line(format!("rules?: {{ output?: {mode} }};"));
    }
    // Tenancy reports the values it refuses like an output violation, with no invariant in sight.
    if fc.tree.uses_rules() || fc.tree.uses_tenancy() {
        let event = fc.rt("RuleViolationEvent");
        w.line(format!("onRuleViolation?: (event: {event}) => void;"));
    }
    if fc.tree.uses_capture() {
        w.line("/** Whether the event of `onRuleViolation` carries a `capture` of the request (`spec/rules.md` section 17.3): on unless `false`, because the target was generated with `capture-violations`. A capture contains whatever the caller sent, except the values of `@pii` and `@secret` fields: send it only where such data may go. */");
        w.line("captureViolations?: boolean;");
    }
    if !fc.tree.validate {
        w.line("/** This target was generated with `validate = false`: requests are not validated unless this is true. */");
        w.line("validateRequests?: boolean;");
    }
    w.close();
}

/// `idempotency: { store, caller }` in the hooks of a service with an `@idempotent` endpoint
/// (`spec/rules.md` section 11.12). The type requires what the endpoints need, so a server that
/// lacks it does not compile; the runtime checks it again when the handler is created.
fn write_idempotency_hook(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    svc: &Service,
    (ctx, who): (&str, &str),
) {
    if !rules::service_is_idempotent(svc) {
        return;
    }
    let store = fc.rt("IdempotencyStore");
    w.line("/** Where the keys of the `@idempotent` endpoints live, and who the caller is for the scope of a key. */");
    let caller = svc
        .endpoints
        .iter()
        .any(|ep| ep.idempotency.as_ref().is_some_and(|i| i.caller));
    if caller {
        w.line(format!(
            "idempotency: {{ store: {store}; caller: (principal: {who}, ctx: {ctx}) => string | Promise<string> }};"
        ));
    } else {
        w.line(format!("idempotency: {{ store: {store} }};"));
    }
}

/// `limiter`, `address` and `caller` of a service with a `@limit` (`spec/platform.md` section 5).
/// `address` (`by: ip`) and `caller` (`by: user`, unless `idempotency.caller` is there already) are
/// required by the type, so a server that needs one and has none does not compile; the runtime
/// checks it again when the handler is created.
fn write_limit_hooks(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    svc: &Service,
    (ctx, who): (&str, &str),
) {
    if !rules::service_has_limits(fc.tree, svc) {
        return;
    }
    let limiter = fc.rt("Limiter");
    w.line("/** Counts the calls of the endpoints with a `@limit`. Without one a memory limiter is used, which counts per process, and says so once at start-up. */");
    w.line(format!("limiter?: {limiter};"));
    if rules::service_needs_address(fc.tree, svc) {
        w.line("/** The client address of a request, for `by: ip`: only the application knows which proxy header to trust. */");
        w.line(format!(
            "address: (ctx: {ctx}) => string | Promise<string>;"
        ));
    }
    if rules::service_needs_limit_caller(fc.tree, svc) {
        w.line("/** Who the caller is, as a text, for `by: user`. */");
        w.line(format!(
            "caller: (principal: {who}, ctx: {ctx}) => string | Promise<string>;"
        ));
    }
}

/// Where the writers of one service put their output: the body and the declarations that must
/// come before it.
struct Out<'a> {
    w: &'a mut CodeWriter,
    pre: &'a mut CodeWriter,
    diags: &'a mut Vec<GenDiagnostic>,
}

fn write_endpoint(
    out: &mut Out<'_>,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    ep: &Endpoint,
) {
    let w = &mut *out.w;
    w.open_with("", "{");
    write_spec_fields(w, fc, ep);
    let list: Vec<String> = ep.middleware.iter().map(|m| ts_str(m)).collect();
    w.line(format!("middleware: [{}],", list.join(", ")));
    let flags = rules::endpoint_flags(fc.tree, ep);
    if fc.tree.uses_flags() && !flags.is_empty() {
        let list: Vec<String> = flags.iter().map(|f| ts_str(f)).collect();
        w.line(format!("flags: [{}],", list.join(", ")));
    }
    if fc.tree.uses_slo() {
        let package = &fc.tree.ir.package(svc.package).name;
        w.line(format!(
            "service: {},",
            ts_str(&format!("{package}/{}", svc.name))
        ));
    }
    for line in veld_emit_typescript::platform::endpoint_members(fc, svc, ep) {
        w.line(line);
    }
    if let Some(p) = svc.principal.as_ref().filter(|_| fc.tree.uses_derived()) {
        let ty = veld_ir::Type::Decl(p.decl);
        if fc.tree.ir.type_reaches_derived(&ty) {
            // The principal is completed once per request, after authentication (spec 5i).
            let schema = schema_expr(fc, &ty, &Constraints::default());
            w.line(format!("principalSchema: {schema},"));
        }
    }
    let request = names.get(svc.id, &ep.name);
    let req = impl_request(fc, svc, ep, &request);
    if ep.method == Method::Ws {
        let (incoming, outgoing) = ws_types(fc, ep);
        let session = fc.rt("WsSession");
        w.line(format!(
            "call: (req, ctx, ws) => impl.{}(req as {req}, ctx, ws as {session}<{incoming}, {outgoing}>),",
            method_key(&ep.name)
        ));
    } else {
        w.line(format!(
            "call: (req, ctx) => impl.{}(req as {req}, ctx),",
            method_key(&ep.name)
        ));
    }
    if rules::endpoint_has_rules(ep) {
        if ep.method == Method::Ws {
            let mut d = GenDiagnostic::new(
                "E0801",
                format!(
                    "{}.{}: endpoint rules on a `ws` endpoint are not supported by the `node` generator",
                    svc.name, ep.name
                ),
            );
            d.origin = ep.rules.first_origin().cloned();
            out.diags.push(d);
        }
        let site = Site {
            svc,
            ep,
            request: &request,
        };
        rules::write_rules_member(w, out.pre, fc, &site, out.diags);
    }
    if let Some(digest) = svc.rules_digest.as_deref() {
        w.line(format!("digest: {},", ts_str(digest)));
    }
    w.close_with("},");
}

/// `export function bookingsEndpoints(impl, loaders): ServerEndpoint[]`: the endpoint
/// descriptions of a service with endpoint rules. The handler factory and the test handler build
/// on it.
fn write_builder(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    diags: &mut Vec<GenDiagnostic>,
) {
    let name = naming::pascal(&svc.name);
    let loaders = if svc.facts.is_empty() {
        String::new()
    } else {
        format!(", loaders: {}", rules::loaders_name(&name))
    };
    let endpoint = fc.rt("ServerEndpoint");
    let mut body = CodeWriter::new();
    doc_comment(
        &mut body,
        Some(&format!(
            "The endpoint descriptions of `{}`, with the facts, `require`, `old` and `ensure` of their blocks.",
            svc.name
        )),
        None,
    );
    body.open(format!(
        "export function {}(impl: {name}Service{loaders}): {endpoint}[]",
        builder_name(svc)
    ));
    body.open_with("return", " [");
    let mut pre = CodeWriter::new();
    let mut inner = Out {
        w: &mut body,
        pre: &mut pre,
        diags,
    };
    for ep in &svc.endpoints {
        write_endpoint(&mut inner, fc, names, svc, ep);
    }
    body.close_with("];");
    body.close();
    let (pre, body) = (pre.finish(), body.finish());
    for line in pre.lines() {
        w.line(line);
    }
    if !pre.is_empty() {
        w.blank();
    }
    for line in body.lines() {
        w.line(line);
    }
    w.blank();
}

fn write_factory(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    (svc, runtime): (&Service, Option<&RuntimeModel>),
    diags: &mut Vec<GenDiagnostic>,
) {
    let name = naming::pascal(&svc.name);
    let ctx = fc.rt("RequestContext");
    let (auth, middleware) = required_hooks(svc, &ctx, "unknown");
    // The runtime config follows the hooks, so the hooks lose their default when it is required.
    let runtime_required = runtime.is_some_and(RuntimeModel::has_required);
    let default = if auth.is_empty()
        && middleware.is_empty()
        && svc.facts.is_empty()
        && !rules::service_is_idempotent(svc)
        && !rules::service_uses_flags(fc.tree, svc)
        && !rules::service_audits(fc.tree, svc)
        && !rules::service_limit_hooks_required(fc.tree, svc)
        && !outbox::service_emits(svc)
        && !runtime_required
    {
        " = {}"
    } else {
        ""
    };
    let ruled = rules::service_has_builder(fc.tree, svc);
    if ruled {
        write_builder(w, fc, names, svc, diags);
    }
    let handler = fc.rt("VeldHandler");
    doc_comment(
        w,
        Some(&format!(
            "Builds the HTTP handler for `{}`. Mount it with an adapter from `./adapters`.",
            svc.name
        )),
        None,
    );
    let runtime_param = match runtime {
        Some(m) => {
            let ty = m.type_name();
            fc.import("runtime", &ty, false);
            let optional = if m.has_required() { "" } else { "?" };
            format!(", runtime{optional}: {ty}")
        }
        None => String::new(),
    };
    w.open(format!("export function create{name}Handler(impl: {name}Service, hooks: {name}Hooks{default}{runtime_param}): {handler}"));
    if runtime.is_some() {
        fc.import("runtime", "checkRuntime", true);
        if output_is_a_setting(runtime) {
            w.line("const config = checkRuntime(runtime);");
        } else {
            w.line("checkRuntime(runtime);");
        }
    }
    if ruled {
        let loaders = if svc.facts.is_empty() {
            ""
        } else {
            ", hooks.facts"
        };
        w.line(format!(
            "const endpoints = {}(impl{loaders});",
            builder_name(svc)
        ));
    } else {
        let endpoint = fc.rt("ServerEndpoint");
        w.open_with(format!("const endpoints: {endpoint}[] ="), " [");
        let mut pre = CodeWriter::new();
        let mut out = Out {
            w: &mut *w,
            pre: &mut pre,
            diags,
        };
        for ep in &svc.endpoints {
            write_endpoint(&mut out, fc, names, svc, ep);
        }
        w.close_with("];");
    }
    let std = std_sigs(fc);
    let create = fc.rt("createHandler");
    let hooks = handler_hooks(fc, runtime);
    w.line(format!("return {create}(endpoints, {std}, {hooks});"));
    w.close();
}

/// The hooks `createHandler` receives: the hooks, with `validateRequests: false` when the target
/// skips request validation and `rules.output` from the checked runtime config.
fn handler_hooks(fc: &mut FileCtx<'_, '_>, runtime: Option<&RuntimeModel>) -> String {
    let validate = fc.tree.validate;
    let mut parts = Vec::new();
    if !validate {
        parts.push("validateRequests: false".to_string());
    }
    parts.push("...hooks".to_string());
    if let Some(output) = runtime.and_then(|m| m.setting(RULES_OUTPUT)) {
        let default = output
            .default
            .as_deref()
            .map(|d| format!(" ?? {}", ts_str(d)))
            .unwrap_or_default();
        parts.push(format!(
            "rules: {{ output: config.rules?.output{default} }}"
        ));
    }
    // The catalogs and the keys of the member, from `messages.ts` (`spec/platform.md` section 22).
    if fc.tree.uses_messages_server() && fc.tree.uses_rules() {
        let module = veld_emit_typescript::messages::module(fc.tree, fc.tree.packages()[0]);
        for name in ["fixedMessageKeys", "messageKeys", "messages"] {
            fc.import(&module, name, true);
        }
        parts.push(
            "messages: { catalogs: messages, keys: messageKeys, fixed: fixedMessageKeys }"
                .to_string(),
        );
    }
    if parts.len() == 1 {
        return "hooks".to_string();
    }
    format!("{{ {} }}", parts.join(", "))
}

pub(crate) fn std_sigs(fc: &mut FileCtx<'_, '_>) -> String {
    let endpoint_rules = fc.tree.uses_endpoint_rules();
    let conflict = fc.tree.uses_idempotency();
    let deadline = fc.tree.uses_timeouts();
    let limits = fc.tree.uses_limits_server();
    let mut sig = |name: &str| match fc.tree.prelude_decl(name) {
        Some(id) => format!("{}.sig", fc.value_ref(id)),
        None => "'about:blank'".to_string(),
    };
    let (u, v, i) = (sig("Unauthorized"), sig("ValidationError"), sig("Internal"));
    let mut out = format!("unauthorized: {u}, validation: {v}, internal: {i}");
    if endpoint_rules {
        out.push_str(&format!(", notFound: {}", sig("NotFound")));
    }
    if conflict {
        out.push_str(&format!(", conflict: {}", sig("Conflict")));
    }
    if deadline {
        out.push_str(&format!(", deadline: {}", sig("DeadlineExceeded")));
    }
    if limits {
        out.push_str(&format!(", tooMany: {}", sig("TooManyRequests")));
    }
    format!("{{ {out} }}")
}

/// Nest's route syntax: `{id}` becomes `:id`, and the ends carry no slash.
fn nest_path(path: &str) -> String {
    let mut out = String::new();
    let mut rest = path.trim_matches('/');
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(len) = rest[open..].find('}') else {
            break;
        };
        out.push(':');
        out.push_str(&rest[open + 1..open + len]);
        rest = &rest[open + len + 1..];
    }
    out.push_str(rest);
    out
}

/// The services as the framework adapters see them.
#[must_use]
pub fn models(tree: &Tree<'_>) -> Vec<ServiceModel> {
    tree.services
        .iter()
        .filter(|sid| events::serves_http(tree.ir.service(**sid)))
        .map(|sid| {
            let svc = tree.ir.service(*sid);
            let (auth, middleware) = required_hooks(svc, "", "unknown");
            let prefix = svc.prefix.trim_end_matches('/');
            let mut endpoints: Vec<(usize, EndpointModel)> = svc
                .endpoints
                .iter()
                // `ws` endpoints are not Nest routes: they attach to the HTTP server (`server/ws.ts`).
                .filter(|ep| ep.method != Method::Ws)
                .map(|ep| {
                    let below = ep.path.strip_prefix(prefix).unwrap_or(&ep.path);
                    let params = ep.path.matches('{').count();
                    let model = EndpointModel {
                        name: method_key(&ep.name),
                        method: ep.method,
                        path: nest_path(below),
                        status: ep.success.status,
                    };
                    (params, model)
                })
                .collect();
            endpoints.sort_by_key(|(params, _)| *params);
            ServiceModel {
                raw_name: svc.name.clone(),
                name: naming::pascal(&svc.name),
                file: naming::kebab(&svc.name),
                module: module_of(tree, svc),
                prefix: nest_path(prefix),
                hooks_required: !auth.is_empty()
                    || !middleware.is_empty()
                    || !svc.facts.is_empty()
                    || rules::service_is_idempotent(svc)
                    || rules::service_audits(tree, svc)
                    || rules::service_limit_hooks_required(tree, svc)
                    || rules::service_uses_flags(tree, svc),
                endpoints: endpoints.into_iter().map(|(_, e)| e).collect(),
            }
        })
        .collect()
}
