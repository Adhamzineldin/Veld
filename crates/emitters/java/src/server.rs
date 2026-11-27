//! `<package>.server`: request records, the `<Service>Service` interface, `<Service>Hooks` and
//! `<Service>Handler`.

use std::cell::RefCell;

use veld_codegen::naming;
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_ir::{AuthKind, Endpoint, Service};

use crate::conditional;
use crate::endpoint;
use crate::frameworks::{EndpointKind, EndpointModel, ServiceModel};
use crate::machine;
use crate::outbox;
use crate::spec::{
    RequestNames, emit_requests, endpoint_docs, endpoint_docs_note, is_sse, is_ws, method_name,
    spec_const, write_spec_const, ws_incoming,
};
use crate::tree::{FileCtx, Tree, java_path};
use crate::types::{codec_expr, java_type};
use crate::util::{add_file, doc_comment, end_call, java_str, member_ident, open_call, unique};

/// What the Javadoc of an endpoint with a `@timeout` says about the deadline.
pub const DEADLINE_NOTE: &str = "This endpoint has a deadline (`@timeout`). The method runs on a thread the server cannot stop: when the deadline passes first, the server answers `504 DeadlineExceeded` and drops what this method returns or throws later, and it interrupts the thread once. `ctx.deadline()` is the instant and `ctx.expired()` says whether it has passed (or the server has answered): a method that writes must watch them, or the interrupt, itself.";

/// Emits the server packages of every package of the tree's services.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<(), Vec<GenDiagnostic>> {
    let names = RequestNames::new(tree.ir, &tree.services);
    for pkg in tree.packages() {
        let package = tree.api_package(pkg, "server");
        for sid in tree
            .services
            .iter()
            .filter(|s| tree.ir.service(**s).package == pkg)
            .filter(|s| crate::events::serves_http(tree.ir.service(**s)))
        {
            let svc = tree.ir.service(*sid);
            let mut refused = endpoint::refusals(svc);
            refused.extend(conditional::refusals(svc));
            refused.extend(crate::outbox::refusals(svc));
            if !refused.is_empty() {
                return Err(refused);
            }
            emit_requests(files, tree, &package, (&names, false), svc)?;
            emit_facts(files, tree, &package, &names, svc)?;
            crate::outbox::emit_files(files, tree, &package, &names, svc)?;
            let base = naming::pascal(&svc.name);
            let problems = RefCell::new(Vec::new());
            let mut put =
                |suffix: &str, write: &dyn Fn(&mut CodeWriter, &mut FileCtx<'_, '_>, &str)| {
                    let class = format!("{base}{suffix}");
                    let mut fc = FileCtx::new(tree, &package, &class);
                    let mut w = CodeWriter::new();
                    write(&mut w, &mut fc, &class);
                    add_file(files, &java_path(&package, &class), fc.finish(&w.finish()))
                };
            put("Service", &|w, fc, class| {
                write_interface(w, fc, &names, svc, class)
            })?;
            put("Hooks", &|w, fc, class| write_hooks(w, fc, svc, class))?;
            put("Handler", &|w, fc, class| {
                let found = write_handler(w, fc, &names, svc, class, &base);
                problems.borrow_mut().extend(found);
            })?;
            if crate::testing::wanted(tree.ir, tree.testing, svc) {
                put("TestHandler", &|w, fc, class| {
                    let found =
                        crate::testing::write_test_handler(w, fc, &names, svc, class, &base);
                    problems.borrow_mut().extend(found);
                })?;
            }
            if !svc.facts.is_empty() {
                put("FactLoaders", &|w, fc, class| {
                    endpoint::write_loaders(w, fc, svc, class);
                })?;
            }
            let timed = crate::timed::timed_endpoints(tree.ir, svc);
            if !timed.is_empty() {
                put("DueLoaders", &|w, fc, _| {
                    crate::timed::write_loaders(w, fc, &names, svc, &timed);
                })?;
                put("DueMoves", &|w, fc, _| {
                    let given = handler_given(&base, svc);
                    crate::timed::write_runner(w, fc, svc, &timed, (&given.0, &given.1));
                })?;
            }
            let problems = problems.into_inner();
            if !problems.is_empty() {
                return Err(problems);
            }
        }
    }
    Ok(())
}

/// One record of loaded facts per endpoint that has facts.
fn emit_facts(
    files: &mut FileSet,
    tree: &Tree<'_>,
    package: &str,
    names: &RequestNames,
    svc: &Service,
) -> Result<(), Vec<GenDiagnostic>> {
    for ep in svc.endpoints.iter().filter(|e| !e.rules.facts.is_empty()) {
        let request = names.get(svc.id, &ep.name);
        let class = endpoint::facts_name(&request);
        let mut fc = FileCtx::new(tree, package, &class);
        let mut w = CodeWriter::new();
        endpoint::write_facts_record(&mut w, &mut fc, &request, ep);
        add_file(files, &java_path(package, &class), fc.finish(&w.finish()))?;
    }
    Ok(())
}

fn write_interface(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    class: &str,
) {
    let ctx = fc.rt("RequestContext");
    let docs = svc
        .docs
        .clone()
        .unwrap_or_else(|| format!("The implementation of the `{}` service.", svc.name));
    doc_comment(
        w,
        Some(&format!(
            "{docs}\n\nMethods may throw any exception: a declared error is answered at its status, anything else becomes `Internal`."
        )),
        &[],
        None,
    );
    w.line("@SuppressWarnings(\"deprecation\")");
    w.open(format!("public interface {class}"));
    for (i, ep) in svc.endpoints.iter().enumerate() {
        if i > 0 {
            w.blank();
        }
        let notes: Vec<&str> = [
            ep.timeout_ms.is_some().then_some(DEADLINE_NOTE),
            ep.rules
                .version
                .is_some()
                .then_some(conditional::VERSION_NOTE),
            ep.rules
                .transition
                .is_some()
                .then_some(machine::TRANSITION_NOTE),
            outbox::emits(ep).then_some(outbox::EMIT_NOTE),
        ]
        .into_iter()
        .flatten()
        .collect();
        if notes.is_empty() {
            endpoint_docs(w, ep);
        } else {
            endpoint_docs_note(w, ep, &notes.join("\n\n"));
        }
        let request = names.get(svc.id, &ep.name);
        w.line(signature(fc, ep, &request, &ctx));
    }
    w.close();
}

