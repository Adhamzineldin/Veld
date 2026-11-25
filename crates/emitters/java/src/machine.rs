//! State machines in the server classes (`spec/rules.md` section 10, `spec/targets/java.md`
//! section 10): the transition guard of step 6, the `transition` value the handler receives and
//! the target check of step 9. The helpers of the enum itself are in `models.rs`.

use veld_codegen::{CodeWriter, GenDiagnostic};
use veld_ir::{DeclKind, EndpointTransition, Enum, Ir, Literal, Root, Step, Type};

use crate::endpoint::{Site, prelude_error, unsupported};
use crate::models::enum_constants;
use crate::rule_expr::JavaExpr;
use crate::tree::FileCtx;
use crate::util::java_str;

/// What the Javadoc of an endpoint with a `transition` says about the `transition` argument.
pub const TRANSITION_NOTE: &str = "The `transition` argument is what the server checked before it called this method: the field, the state it held (`from`) and the state this call moves it to (`to`). The check is a guard, not an atomic guarantee: a second request can change the row after it. Apply the change with a conditional write (compare-and-swap or a row lock, in your own transaction) that only matches while the row still holds `from`, and answer `InvalidTransition` yourself when that write finds another value.";

/// `Transition<Status>`: the type of the `transition` argument of a handler method.
pub fn transition_type(fc: &mut FileCtx<'_, '_>, t: &EndpointTransition) -> String {
    format!("{}<{}>", fc.rt("Transition"), fc.decl(t.decl))
}

/// `Status.PAID` for the variant `name` of the state machine `class`.
fn state_constant(e: &Enum, class: &str, name: &str) -> Option<String> {
    let consts = enum_constants(e);
    let i = e.variants.iter().position(|v| v.name == name)?;
    consts.get(i).map(|c| format!("{class}.{c}"))
}

/// The wire value of the variant `name`.
fn state_wire<'e>(e: &'e Enum, name: &str) -> Option<&'e str> {
    e.variants
        .iter()
        .find(|v| v.name == name)
        .map(|v| v.wire.as_str())
}

/// The wire path of the state field inside the fact: the wire names of the steps, joined by `.`.
fn field_path(t: &EndpointTransition) -> String {
    let wires: Vec<&str> = t.steps.iter().map(|s| s.wire.as_str()).collect();
    wires.join(".")
}

/// The default variant of the field the `steps` reach below a value of type `start`, if it has
/// one: a record leaves such a field `null` to mean "the default".
fn state_default(ir: &Ir, start: &Type, steps: &[Step]) -> Option<String> {
    let mut ty = start.clone();
    let mut default = None;
    for step in steps {
        let Type::Decl(id) = &ty else {
            return None;
        };
        let field = ir.decl(*id).fields().iter().find(|f| f.name == step.name)?;
        default = match &field.default {
            Some(Literal::EnumVariant { name, .. }) => Some(name.clone()),
            Some(_) | None => None,
        };
        ty = field.ty.clone();
    }
    default
}

/// The Java expression of the state a value holds: what the field reads, or its default when the
/// field is `null`.
fn state_read(read: &str, default: Option<String>, machine: &Enum, class: &str) -> String {
    match default.and_then(|d| state_constant(machine, class, &d)) {
        Some(d) => format!("{read} == null ? {d} : {read}"),
        None => read.to_string(),
    }
}

/// The guard of a `transition` (step 6, after the policies and before `require`): leaves the
/// `transition` value the handler receives.
pub fn write_guard(
    w: &mut CodeWriter,
    printer: &mut JavaExpr<'_, '_, '_>,
    site: &Site<'_>,
    diags: &mut Vec<GenDiagnostic>,
) {
    let Some(t) = &site.ep.rules.transition else {
        return;
    };
    let ir = printer.file().tree.ir;
    let DeclKind::Enum(machine) = &ir.decl(t.decl).kind else {
        diags.push(unsupported(
            site,
            "a transition on something that is not an enum",
        ));
        return;
    };
    let fact = site.ep.rules.facts.iter().find(|f| f.name == t.fact);
    let Some(fact) = fact.filter(|f| !f.optional) else {
        diags.push(unsupported(site, "a transition on an optional fact"));
        return;
    };
    let Some(read) = printer.field_code(Root::Fact(t.fact.clone()), &t.steps) else {
        diags.push(unsupported(site, "a transition path it cannot read"));
        return;
    };
    let class = printer.file().decl(t.decl);
    let default = state_default(ir, &fact.ty, &t.steps);
    let from = state_read(&read, default, machine, &class);
    let sources: Vec<String> = t
        .from
        .iter()
        .filter_map(|s| state_constant(machine, &class, s))
        .collect();
    let (Some(target), Some(to_wire)) = (
        state_constant(machine, &class, &t.to),
        state_wire(machine, &t.to),
    ) else {
        diags.push(unsupported(
            site,
            "a transition to a state the enum does not have",
        ));
        return;
    };
    let list = printer.file().name("java.util.List");
    let json = printer.file().rt("Json");
    let transition = transition_type(printer.file(), t);
    let data = format!(
        "{json}.object().put(\"field\", {json}.of({})).put(\"from\", {json}.of(transition_from.wire())).put(\"to\", {json}.of({})).build()",
        java_str(&field_path(t)),
        java_str(to_wire)
    );
    let error = prelude_error(printer.file(), site, ("InvalidTransition", 409), &data);
    w.line(format!("{class} transition_from = {from};"));
    w.line(format!(
        "{transition} transition = run.transition({}, {}, transition_from, {list}.of({}), {target}, () -> {error});",
        java_str(&t.id),
        java_str(&field_path(t)),
        sources.join(", ")
    ));
}

