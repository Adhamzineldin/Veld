//! State machines and policies in the server module (`spec/rules.md` sections 10 and 11,
//! `spec/targets/typescript.md` sections 5d and 5e): the `transition` the handler receives, and the
//! `principal`, `allows` and `transition` members of the `rules` of an endpoint description. The
//! runtime (`runtime/veld.ts`, regions `policies` and `transitions`) runs them in step 6, and the
//! target check as the last `ensure` of step 9.

use veld_codegen::{CodeWriter, GenDiagnostic};
use veld_emit_typescript::expr::{EndpointPrinter, Principal};
use veld_emit_typescript::machine::Move;
use veld_emit_typescript::policy::{Plan, verdict_call};
use veld_emit_typescript::tree::FileCtx;
use veld_emit_typescript::types::ts_type;
use veld_emit_typescript::util::{doc_comment, prop_key, ts_str};
use veld_ir::{Endpoint, EndpointAllow, Root};

use crate::rules::{Sig, Site, unsupported};

/// What the doc comment of the `transition` the handler receives says.
const TRANSITION_NOTE: &str = "The move this call makes, as the server checked it before it called the handler: the wire path of the state field (`field`), the state it held (`from`) and the state this call moves it to (`to`).\n\nThe check is a guard, not an atomic guarantee: a second request can change the row after it. Apply the change with a conditional write (compare-and-swap or a row lock, in your own transaction) that only matches while the row still holds `from`, and answer `InvalidTransition` yourself when that write finds another value.";

/// What the doc comment of the `version` the handler receives says.
const VERSION_NOTE: &str = "The version the caller sent in `If-Match` and the server compared with the fact before it called the handler. It is absent when the header was absent or `*`, and when the guarded fact is absent.\n\nThe check is a guard, not an atomic guarantee: a second request can change the row after it. Write conditionally (compare-and-swap or a row lock, in your own transaction) so that the write only matches while the row still holds this version, and answer `PreconditionFailed` yourself when that write finds another value.";

/// The type of the `version` the handler of `request` receives: `RenameRoomVersion`.
#[must_use]
pub fn version_name(request: &str) -> String {
    format!(
        "{}Version",
        request.strip_suffix("Request").unwrap_or(request)
    )
}

/// `export interface RenameRoomVersion { version?: string; }`: what the handler gets of an
/// `If-Match` besides the parameters and the facts.
pub fn write_version_type(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    request: &str,
    ep: &Endpoint,
) {
    let Some(field) = veld_emit_typescript::version::guard_field(fc.tree.ir, ep) else {
        return;
    };
    let ty = ts_type(fc, &field.ty);
    w.line(format!(
        "/** The checked `If-Match` of `{}`, handed to the handler with the parameters and the facts. */",
        ep.name
    ));
    w.open(format!("export interface {}", version_name(request)));
    doc_comment(w, Some(VERSION_NOTE), None);
    w.line(format!("version?: {ty};"));
    w.close();
    w.blank();
}

/// The type of the `transition` the handler of `request` receives: `PayBookingTransition`.
#[must_use]
pub fn transition_name(request: &str) -> String {
    format!(
        "{}Transition",
        request.strip_suffix("Request").unwrap_or(request)
    )
}

/// `export interface PayBookingTransition { transition: veld.Transition<Status>; }`: what the
/// handler gets besides the parameters and the facts.
pub fn write_transition_type(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    request: &str,
    ep: &Endpoint,
) {
    let Some(t) = &ep.rules.transition else {
        return;
    };
    let machine = fc.type_ref(t.decl);
    let transition = fc.rt("Transition");
    w.line(format!(
        "/** The move of `{}`, handed to the handler with the parameters and the facts. */",
        ep.name
    ));
    w.open(format!("export interface {}", transition_name(request)));
    doc_comment(w, Some(TRANSITION_NOTE), None);
    w.line(format!("transition: {transition}<{machine}>;"));
    w.close();
    w.blank();
}

/// The declared prelude error `name`, as a class to take the `sig` of.
fn prelude_error(fc: &mut FileCtx<'_, '_>, site: &Site<'_>, name: &str) -> Option<String> {
    let id = site
        .ep
        .errors
        .iter()
        .map(|e| e.decl)
        .find(|d| fc.tree.ir.decl(*d).name == name)
        .or_else(|| fc.tree.prelude_decl(name))?;
    Some(fc.value_ref(id))
}