/// The request parameter list of a handler method: the request record, then the facts record
/// when the endpoint has facts, then the `version` and the `transition` when it has them, then the
/// `emit` of a move that emits (`spec/events.md` section 11.1), then the context.
fn request_params(fc: &mut FileCtx<'_, '_>, ep: &Endpoint, request: &str, ctx: &str) -> String {
    let mut params = vec![format!("{request} request")];
    if !ep.rules.facts.is_empty() {
        params.push(format!("{} facts", endpoint::facts_name(request)));
    }
    if let Some(ty) = conditional::version_type(fc, ep) {
        params.push(format!("{ty} version"));
    }
    if let Some(t) = &ep.rules.transition {
        params.push(format!("{} transition", machine::transition_type(fc, t)));
    }
    if outbox::emits(ep) {
        params.push(format!("{} emit", outbox::emit_name(request)));
    }
    params.push(format!("{ctx} ctx"));
    params.join(", ")
}

/// What follows `request` in a call of a handler method: `facts, version, transition, emit, `, each
/// when the endpoint has it.
pub(crate) fn extra_args(ep: &Endpoint) -> String {
    let mut out = String::new();
    if !ep.rules.facts.is_empty() {
        out.push_str("facts, ");
    }
    if ep.rules.version.is_some() {
        out.push_str("version, ");
    }
    if ep.rules.transition.is_some() {
        out.push_str("transition, ");
    }
    if outbox::emits(ep) {
        out.push_str("emit, ");
    }
    out
}

/// The service parameters of `create` (`OrdersService impl, OrdersFactLoaders loaders`) and the same
/// as the arguments of a call.
pub(crate) fn handler_given(base: &str, svc: &Service) -> (String, String) {
    let service = format!("{base}Service");
    if svc.facts.is_empty() {
        (format!("{service} impl"), "impl".to_string())
    } else {
        (
            format!("{service} impl, {} loaders", endpoint::loaders_name(base)),
            "impl, loaders".to_string(),
        )
    }
}

/// One required hook of a service.
struct Hook {
    /// Constructor parameter name.
    param: String,
    /// Parameter type text (an inner type of `HandlerHooks`).
    ty: String,
    /// Statement that registers it on the builder.
    register: String,
    doc: String,
    /// The typed interface this hook has in the hooks class (an auth hook of a service with a
    /// principal), as lines relative to the class body.
    iface: Vec<String>,
}

/// The typed auth hook of a service with a principal: it returns the principal's model.
fn typed_auth(ctx: &str, principal: &str, (setter, name): (&str, &str)) -> Vec<String> {
    let (what, params) = match setter {
        "basic" => (
            "The basic-credentials hook",
            format!("String username, String password, {ctx} ctx"),
        ),
        "apiKey" => ("The API-key hook", format!("String key, {ctx} ctx")),
        _ => ("The bearer-token hook", format!("String token, {ctx} ctx")),
    };
    vec![
        format!("/** {what}: returns the principal (a `{principal}`), or `null` to reject. */"),
        "@FunctionalInterface".to_string(),
        format!("public interface {name} {{"),
        "  /** Authenticates. */".to_string(),
        format!("  {principal} authenticate({params}) throws Exception;"),
        "}".to_string(),
    ]
}

fn auth_kind_name(kind: AuthKind) -> Option<(&'static str, &'static str, &'static str)> {
    match kind {
        AuthKind::None => None,
        AuthKind::Bearer => Some(("bearer", "BearerAuth", "the bearer-token hook")),
        AuthKind::Basic => Some(("basic", "BasicAuth", "the basic-credentials hook")),
        AuthKind::ApiKey => Some(("apiKey", "ApiKeyAuth", "the API-key hook")),
    }
}

