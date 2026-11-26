//! Timed moves in the server classes (`spec/platform.md` section 10.1, `spec/targets/java.md`
//! section 16): `<Service>DueLoaders`, the runner `<Service>DueMoves` and the `timer` hook of
//! `<Service>Hooks`. The guard is in `machine.rs`, `timedMoves()` in `models.rs`.

use veld_codegen::CodeWriter;
use veld_ir::{DeclKind, Endpoint, Ir, Service};

use crate::conditional;
use crate::machine::timed_wait;
use crate::server::{build_call, std_sigs};
use crate::spec::{RequestNames, method_name};
use crate::tree::FileCtx;
use crate::util::{doc_comment, java_str};

/// The endpoints of a service that take a timed edge, with the wait of the edge.
#[must_use]
pub fn timed_endpoints<'a>(ir: &Ir, svc: &'a Service) -> Vec<(&'a Endpoint, i64)> {
    svc.endpoints
        .iter()
        .filter_map(|ep| {
            let t = ep.rules.transition.as_ref()?;
            let DeclKind::Enum(machine) = &ir.decl(t.decl).kind else {
                return None;
            };
            timed_wait(machine, t).map(|ms| (ep, ms))
        })
        .collect()
}

/// `<Service>DueLoaders`: one method per timed endpoint that answers the calls that are due.
pub fn write_loaders(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    names: &RequestNames,
    svc: &Service,
    timed: &[(&Endpoint, i64)],
) {
    let ctx = fc.rt("RequestContext");
    let (list, instant) = (fc.name("java.util.List"), fc.name("java.time.Instant"));
    doc_comment(
        w,
        Some(&format!(
            "What the due-moves runner of the `{}` service asks the application's store: one method per endpoint that takes a timed edge. The contract keeps no timer (`spec/platform.md` section 10.1): the row that holds the state is the only source of truth.",
            svc.name
        )),
        &[],
        None,
    );
    w.line("@SuppressWarnings(\"deprecation\")");
    w.open(format!("public interface {}DueLoaders", pascal(&svc.name)));
    for (i, (ep, ms)) in timed.iter().enumerate() {
        if i > 0 {
            w.blank();
        }
        let request = names.get(svc.id, &ep.name);
        w.line("/**");
        w.line(format!(
            " * The calls of `{}` that are due: up to {{@code limit}} requests whose value entered the source state at or before {{@code before}} ({ms} ms before the clock of the run).",
            ep.name
        ));
        w.line(" * The runner runs each answer as a call of the endpoint, through steps 4 to 10 of the pipeline.");
        w.line(" */");
        w.line(format!(
            "{list}<{request}> {}({instant} before, int limit, {ctx} ctx) throws Exception;",
            method_name(ep)
        ));
    }
    w.close();
}

fn pascal(name: &str) -> String {
    veld_codegen::naming::pascal(name)
}