/// The type of the scope `f` the rules of an endpoint see: its facts, and the caller when the
/// rules read it (`{ readonly user: User }`).
pub fn scope_type(
    fc: &mut FileCtx<'_, '_>,
    facts: Option<String>,
    caller: Option<&Principal>,
) -> String {
    let own = facts.unwrap_or_default();
    let Some(caller) = caller else {
        return if own.is_empty() {
            "Record<string, never>".to_string()
        } else {
            own
        };
    };
    let user = format!(
        "{{ readonly {}: {} }}",
        prop_key(&caller.name),
        ts_type(fc, &caller.ty)
    );
    if own.is_empty() {
        user
    } else {
        format!("{own} & {user}")
    }
}

fn allow_step(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    printer: &mut EndpointPrinter<'_>,
    (site, sig, plan): (&Site<'_>, &Sig, &Plan),
    allow: &EndpointAllow,
    diags: &mut Vec<GenDiagnostic>,
) {
    let Some(policy) = fc.tree.ir.policies.get(allow.policy.index()) else {
        diags.push(unsupported(site, "an `allow` of a policy it cannot find"));
        return;
    };
    let mut notes = Vec::new();
    let call = verdict_call(fc, printer, plan, allow, &mut notes);
    diags.extend(notes.iter().map(|n| unsupported(site, n)));
    let (Some(call), Some(class)) = (call, prelude_error(fc, site, "Forbidden")) else {
        return;
    };
    w.open_with("", "{");
    w.line(format!("id: {},", ts_str(&policy.id)));
    let fields: Vec<String> = allow.reads.iter().map(|r| ts_str(r)).collect();
    w.line(format!("fields: [{}],", fields.join(", ")));
    w.line(format!(
        "verdict: ({}, {}, now: bigint) => {call},",
        sig.p, sig.f
    ));
    w.line(format!("otherwise: {{ sig: {class}.sig }},"));
    w.close_with("},");
}

fn version_step(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    site: &Site<'_>,
    diags: &mut Vec<GenDiagnostic>,
) {
    let Some(guard) = &site.ep.rules.version else {
        return;
    };
    let Some(class) = prelude_error(fc, site, "PreconditionFailed") else {
        diags.push(unsupported(
            site,
            "a version guard without `PreconditionFailed`",
        ));
        return;
    };
    w.open_with("version:", " {");
    w.line(format!("id: {},", ts_str(&guard.id)));
    w.line(format!("fact: {},", ts_str(&guard.fact)));
    w.line(format!("field: {},", ts_str(&guard.field)));
    w.line(format!("otherwise: {{ sig: {class}.sig }},"));
    w.close_with("},");
}

fn transition_step(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    printer: &EndpointPrinter<'_>,
    (site, sig): (&Site<'_>, &Sig),
    (mv, found): (&Move<'_>, Option<&str>),
    diags: &mut Vec<GenDiagnostic>,
) {
    let (Some(current), Some(class)) = (
        mv.current(printer),
        prelude_error(fc, site, "InvalidTransition"),
    ) else {
        diags.push(unsupported(site, "a transition path it cannot read"));
        return;
    };
    w.open_with("transition:", " {");
    w.line(format!("id: {},", ts_str(&mv.transition.id)));
    w.line(format!("field: {},", ts_str(&mv.field)));
    w.line(format!("sources: {},", mv.sources_array()));
    w.line(format!("to: {},", ts_str(&mv.target)));
    w.line(format!("current: ({}) => {current},", sig.f));
    if let Some(state) = found {
        let result = match &site.ep.success.ty {
            Some(t) => ts_type(fc, t),
            None => "undefined".to_string(),
        };
        w.line(format!("found: (result: {result}) => String({state}),"));
    }
    if !write_due(w, fc, printer, sig, mv) {
        diags.push(unsupported(site, "the due check of a timed transition"));
        return;
    }
    write_flagged_edges(w, mv);
    write_emits(w, fc, site, mv);
    w.line(format!("otherwise: {{ sig: {class}.sig }},"));
    w.close_with("},");
}

/// `flagged: { 'pending': 'flag' }`: the flag of each edge from a source to the target that has one
/// (`spec/platform.md` section 11), by wire value of the source.
fn write_flagged_edges(w: &mut CodeWriter, mv: &Move<'_>) {
    let t = mv.transition;
    let items: Vec<String> = t
        .from
        .iter()
        .zip(&mv.sources)
        .filter_map(|(name, wire)| {
            let flag = mv
                .machine
                .flows
                .iter()
                .find(|f| f.from == *name && f.to == t.to)?
                .flag
                .as_deref()?;
            Some(format!("{}: {}", prop_key(wire), ts_str(flag)))
        })
        .collect();
    if !items.is_empty() {
        w.line(format!("flagged: {{ {} }},", items.join(", ")));
    }
}

/// `emits: { edges, make }` of a move that emits events (`spec/events.md` section 11.1): the
/// events of the edge from each source, for the check after the handler, and the builder of the
/// `emit` the handler gets (in the events module of the service).
fn write_emits(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, site: &Site<'_>, mv: &Move<'_>) {
    if mv.transition.emits.is_empty() {
        return;
    }
    let module = crate::events::module_of(fc.tree, site.svc);
    let make = crate::outbox::make_name(site.request);
    fc.import(&module, &make, true);
    w.line(format!(
        "emits: {{ edges: {}, make: {make} }},",
        crate::outbox::edges(fc.tree.ir, mv)
    ));
}

/// `due: { afterMs, since }` of a timed transition (`spec/platform.md` section 10): the wait of
/// its edge and the reading of the `since` field.
fn write_due(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    printer: &EndpointPrinter<'_>,
    sig: &Sig,
    mv: &Move<'_>,
) -> bool {
    let t = mv.transition;
    if t.since.is_empty() {
        return true;
    }
    let after = t.from.first().and_then(|source| {
        mv.machine
            .flows
            .iter()
            .find(|f| f.from == *source && f.to == t.to)
            .and_then(|f| f.after_ms.as_deref())
    });
    let since = printer.field_code(&Root::Fact(t.fact.clone()), &t.since);
    let (Some(after), Some(since)) = (after, since) else {
        return false;
    };
    let ns = fc.rules_ns();
    w.open_with("due:", " {");
    w.line(format!("afterMs: {after}n,"));
    w.line(format!("since: ({}) => {ns}.toDatetime({since}),", sig.f));
    w.close_with("},");
    true
}

/// The members after `ensures` in the `rules` of an endpoint description: the caller's name in
/// the scope, the `allow` lines and the transition.
pub fn write_members(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    printer: &mut EndpointPrinter<'_>,
    (site, sig): (&Site<'_>, &Sig),
    (caller, mv, found): (Option<&Principal>, Option<&Move<'_>>, Option<&str>),
    diags: &mut Vec<GenDiagnostic>,
) {
    if let Some(caller) = caller {
        w.line(format!("principal: {},", ts_str(&caller.name)));
    }
    if !site.ep.rules.allows.is_empty() {
        let plan = Plan::new(fc.tree);
        let mut inner = CodeWriter::new();
        for allow in &site.ep.rules.allows {
            allow_step(&mut inner, fc, printer, (site, sig, &plan), allow, diags);
        }
        w.open_with("allows:", " [");
        for line in inner.finish().lines() {
            w.line(line);
        }
        w.close_with("],");
    }
    version_step(w, fc, site, diags);
    if let Some(mv) = mv {
        transition_step(w, fc, printer, (site, sig), (mv, found), diags);
    }
}

/// The target check of a transition, as the last `ensure` of the endpoint: the result holds the
/// target state. The message is the reference evaluator's; the runtime completes it with the
/// state the result held (``found `pending` ``).
pub fn write_target(w: &mut CodeWriter, sig: &Sig, result: &str, (mv, state): (&Move<'_>, &str)) {
    w.open_with("", "{");
    w.line(format!("id: {},", ts_str(&mv.transition.id)));
    w.line("name: 'transition',");
    w.line("fields: [],");
    w.line(format!(
        "message: {},",
        ts_str(&format!(
            "the result must hold the state `{}` in `{}`",
            mv.target, mv.field
        ))
    ));
    w.line(format!(
        "holds: ({}, result: {result}, now: bigint) => ({state}) === {},",
        sig.p,
        ts_str(&mv.target)
    ));
    w.close_with("},");
}