/// The hooks the service's endpoints need, in a stable order: auth hooks by name, then
/// middleware in order of first use.
fn required_hooks(fc: &mut FileCtx<'_, '_>, svc: &Service) -> Vec<Hook> {
    let hooks = fc.rt("HandlerHooks");
    let mut auth: Vec<(&'static str, &'static str, &'static str)> = Vec::new();
    let mut middleware: Vec<String> = Vec::new();
    for ep in &svc.endpoints {
        if let Some(a) = auth_kind_name(ep.auth.kind)
            && !auth.contains(&a)
        {
            auth.push(a);
        }
        for m in &ep.middleware {
            if !middleware.contains(m) {
                middleware.push(m.clone());
            }
        }
    }
    auth.sort_unstable();
    let (needs_store, needs_idempotency_caller) = conditional::hooks_needed(svc);
    // A limit `by: user` asks the caller-identity hook that idempotency uses; a service with a
    // limit and no `@idempotent` endpoint that needs it gets the same hook of its own
    // (`spec/platform.md` section 9.2).
    let (limit_user, limit_ip) = crate::platform::limit_needs(svc);
    let needs_caller = needs_idempotency_caller || limit_user;
    let idempotency: Vec<&str> = [
        needs_store.then_some("store"),
        needs_caller.then_some("caller"),
    ]
    .into_iter()
    .flatten()
    .collect();
    let needs_flags = crate::flags::service_uses(fc.tree, svc);
    let needs_outbox = outbox::service_emits(svc);
    let needs_audit = crate::platform::service_audited(svc);
    let params = unique(
        auth.iter()
            .map(|a| a.0.to_string())
            .chain(middleware.iter().map(|m| member_ident(m)))
            .chain(idempotency.iter().map(|n| (*n).to_string()))
            .chain(needs_flags.then(|| "flags".to_string()))
            .chain(needs_outbox.then(|| "outbox".to_string()))
            .chain(needs_audit.then(|| "audit".to_string()))
            .chain(limit_ip.then(|| "address".to_string()))
            .collect(),
    );
    let principal = match (&svc.principal, fc.tree.policies, auth.is_empty()) {
        (Some(p), true, false) => Some(fc.decl(p.decl)),
        (Some(_) | None, _, _) => None,
    };
    let ctx = match &principal {
        Some(_) => fc.rt("RequestContext"),
        None => String::new(),
    };
    let completes = completed_principal(fc, svc).is_some();
    let mut out = Vec::new();
    for (i, (setter, ty, doc)) in auth.iter().enumerate() {
        let register = if completes {
            register_completing(setter, &params[i])
        } else if principal.is_some() {
            format!("builder.{setter}({}::authenticate);", params[i])
        } else {
            format!("builder.{setter}({});", params[i])
        };
        let hook = match &principal {
            Some(class) => Hook {
                param: params[i].clone(),
                ty: (*ty).to_string(),
                register,
                doc: (*doc).to_string(),
                iface: typed_auth(&ctx, class, (setter, ty)),
            },
            None => Hook {
                param: params[i].clone(),
                ty: format!("{hooks}.{ty}"),
                register,
                doc: (*doc).to_string(),
                iface: Vec::new(),
            },
        };
        out.push(hook);
    }
    for (j, m) in middleware.iter().enumerate() {
        let param = params[auth.len() + j].clone();
        out.push(Hook {
            ty: format!("{hooks}.Middleware"),
            register: format!("builder.middleware({}, {param});", java_str(m)),
            doc: format!("the `{m}` middleware"),
            param,
            iface: Vec::new(),
        });
    }
    let first = auth.len() + middleware.len();
    if needs_store {
        out.push(Hook {
            param: params[first].clone(),
            ty: fc.rt("IdempotencyStore"),
            register: format!("builder.idempotencyStore({});", params[first]),
            doc: "the store of the `@idempotent` endpoints (`MemoryIdempotencyStore` for tests)"
                .to_string(),
            iface: Vec::new(),
        });
    }
    if needs_caller {
        let param = params[first + usize::from(needs_store)].clone();
        out.push(caller_hook(
            fc,
            &hooks,
            principal.as_deref(),
            (param, needs_idempotency_caller, limit_user),
        ));
    }
    if needs_flags {
        let param = params[first + usize::from(needs_store) + usize::from(needs_caller)].clone();
        out.push(flags_hook(fc, svc, param));
    }
    if needs_outbox {
        let at =
            first + usize::from(needs_store) + usize::from(needs_caller) + usize::from(needs_flags);
        out.push(Hook {
            param: params[at].clone(),
            ty: fc.rt("Outbox"),
            register: format!("this.outbox = {};", params[at]),
            doc: "the outbox the moves that emit write to (`MemoryOutbox` for tests)".to_string(),
            iface: Vec::new(),
        });
    }
    if needs_audit {
        let at = first
            + usize::from(needs_store)
            + usize::from(needs_caller)
            + usize::from(needs_flags)
            + usize::from(needs_outbox);
        out.push(Hook {
            param: params[at].clone(),
            ty: format!("{hooks}.AuditHook"),
            register: format!("builder.audit({});", params[at]),
            doc: "the audit log: where the event of every audited request goes (`spec/platform.md` section 4)"
                .to_string(),
            iface: Vec::new(),
        });
    }
    if limit_ip {
        let at = first
            + usize::from(needs_store)
            + usize::from(needs_caller)
            + usize::from(needs_flags)
            + usize::from(needs_outbox)
            + usize::from(needs_audit);
        out.push(Hook {
            param: params[at].clone(),
            ty: format!("{hooks}.AddressHook"),
            register: format!("builder.address({});", params[at]),
            doc: "the address of the client of a request, as text, for the limits `by: ip` (only the application knows which proxy header to trust)"
                .to_string(),
            iface: Vec::new(),
        });
    }
    out
}

/// The generated class of the principal when the hooks of the service are typed: the contract uses
/// policies, the service has a principal and some endpoint authenticates.
fn typed_principal(fc: &mut FileCtx<'_, '_>, svc: &Service) -> Option<String> {
    let p = svc.principal.as_ref()?;
    let authenticates = svc.endpoints.iter().any(|e| e.auth.kind != AuthKind::None);
    (fc.tree.policies && authenticates).then(|| fc.decl(p.decl))
}

/// The principal's model when it has derived fields and the service authenticates: the hooks then
/// complete the value the authentication hook returns (`spec/rules.md` section 17.2).
fn completed_principal(fc: &mut FileCtx<'_, '_>, svc: &Service) -> Option<veld_ir::DeclId> {
    let p = svc.principal.as_ref()?;
    let authenticates = svc.endpoints.iter().any(|e| e.auth.kind != AuthKind::None);
    (authenticates
        && fc
            .tree
            .ir
            .type_reaches_derived(&veld_ir::Type::Decl(p.decl)))
    .then_some(p.decl)
}

/// Registers an authentication hook so that its principal is completed once, after authentication,
/// whatever the endpoint (`Derive.principal`).
fn register_completing(setter: &str, param: &str) -> String {
    let (names, call) = match setter {
        "basic" => (
            "veldUser, veldPassword, veldCtx",
            "veldUser, veldPassword, veldCtx",
        ),
        "apiKey" => ("veldKey, veldCtx", "veldKey, veldCtx"),
        _ => ("veldToken, veldCtx", "veldToken, veldCtx"),
    };
    format!("builder.{setter}(({names}) -> completePrincipal({param}.authenticate({call})));")
}

/// `completePrincipal`: the private helper the registrations above call.
fn write_complete_principal(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, decl: veld_ir::DeclId) {
    let ty = veld_ir::Type::Decl(decl);
    let (class, derive) = (fc.decl(decl), fc.rt("Derive"));
    let schema = crate::types::schema_expr(fc, &ty, &veld_ir::Constraints::default(), false);
    let codec = codec_expr(fc, &ty);
    w.line("/** The caller with its derived fields, computed once per request after authentication. */");
    w.open("private static Object completePrincipal(Object principal)");
    w.line(format!(
        "return {derive}.principal({schema}, {codec}, {class}.class, principal);"
    ));
    w.close();
    w.blank();
}

/// The provider of the flags (`spec/platform.md` section 11): required, because an endpoint of the
/// service can reach a flag. It also registers the flags each endpoint reads, by operation.
fn flags_hook(fc: &mut FileCtx<'_, '_>, svc: &Service, param: String) -> Hook {
    let (flags, map, list) = (
        fc.rt("Flags"),
        fc.name("java.util.Map"),
        fc.name("java.util.List"),
    );
    let entries: Vec<String> = svc
        .endpoints
        .iter()
        .filter_map(|e| {
            let names = crate::flags::endpoint_flags(fc.tree, e);
            (!names.is_empty()).then(|| {
                let quoted: Vec<String> = names.iter().map(|n| crate::flags::quoted(n)).collect();
                format!(
                    "{map}.entry({}, {list}.of({}))",
                    java_str(&e.sig),
                    quoted.join(", ")
                )
            })
        })
        .collect();
    Hook {
        ty: format!("{flags}.Provider"),
        register: format!(
            "builder.flags({param}, {map}.ofEntries({}));",
            entries.join(", ")
        ),
        doc: "the provider of the flags the endpoints can reach (`Flags.Memory` for tests)"
            .to_string(),
        iface: Vec::new(),
        param,
    }
}

/// The hook that tells who the caller is: for the scope of an idempotency key (`idempotent`), for the
/// limits `by: user` (`limited`), or for both. Typed with the principal's model when the service has
/// one.
fn caller_hook(
    fc: &mut FileCtx<'_, '_>,
    hooks: &str,
    principal: Option<&str>,
    (param, idempotent, limited): (String, bool, bool),
) -> Hook {
    let doc = match (idempotent, limited) {
        (true, true) => "who the caller is, as text, for the scope of an idempotency key and for the limits `by: user`",
        (true, false) => "who the caller is, as text, for the scope of an idempotency key",
        (false, _) => "who the caller is, as text, for the limits `by: user`",
    }
    .to_string();
    let typed = |param: &str, class: &str| {
        let mut out = Vec::new();
        if idempotent {
            out.push(format!(
                "builder.idempotencyCaller((principal, ctx) -> {param}.caller(({class}) principal, ctx));"
            ));
        }
        if limited {
            // A hook that is not there stays missing, so that the server does not start (`null`).
            out.push(format!(
                "builder.limitCaller({param} == null ? null : (principal, ctx) -> {param}.caller(({class}) principal, ctx));"
            ));
        }
        out.join("\n")
    };
    let untyped = |param: &str| {
        let mut out = Vec::new();
        if idempotent {
            out.push(format!("builder.idempotencyCaller({param});"));
        }
        if limited {
            out.push(format!(
                "builder.limitCaller({param} == null ? null : {param}::caller);"
            ));
        }
        out.join("\n")
    };
    match principal {
        Some(class) => {
            let ctx = fc.rt("RequestContext");
            let name = if idempotent {
                "IdempotencyCaller"
            } else {
                "Caller"
            };
            Hook {
                ty: name.to_string(),
                register: typed(&param, class),
                doc,
                iface: vec![
                    format!("/** The caller hook: returns the text that stands for the caller (a `{class}`). */"),
                    "@FunctionalInterface".to_string(),
                    format!("public interface {name} {{"),
                    "  /** Returns the caller text; the same caller must always give the same text. */".to_string(),
                    format!("  String caller({class} principal, {ctx} ctx) throws Exception;"),
                    "}".to_string(),
                ],
                param,
            }
        }
        None => Hook {
            ty: if idempotent {
                format!("{hooks}.IdempotencyCaller")
            } else {
                format!("{hooks}.CallerHook")
            },
            register: untyped(&param),
            doc,
            iface: Vec::new(),
            param,
        },
    }
}

fn write_hooks(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service, class: &str) {
    let hooks = fc.rt("HandlerHooks");
    let required = required_hooks(fc, svc);
    doc_comment(
        w,
        Some(&format!(
            "The hooks of the `{}` service. The constructor takes the hooks its endpoints need.",
            svc.name
        )),
        &[],
        None,
    );
    w.open(format!("public final class {class}"));
    let off = if fc.tree.validate {
        ""
    } else {
        ".validateRequests(false)"
    };
    w.line(format!(
        "private final {hooks}.Builder builder = {hooks}.builder(){off};"
    ));
    let clock = if endpoint::service_has_rules(svc) || crate::platform::service_state(fc.tree, svc)
    {
        Some(fc.name("java.time.Clock"))
    } else {
        None
    };
    if let Some(clock) = &clock {
        w.line(format!("private {clock} clock = {clock}.systemUTC();"));
    }
    if outbox::service_emits(svc) {
        let (outbox_ty, events) = (fc.rt("Outbox"), fc.rt("Events"));
        w.line(format!("private final {outbox_ty} outbox;"));
        w.line(format!(
            "private {events}.PublisherOptions events = new {events}.PublisherOptions();"
        ));
    }
    w.blank();
    for h in required.iter().filter(|h| !h.iface.is_empty()) {
        for line in &h.iface {
            w.line(line.clone());
        }
        w.blank();
    }
    let params: Vec<(String, String)> = required
        .iter()
        .map(|h| (h.param.clone(), h.doc.clone()))
        .collect();
    doc_comment(w, Some("Creates the hooks."), &params, None);
    let sig: Vec<String> = required
        .iter()
        .map(|h| format!("{} {}", h.ty, h.param))
        .collect();
    w.open(format!("public {class}({})", sig.join(", ")));
    for h in &required {
        for statement in h.register.lines() {
            w.line(statement);
        }
    }
    for line in crate::platform::registrations(fc, svc) {
        w.line(line);
    }
    for line in crate::messages::registrations(fc, svc) {
        w.line(line);
    }
    let deadlines: Vec<String> = svc
        .endpoints
        .iter()
        .filter_map(|e| {
            e.timeout_ms
                .as_ref()
                .map(|ms| format!("{}, {ms}L", java_str(&e.sig)))
        })
        .collect();
    if !deadlines.is_empty() {
        let map = fc.name("java.util.Map");
        let entries: Vec<String> = deadlines
            .iter()
            .map(|d| format!("{map}.entry({d})"))
            .collect();
        w.line(format!(
            "builder.timeouts({map}.ofEntries({}));",
            entries.join(", ")
        ));
    }
    w.close();
    w.blank();
    let completes = completed_principal(fc, svc);
    if let Some(decl) = completes {
        write_complete_principal(w, fc, decl);
    }
    if outbox::service_emits(svc) {
        outbox::write_hooks_members(w, fc, svc, class);
    }
    if crate::slo::service_has_slo(svc) {
        crate::slo::write_hook(w, fc, class);
    }
    if !crate::timed::timed_endpoints(fc.tree.ir, svc).is_empty() {
        let typed = typed_principal(fc, svc);
        crate::timed::write_hooks_timer(w, fc, class, typed.as_deref(), completes.is_some());
    }
    w.line(
        "/** Observes errors that become `Internal`; a failing hook never changes the response. */",
    );
    w.open(format!("public {class} onError({hooks}.ErrorHook hook)"));
    w.line("builder.onError(hook);");
    w.line("return this;");
    w.close();
    w.blank();
    w.line("/** When true, a response that fails validation becomes a 500 (default false). */");
    w.open(format!("public {class} validateResponses(boolean value)"));
    w.line("builder.validateResponses(value);");
    w.line("return this;");
    w.close();
    w.blank();
    if fc.tree.rules {
        write_rule_hooks(w, fc, class);
    }
    if crate::platform::service_limited(svc) {
        crate::platform::write_limiter(w, fc, class);
    }
    if build_call(fc) != "hooks.build()" {
        write_build_with_config(w, fc);
    }
    if let Some(clock) = &clock {
        w.line("/** The clock `now` reads in endpoint rules, once per request (default: the system clock). */");
        w.open(format!("public {class} clock({clock} value)"));
        w.line("this.clock = value;");
        w.line("return this;");
        w.close();
        w.blank();
        w.open(format!("{clock} clock()"));
        w.line("return clock;");
        w.close();
        w.blank();
    }
    w.open(format!("{hooks} build()"));
    w.line("return builder.build();");
    w.close();
    w.close();
}

/// `build(config)`: the hooks with the rule output mode of the runtime config.
fn write_build_with_config(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>) {
    let hooks = fc.rt("HandlerHooks");
    let ty = fc
        .tree
        .runtime_config
        .as_ref()
        .map(|i| i.type_fqn())
        .unwrap_or_default();
    let ty = fc.name(&ty);
    w.line("/** The hooks with the rule output mode of the runtime config. */");
    w.open(format!("{hooks} build({ty} config)"));
    w.line("builder.rulesOutput(config.rulesOutput());");
    w.line("return builder.build();");
    w.close();
    w.blank();
}

/// The rule options of the hooks: the violation hook (`spec/rules.md` 5.3). No production server
/// keeps the output mode in its hooks: it is `rules.output` of the runtime config, and a server
/// without that setting has no output mode (`spec/runtime-config.md` section 5).
fn write_rule_hooks(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, class: &str) {
    let walk = fc.rt("RuleWalk");
    w.line("/** Observes every violated rule; a failing hook never changes the response. */");
    w.open(format!("public {class} onRuleViolation({walk}.Hook hook)"));
    w.line("builder.onRuleViolation(hook);");
    w.line("return this;");
    w.close();
    w.blank();
    if fc.tree.platform.capture {
        w.line("/** Whether the event of `onRuleViolation` carries a `capture` of the request (`spec/rules.md` section 17.3). The server was generated with `capture-violations`, so the default is `true`; `false` turns it off for this handler. A capture contains whatever the caller sent, except the values of `@pii` and `@secret` fields. */");
        w.open(format!("public {class} captureViolations(boolean value)"));
        w.line("builder.captureViolations(value);");
        w.line("return this;");
        w.close();
        w.blank();
    }
}

fn write_handler(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    class: &str,
    base: &str,
) -> Vec<GenDiagnostic> {
    let handler = fc.rt("VeldHandler");
    let endpoint = fc.rt("ServerEndpoint");
    let list = fc.name("java.util.List");
    let mut problems = Vec::new();
    doc_comment(
        w,
        Some(&format!(
            "Builds the HTTP handler for `{}`. Mount it with an adapter.",
            svc.name
        )),
        &[],
        None,
    );
    w.line("@SuppressWarnings(\"deprecation\")");
    w.open(format!("public final class {class}"));
    w.line(format!("private {class}() {{}}"));
    for ep in &svc.endpoints {
        w.blank();
        write_spec_const(w, fc, ep);
    }
    w.blank();
    let needs_hooks = needs_hooks(fc.tree, svc);
    let hooks_class = format!("{base}Hooks");
    let service = format!("{base}Service");
    let rules = endpoint::service_has_rules(svc);
    let loaders = (!svc.facts.is_empty()).then(|| endpoint::loaders_name(base));
    let (given, pass) = match &loaders {
        Some(l) => (format!("{service} impl, {l} loaders"), "impl, loaders"),
        None => (format!("{service} impl"), "impl"),
    };
    let entry = Entry {
        handler: &handler,
        given: &given,
        pass,
        hooks_class: &hooks_class,
        needs_hooks,
        docs: (
            "Builds the handler with default hooks.",
            "Builds the handler.",
        ),
    };
    entry.write_head(w, fc);
    let site = EndpointsSite {
        names,
        svc,
        endpoint: &endpoint,
        list: &list,
    };
    // A service with a guard, a key or an `ETag` lists its endpoints in a method of its own too: the
    // test handler builds the same endpoints (`spec/targets/java.md` section 9.5).
    if rules
        || conditional::service_uses(fc.tree.ir, svc)
        || crate::platform::service_state(fc.tree, svc)
    {
        write_create_with_endpoints(w, fc, &site, pass, rules);
        w.blank();
        write_endpoints_method(w, fc, &site, &given, rules, &mut problems);
    } else {
        problems.extend(write_create_plain(w, fc, &site));
    }
    if conditional::service_uses(fc.tree.ir, svc) {
        w.blank();
        conditional::write_conditions(w, fc, svc);
    }
    w.close();
    problems
}

/// The call that builds the `HandlerHooks` of an entry: with a runtime config that has the rule
/// output mode, the mode comes from it (`spec/runtime-config.md` section 5).
pub(crate) fn build_call(fc: &FileCtx<'_, '_>) -> &'static str {
    if fc
        .tree
        .runtime_config
        .as_ref()
        .is_some_and(crate::config::Info::has_rules_output)
        && fc.tree.rules
    {
        "hooks.build(config)"
    } else {
        "hooks.build()"
    }
}

