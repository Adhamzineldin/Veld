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
use veld_ir::{Endpoint, EndpointAllow};

use crate::rules::{Sig, Site, unsupported};

/// What the doc comment of the `transition` the handler receives says.
const TRANSITION_NOTE: &str = "The move this call makes, as the server checked it before it called the handler: the wire path of the state field (`field`), the state it held (`from`) and the state this call moves it to (`to`).\n\nThe check is a guard, not an atomic guarantee: a second request can change the row after it. Apply the change with a conditional write (compare-and-swap or a row lock, in your own transaction) that only matches while the row still holds `from`, and answer `InvalidTransition` yourself when that write finds another value.";

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
    w.line(format!("otherwise: {{ sig: {class}.sig }},"));
    w.close_with("},");
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
            allow_step(
                &mut inner,
                fc,
                printer,
                (site, sig, &plan),
                allow,
                diags,
            );
        }
        w.open_with("allows:", " [");
        for line in inner.finish().lines() {
            w.line(line);
        }
        w.close_with("],");
    }
    if let Some(mv) = mv {
        transition_step(w, fc, printer, (site, sig), (mv, found), diags);
    }
}

/// The target check of a transition, as the last `ensure` of the endpoint: the result holds the
/// target state. The message is the reference evaluator's; the runtime completes it with the
/// state the result held (``found `pending` ``).
pub fn write_target(
    w: &mut CodeWriter,
    sig: &Sig,
    result: &str,
    (mv, state): (&Move<'_>, &str),
) {
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