/// The transition part of `can<Endpoint>` in the client: a Java condition that is true when the
/// state the fact holds is one of the sources. `None` when the endpoint has no `transition`, or
/// (with a note) when it cannot be printed.
pub fn can_condition(
    printer: &mut JavaExpr<'_, '_, '_>,
    site: &Site<'_>,
    diags: &mut Vec<GenDiagnostic>,
) -> Option<String> {
    let t = site.ep.rules.transition.as_ref()?;
    let ir = printer.file().tree.ir;
    let DeclKind::Enum(machine) = &ir.decl(t.decl).kind else {
        diags.push(unsupported(
            site,
            "a transition on something that is not an enum",
        ));
        return None;
    };
    let fact = site.ep.rules.facts.iter().find(|f| f.name == t.fact);
    let Some(fact) = fact.filter(|f| !f.optional) else {
        diags.push(unsupported(site, "a transition on an optional fact"));
        return None;
    };
    let Some(read) = printer.field_code(Root::Fact(t.fact.clone()), &t.steps) else {
        diags.push(unsupported(site, "a transition path it cannot read"));
        return None;
    };
    let class = printer.file().decl(t.decl);
    let default = state_default(ir, &fact.ty, &t.steps);
    let from = state_read(&read, default, machine, &class);
    let sources: Vec<String> = t
        .from
        .iter()
        .filter_map(|s| state_constant(machine, &class, s))
        .collect();
    let list = printer.file().name("java.util.List");
    Some(format!(
        "{list}.of({}).contains({from})",
        sources.join(", ")
    ))
}

/// The target check (step 9, after the `ensure` statements): the same field of the result must
/// hold the target state. It exists only when the success type is the fact's own declaration.
pub fn write_target(
    w: &mut CodeWriter,
    printer: &mut JavaExpr<'_, '_, '_>,
    site: &Site<'_>,
    diags: &mut Vec<GenDiagnostic>,
) {
    let Some(t) = &site.ep.rules.transition else {
        return;
    };
    let fact_ty = site
        .ep
        .rules
        .facts
        .iter()
        .find(|f| f.name == t.fact)
        .map(|f| &f.ty);
    let same = matches!(
        (fact_ty, site.ep.success.ty.as_ref()),
        (Some(Type::Decl(a)), Some(Type::Decl(b))) if a == b
    );
    if !same || site.ep.success.stream {
        return;
    }
    let ir = printer.file().tree.ir;
    let DeclKind::Enum(machine) = &ir.decl(t.decl).kind else {
        return;
    };
    let Some(read) = printer.field_code(Root::Result, &t.steps) else {
        diags.push(unsupported(
            site,
            "a transition path it cannot read in the result",
        ));
        return;
    };
    let class = printer.file().decl(t.decl);
    let default = fact_ty.and_then(|ty| state_default(ir, ty, &t.steps));
    let has_default = default.is_some();
    let found = state_read(&read, default, machine, &class);
    let (Some(target), Some(to_wire)) = (
        state_constant(machine, &class, &t.to),
        state_wire(machine, &t.to),
    ) else {
        return;
    };
    let list = printer.file().name("java.util.List");
    let head = java_str(&format!(
        "the result must hold the state `{to_wire}` in `{}`, found `",
        field_path(t)
    ));
    let shown = if has_default {
        "transition_found.wire()"
    } else {
        "(transition_found == null ? \"null\" : transition_found.wire())"
    };
    w.line(format!("{class} transition_found = {found};"));
    w.line(format!(
        "run.ensure({}, \"transition\", {list}.of(), {head} + {shown} + \"`\", () -> transition_found == {target});",
        java_str(&t.id)
    ));
}