/// The `create` methods of a handler: `create(impl[, loaders])`, `create(impl, hooks)` and, when
/// the target has a typed runtime config, the same with the config as the last argument. The
/// config is optional exactly when it has no required path. The check of the config runs first.
pub(crate) struct Entry<'a> {
    /// The handler type returned.
    pub handler: &'a str,
    /// The service parameters (`OrdersService impl, OrdersFactLoaders loaders`).
    pub given: &'a str,
    /// The same as arguments of a call.
    pub pass: &'a str,
    /// The hooks class of the service.
    pub hooks_class: &'a str,
    /// Whether the endpoints need hooks (without it, `create(impl)` builds default hooks).
    pub needs_hooks: bool,
    /// The doc of the shortcut and of the main method.
    pub docs: (&'a str, &'a str),
}

impl Entry<'_> {
    /// Writes the shortcut methods and the head of the main method (up to and including the check of
    /// the config); the caller writes the body and closes the method.
    pub(crate) fn write_head(&self, w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>) {
        let Some(info) = fc.tree.runtime_config.clone() else {
            return self.write_plain(w);
        };
        let ty = fc.name(&info.type_fqn());
        let (handler, given, pass, hooks) = (self.handler, self.given, self.pass, self.hooks_class);
        let optional = !info.model.has_required();
        if optional && !self.needs_hooks {
            w.line(format!("/** {} */", self.docs.0));
            w.open(format!("public static {handler} create({given})"));
            w.line(format!("return create({pass}, new {hooks}(), null);"));
            w.close();
            w.blank();
        }
        if optional {
            w.line(format!(
                "/** {} The runtime config has no required path, so it takes its defaults. */",
                self.docs.1
            ));
            w.open(format!(
                "public static {handler} create({given}, {hooks} hooks)"
            ));
            w.line(format!("return create({pass}, hooks, null);"));
            w.close();
            w.blank();
        }
        if !self.needs_hooks {
            w.line(format!("/** {} */", self.docs.0));
            w.open(format!(
                "public static {handler} create({given}, {ty} runtime)"
            ));
            w.line(format!("return create({pass}, new {hooks}(), runtime);"));
            w.close();
            w.blank();
        }
        w.line(format!(
            "/** {} The runtime config is checked first: `null` means no value, which fails when a path is required. */",
            self.docs.1
        ));
        w.open(format!(
            "public static {handler} create({given}, {hooks} hooks, {ty} runtime)"
        ));
        w.line(format!("{ty} config = {ty}.ensure(runtime);"));
    }

    /// The methods of a target without a runtime config: the bytes of before.
    pub(crate) fn write_plain(&self, w: &mut CodeWriter) {
        let (handler, given, pass, hooks) = (self.handler, self.given, self.pass, self.hooks_class);
        if !self.needs_hooks {
            w.line(format!("/** {} */", self.docs.0));
            w.open(format!("public static {handler} create({given})"));
            w.line(format!("return create({pass}, new {hooks}());"));
            w.close();
            w.blank();
        }
        w.line(format!("/** {} */", self.docs.1));
        w.open(format!(
            "public static {handler} create({given}, {hooks} hooks)"
        ));
    }
}