/// `<Service>DueMoves`: the runner. `run` builds the router of the service with the clock of the
/// run and gives every due call to it as an in-process request.
pub fn write_runner(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    svc: &Service,
    timed: &[(&Endpoint, i64)],
    given: (&str, &str),
) {
    let base = pascal(&svc.name);
    let (map, linked) = (fc.name("java.util.Map"), fc.name("java.util.LinkedHashMap"));
    let (list, instant) = (fc.name("java.util.List"), fc.name("java.time.Instant"));
    let (router, endpoint) = (fc.rt("Router"), fc.rt("ServerEndpoint"));
    let (due, built, spec) = (
        fc.rt("DueMoves"),
        fc.rt("HandlerHooks"),
        fc.rt("EndpointSpec"),
    );
    let ctx = fc.rt("RequestContext");
    let to_long = fc.name("java.util.function.ToLongFunction");
    let (given_params, pass) = given;
    let uses = conditional::service_uses(fc.tree.ir, svc);
    let args = if uses {
        format!("endpoints, {base}Handler.conditions()")
    } else {
        "endpoints".to_string()
    };
    let std = std_sigs(fc);
    doc_comment(
        w,
        Some(&format!(
            "The due-moves runner of the `{0}` service (`spec/platform.md` section 10.1): for each endpoint that takes a timed edge it asks the loader which values have waited long enough and runs every answer through steps 4 to 10 of the pipeline, in process, as a call of that endpoint. The caller is what `hooks.timer` answers (the policies judge it like any caller) and no middleware runs. One failure does not stop the others; it goes to `onError`.\n\nThe guard of a transition is not an atomic guarantee, and neither is a run: running it twice, or on two machines at once, is as safe as the handler's conditional write is.",
            svc.name
        )),
        &[],
        None,
    );
    w.line("@SuppressWarnings(\"deprecation\")");
    w.open(format!("public final class {base}DueMoves"));
    w.line(format!("private {base}DueMoves() {{}}"));
    w.blank();
    write_options(w, &base, &instant);
    w.blank();
    let ty = fc
        .tree
        .runtime_config
        .as_ref()
        .map(|i| fc.name(&i.type_fqn()));
    let report = format!("{map}<String, {due}.Entry>");
    let hooks_class = format!("{base}Hooks");
    if let Some(ty) = &ty {
        w.line("/** Runs the due moves. The runtime config is the one `create` takes (`null` means no value, which fails when a path is required). */");
        w.open(format!(
            "public static {report} run({given_params}, {hooks_class} hooks, Options options)"
        ));
        w.line(format!("return run({pass}, hooks, options, null);"));
        w.close();
        w.blank();
        w.line("/** Runs the due moves with a runtime config. */");
        w.open(format!(
            "public static {report} run({given_params}, {hooks_class} hooks, Options options, {ty} runtime)"
        ));
        w.line(format!("{ty} config = {ty}.ensure(runtime);"));
    } else {
        w.line("/** Runs the due moves: one call of the loader per timed endpoint, and a report of what became of the calls. */");
        w.open(format!(
            "public static {report} run({given_params}, {hooks_class} hooks, Options options)"
        ));
    }
    for (ep, _) in timed
        .iter()
        .filter(|(ep, _)| ep.auth.kind != veld_ir::AuthKind::None)
    {
        w.open("if (hooks.timerPrincipal() == null)");
        w.line(format!(
            "throw new IllegalStateException({});",
            java_str(&format!("{}: hooks.timer.principal is required", ep.name))
        ));
        w.close();
    }
    w.line(format!("{built} built = {};", build_call(fc)));
    w.line("long now = options.now == null ? hooks.clock().millis() : options.now.toEpochMilli();");
    w.line(format!("{to_long}<{ctx}> clock = ctx -> now;"));
    let emissions = if crate::outbox::service_emits(svc) {
        ", hooks.emissions()"
    } else {
        ""
    };
    w.line(format!(
        "{list}<{endpoint}> endpoints = {base}Handler.endpoints({pass}, built, clock{emissions});"
    ));
    let handler = fc.rt("VeldHandler");
    w.line(format!(
        "{handler} router = {router}.create({args}, {std}, built);"
    ));
    w.line(format!("{report} report = new {linked}<>();"));
    for (ep, ms) in timed {
        let request = RequestNames::new(fc.tree.ir, &fc.tree.services).get(svc.id, &ep.name);
        let method = method_name(ep);
        w.line(format!(
            "report.put({}, {due}.endpoint(router, built, specOf(endpoints, {}), now, {ms}L, options.limit, hooks.timerPrincipal(),",
            java_str(&ep.name),
            java_str(&ep.name)
        ));
        w.line(format!(
            "    (before, max, ctx) -> options.loaders.{method}(before, max, ctx).stream().map({request}::toWire).toList()));"
        ));
    }
    w.line("return report;");
    w.close();
    w.blank();
    w.open(format!(
        "private static {spec} specOf({list}<{endpoint}> endpoints, String name)"
    ));
    w.open(format!("for ({endpoint} e : endpoints)"));
    w.line("if (e.spec().name().equals(name)) { return e.spec(); }");
    w.close();
    w.line("throw new IllegalStateException(name + \": no such endpoint\");");
    w.close();
    w.close();
}

/// `Options`: the loaders, the clock of the run and the most calls per endpoint.
fn write_options(w: &mut CodeWriter, base: &str, instant: &str) {
    w.line("/** What a run needs: the loaders, and optionally the clock of the run and the most calls per endpoint. */");
    w.open("public static final class Options");
    w.line(format!("private final {base}DueLoaders loaders;"));
    w.line(format!("private {instant} now;"));
    w.line("private int limit = 100;");
    w.blank();
    w.line("/** Creates the options of a run. */");
    w.open(format!("public Options({base}DueLoaders loaders)"));
    w.line("this.loaders = java.util.Objects.requireNonNull(loaders, \"loaders\");");
    w.close();
    w.blank();
    w.line("/** The clock of the run, for every call: default the clock of the hooks. */");
    w.open(format!("public Options now({instant} value)"));
    w.line("this.now = value;");
    w.line("return this;");
    w.close();
    w.blank();
    w.line("/** The most calls per endpoint (default 100). */");
    w.open("public Options limit(int value)");
    w.line("this.limit = value;");
    w.line("return this;");
    w.close();
    w.close();
}

/// The `timer` hook of `<Service>Hooks`: the caller of a due move. `principal` is the typed
/// principal when the hooks are typed, else an `Object`; `complete` names the helper that
/// completes its derived fields.
pub fn write_hooks_timer(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    class: &str,
    principal: Option<&str>,
    completes: bool,
) {
    let due = fc.rt("DueMoves");
    let ty = principal.unwrap_or("Object");
    w.line("/** The caller of a due move: what `timer(...)` answers is the principal the policies judge and the handler sees. */");
    w.line("@FunctionalInterface");
    w.open("public interface TimerPrincipal");
    w.line(format!(
        "/** The principal of the timer. */\n  {ty} principal() throws Exception;"
    ));
    w.close();
    w.blank();
    w.line("private TimerPrincipal timer;");
    w.blank();
    w.line("/** Gives the due-moves runner its caller (the principal of the application's own timer). Without it a timed endpoint that has auth does not run. */");
    w.open(format!("public {class} timer(TimerPrincipal hook)"));
    w.line("this.timer = hook;");
    w.line("return this;");
    w.close();
    w.blank();
    w.open(format!("{due}.Principal timerPrincipal()"));
    w.line("TimerPrincipal hook = timer;");
    if completes {
        w.line("return hook == null ? null : () -> completePrincipal(hook.principal());");
    } else {
        w.line("return hook == null ? null : hook::principal;");
    }
    w.close();
    w.blank();
}