/// The body of `create` for a service whose endpoints have rules, or a guard, a key or an `ETag`:
/// the hooks (and, with rules, the clock) are read once and handed to `endpoints(...)`, which the
/// test handler calls too.
fn write_create_with_endpoints(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    site: &EndpointsSite<'_>,
    pass: &str,
    rules: bool,
) {
    let built = fc.rt("HandlerHooks");
    let (router, list) = (fc.rt("Router"), site.list);
    w.line(format!("{built} built = {};", build_call(fc)));
    let clock = if rules {
        let clock = fc.name("java.time.Clock");
        w.line(format!("{clock} clock = hooks.clock();"));
        ", ctx -> clock.millis()"
    } else {
        ""
    };
    let emissions = if outbox::service_emits(site.svc) {
        w.line(format!(
            "{}.Source emissions = hooks.emissions();",
            fc.rt("Emission")
        ));
        ", emissions"
    } else {
        ""
    };
    w.line(format!(
        "{list}<{}> endpoints = endpoints({pass}, built{clock}{emissions});",
        site.endpoint
    ));
    let std = std_sigs(fc);
    let args = conditional::router_args(fc.tree.ir, site.svc);
    let create = format!("{router}.create({args}, {std}, built)");
    w.line(format!("return {};", wrap_digest(fc, site.svc, &create)));
    w.close();
}

/// The body of `create` for a service without endpoint rules.
fn write_create_plain(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    site: &EndpointsSite<'_>,
) -> Vec<GenDiagnostic> {
    let (router, list, svc) = (fc.rt("Router"), site.list, site.svc);
    let mut problems = Vec::new();
    open_call(
        w,
        format!("{list}<{}> endpoints = {list}.of", site.endpoint),
    );
    for (i, ep) in svc.endpoints.iter().enumerate() {
        let last = i + 1 == svc.endpoints.len();
        problems.extend(write_endpoint(w, fc, site.names, svc, ep, last));
    }
    end_call(w);
    let std = std_sigs(fc);
    let args = conditional::router_args(fc.tree.ir, svc);
    let create = format!("{router}.create({args}, {std}, {})", build_call(fc));
    w.line(format!("return {};", wrap_digest(fc, svc, &create)));
    w.close();
    problems
}

/// What the method that lists the endpoints of a service needs.
struct EndpointsSite<'a> {
    names: &'a RequestNames,
    svc: &'a Service,
    endpoint: &'a str,
    list: &'a str,
}

/// `return DigestHandler.wrap(create, "digest")` for a service that has a rules digest.
fn wrap_digest(fc: &mut FileCtx<'_, '_>, svc: &Service, create: &str) -> String {
    match (&svc.rules_digest, fc.tree.rules) {
        (Some(digest), true) => {
            let wrapper = fc.rt("DigestHandler");
            format!("{wrapper}.wrap({create}, {})", java_str(digest))
        }
        (Some(_) | None, _) => create.to_string(),
    }
}

/// `static List<ServerEndpoint> endpoints(impl, loaders, hooks, clock)`: the endpoints of a
/// service with rules. Package-private: the test handler builds the same endpoints with loaders
/// and a clock that read the request.
fn write_endpoints_method(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    site: &EndpointsSite<'_>,
    given: &str,
    rules: bool,
    problems: &mut Vec<GenDiagnostic>,
) {
    let hooks = fc.rt("HandlerHooks");
    let clock = if rules {
        let (ctx, to_long) = (
            fc.rt("RequestContext"),
            fc.name("java.util.function.ToLongFunction"),
        );
        format!(", {to_long}<{ctx}> clock")
    } else {
        String::new()
    };
    let clock = if outbox::service_emits(site.svc) {
        // A service whose moves emit: where `emit` writes (the test handler brings its own).
        format!("{clock}, {}.Source emissions", fc.rt("Emission"))
    } else {
        clock
    };
    w.line("/** The endpoints of the service; the test handler reuses them. */");
    w.open(format!(
        "static {}<{}> endpoints({given}, {hooks} hooks{clock})",
        site.list, site.endpoint
    ));
    open_call(w, format!("return {}.of", site.list));
    let svc = site.svc;
    for (i, ep) in svc.endpoints.iter().enumerate() {
        problems.extend(write_endpoint(
            w,
            fc,
            site.names,
            svc,
            ep,
            i + 1 == svc.endpoints.len(),
        ));
    }
    end_call(w);
    w.close();
}

fn write_endpoint(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    ep: &Endpoint,
    last: bool,
) -> Vec<GenDiagnostic> {
    let mut problems = Vec::new();
    let endpoint = fc.rt("ServerEndpoint");
    let list = fc.name("java.util.List");
    let request = names.get(svc.id, &ep.name);
    let middleware: Vec<String> = ep.middleware.iter().map(|m| java_str(m)).collect();
    let end = if last { ");" } else { "," };
    let method = method_name(ep);
    let call = format!("impl.{method}({request}.fromWire(params), ctx)");
    let head = if is_ws(ep) {
        format!("{endpoint}.ws")
    } else if is_sse(ep) {
        format!("{endpoint}.sse")
    } else {
        format!("new {endpoint}")
    };
    open_call(w, head);
    w.line(format!("{},", spec_const(ep)));
    w.line(format!("{list}.of({}),", middleware.join(", ")));
    if is_ws(ep) {
        write_ws_call(
            w,
            fc,
            ep,
            &format!("impl.{method}({request}.fromWire(params), ctx, "),
            end,
        );
    } else if is_sse(ep) {
        let (encode, codec) = (
            fc.rt("SseEvent"),
            ep.success.ty.as_ref().map(|t| codec_expr(fc, t)),
        );
        let codec = codec.unwrap_or_else(|| "null".to_string());
        w.line(format!(
            "(params, ctx) -> {encode}.encode({call}, {codec})){end}"
        ));
    } else if endpoint::endpoint_has_rules(ep) {
        let site = endpoint::Site {
            svc,
            ep,
            request: &request,
        };
        let call = format!("impl.{method}(request, {}ctx)", extra_args(ep));
        let encode = ep
            .success
            .ty
            .as_ref()
            .map(|t| format!("{}.encode(result)", codec_expr(fc, t)));
        w.open_with("(params, ctx) ->", " {");
        problems.extend(endpoint::write_call_body(
            w,
            fc,
            &site,
            &call,
            encode.as_deref(),
        ));
        w.close_with(format!("}}){end}"));
    } else {
        match &ep.success.ty {
            Some(t) => {
                let c = codec_expr(fc, t);
                w.line(format!("(params, ctx) -> {c}.encode({call})){end}"));
            }
            None => {
                w.open_with("(params, ctx) ->", " {");
                w.line(format!("{call};"));
                w.line("return null;");
                w.close_with(format!("}}){end}"));
            }
        }
    }
    end_call(w);
    problems
}

/// The lambda of a `ws` endpoint: the router's raw session is wrapped with the message codecs.
fn write_ws_call(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    ep: &Endpoint,
    call: &str,
    end: &str,
) {
    let session = fc.rt("WsSession");
    let incoming = ws_incoming(ep)
        .map(|p| codec_expr(fc, &p.ty))
        .unwrap_or_else(|| "null".to_string());
    let outgoing = ep
        .success
        .ty
        .as_ref()
        .map(|t| codec_expr(fc, t))
        .unwrap_or_else(|| "null".to_string());
    w.line(format!(
        "(params, ctx, ws) -> {call}{session}.adapt(ws, {incoming}, {outgoing}))){end}"
    ));
}

pub(crate) fn std_sigs(fc: &mut FileCtx<'_, '_>) -> String {
    let router = fc.rt("Router");
    let mut sig = |name: &str| match fc.tree.prelude_decl(name) {
        Some(id) => format!("{}.VELD_SIG", fc.decl(id)),
        None => "\"about:blank\"".to_string(),
    };
    let (u, v, i) = (sig("Unauthorized"), sig("ValidationError"), sig("Internal"));
    format!("new {router}.StdSigs({u}, {v}, {i})")
}

/// Whether any endpoint of the service needs an auth or middleware hook.
pub(crate) fn needs_hooks(tree: &Tree<'_>, svc: &Service) -> bool {
    svc.endpoints
        .iter()
        .any(|e| auth_kind_name(e.auth.kind).is_some() || !e.middleware.is_empty())
        || conditional::hooks_needed(svc).0
        || crate::flags::service_uses(tree, svc)
        || outbox::service_emits(svc)
        || crate::platform::service_audited(svc)
        || {
            let (user, ip) = crate::platform::limit_needs(svc);
            user || ip
        }
}

/// The services of the tree as the framework adapters see them, in tree order.
#[must_use]
pub fn service_models(tree: &Tree<'_>) -> Vec<ServiceModel> {
    let mut out = Vec::new();
    for sid in &tree.services {
        let svc = tree.ir.service(*sid);
        if !crate::events::serves_http(svc) {
            continue;
        }
        let pkg = svc.package;
        let dir = tree
            .pkg_package(pkg)
            .strip_prefix(&format!("{}.", tree.base))
            .unwrap_or_default()
            .to_string();
        out.push(ServiceModel {
            name: naming::pascal(&svc.name),
            server_package: tree.api_package(pkg, "server"),
            adapter_package: format!("{}.adapters.{dir}", tree.base),
            needs_hooks: needs_hooks(tree, svc),
            loaders: (!svc.facts.is_empty())
                .then(|| endpoint::loaders_name(&naming::pascal(&svc.name))),
            endpoints: svc
                .endpoints
                .iter()
                .map(|ep| EndpointModel {
                    name: ep.name.clone(),
                    method_name: method_name(ep),
                    http_method: ep.method.as_str(),
                    path: ep.path.clone(),
                    kind: if is_ws(ep) {
                        EndpointKind::Ws
                    } else if is_sse(ep) {
                        EndpointKind::Sse
                    } else {
                        EndpointKind::Http
                    },
                })
                .collect(),
        });
    }
    out
}

/// The message types of a `ws` endpoint as Java text: (from the client, to the client); `Void`
/// for a side that has none.
fn ws_types(fc: &mut FileCtx<'_, '_>, ep: &Endpoint) -> (String, String) {
    let incoming = match ws_incoming(ep) {
        Some(p) => java_type(fc, &p.ty, true),
        None => "Void".to_string(),
    };
    let outgoing = match &ep.success.ty {
        Some(t) => java_type(fc, t, true),
        None => "Void".to_string(),
    };
    (incoming, outgoing)
}

/// The declaration of an endpoint's method in the `<Service>Service` interface.
pub(crate) fn signature(
    fc: &mut FileCtx<'_, '_>,
    ep: &Endpoint,
    request: &str,
    ctx: &str,
) -> String {
    let method = method_name(ep);
    if is_ws(ep) {
        let (incoming, outgoing) = ws_types(fc, ep);
        let session = fc.rt("WsSession");
        return format!(
            "void {method}({request} request, {ctx} ctx, {session}<{incoming}, {outgoing}> ws) throws Exception;"
        );
    }
    let ret = match &ep.success.ty {
        Some(t) if is_sse(ep) => {
            let (stream, event) = (fc.name("java.util.stream.Stream"), fc.rt("SseEvent"));
            format!("{stream}<{event}<{}>>", java_type(fc, t, true))
        }
        Some(t) => java_type(fc, t, true),
        None => "void".to_string(),
    };
    let params = request_params(fc, ep, request, ctx);
    format!("{ret} {method}({params}) throws Exception;")
}
