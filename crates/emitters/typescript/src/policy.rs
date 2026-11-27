//! Policies (`spec/rules.md` section 11, `spec/targets/typescript.md` section 5e): one function
//! per policy that an `allow` of the tree's services names, in the client and the server output.
//!
//! A policy permits when at least one `permit` is true and no `forbid` is true; a clause that
//! cannot be evaluated fails closed (`veld.permits`, `veld.forbids` in the runtime). Each policy
//! has a boolean function that reads the system clock, and a verdict function that takes the
//! request's clock and also gives the message of the denial; the server calls the verdict.
//! Only the policies an `allow` names are generated.

use std::collections::{BTreeMap, BTreeSet};

use veld_codegen::naming::{self, Lang};
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_ir::{Expr, ExprKind, Policy, PolicyEffect, Root};

use crate::expr::{EndpointPrinter, EndpointScope};
use crate::tree::{FileCtx, Tree};
use crate::types::ts_type;
use crate::util::{add_file, doc_comment, ts_str};

/// Where the functions of one policy are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The root-relative module (`policies/bookings`).
    pub module: String,
    /// The boolean function; the verdict function is `<name>Verdict`.
    pub name: String,
}

/// The policies the tree's services use, by index into `Ir.policies`.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    entries: BTreeMap<usize, Entry>,
}

impl Plan {
    /// The policies named by an `allow` of an endpoint of the tree's services.
    #[must_use]
    pub fn new(tree: &Tree<'_>) -> Self {
        let mut used = BTreeSet::new();
        for sid in &tree.services {
            for ep in &tree.ir.service(*sid).endpoints {
                used.extend(ep.rules.allows.iter().map(|a| a.policy.index()));
            }
        }
        let mut taken: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut entries = BTreeMap::new();
        for i in used {
            let Some(policy) = tree.ir.policies.get(i) else {
                continue;
            };
            let module = tree.policy_module(policy);
            let names = taken.entry(module.clone()).or_default();
            let mut name = function_name(&policy.name);
            while names.contains(&name) || names.contains(&format!("{name}Verdict")) {
                name.push('_');
            }
            names.insert(name.clone());
            names.insert(format!("{name}Verdict"));
            entries.insert(i, Entry { module, name });
        }
        Self { entries }
    }

    /// Where the policy `index` of `Ir.policies` is generated.
    #[must_use]
    pub fn entry(&self, index: usize) -> Option<&Entry> {
        self.entries.get(&index)
    }
}

/// `manageBooking` for the policy `ManageBooking`.
fn function_name(policy: &str) -> String {
    let name = naming::escape(Lang::Ts, &naming::camel(policy));
    if matches!(name.as_str(), "veld" | "veldRules") {
        return format!("{name}_");
    }
    name
}

/// Identifiers for `names` that are valid, distinct and none of `reserved`.
#[must_use]
pub fn idents<'a>(names: impl IntoIterator<Item = &'a str>, reserved: &[&str]) -> Vec<String> {
    let mut used: BTreeSet<String> = reserved.iter().map(|r| (*r).to_owned()).collect();
    let mut out = Vec::new();
    for name in names {
        let mut ident: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        ident = naming::escape(Lang::Ts, &ident);
        while used.contains(&ident) {
            ident.push('_');
        }
        used.insert(ident.clone());
        out.push(ident);
    }
    out
}

/// Names a generated function must not use for its own locals.
pub const LOCALS: [&str; 3] = ["now", "veld", "veldRules"];

/// Writes the modules of the policies the tree's services use; returns their module paths.
///
/// # Errors
/// A generated file path that cannot be added (`E0805`).
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<Vec<String>, Vec<GenDiagnostic>> {
    if !tree.uses_policies() {
        return Ok(Vec::new());
    }
    let plan = Plan::new(tree);
    let mut by_module: BTreeMap<String, Vec<(usize, String)>> = BTreeMap::new();
    for (i, entry) in &plan.entries {
        by_module
            .entry(entry.module.clone())
            .or_default()
            .push((*i, entry.name.clone()));
    }
    let mut modules = Vec::new();
    for (module, policies) in by_module {
        let mut fc = FileCtx::new(tree, &module);
        let mut w = CodeWriter::new();
        for (n, (i, name)) in policies.iter().enumerate() {
            let Some(policy) = tree.ir.policies.get(*i) else {
                continue;
            };
            if n > 0 {
                w.blank();
            }
            if let Some(d) = flag_name_clash(policy) {
                return Err(vec![d]);
            }
            write_policy(&mut w, &mut fc, policy, name);
        }
        add_file(files, &format!("{module}.ts"), fc.finish(&w.finish()))?;
        modules.push(module);
    }
    Ok(modules)
}

/// A policy with a flagged clause takes `flags` as its last argument and reads `veldFlags`: a
/// parameter of either name would be shadowed, so it is refused (`E0801`).
fn flag_name_clash(policy: &Policy) -> Option<GenDiagnostic> {
    if !policy.clauses.iter().any(|c| c.flag.is_some()) {
        return None;
    }
    let clash = policy
        .params
        .iter()
        .find(|p| matches!(p.name.as_str(), "flags" | "veldFlags"))?;
    Some(GenDiagnostic::new(
        "E0801",
        format!(
            "policy `{}`: a parameter called `{}` clashes with the flags the generated function takes",
            policy.name, clash.name
        ),
    ))
}

fn write_policy(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, policy: &Policy, name: &str) {
    let veld = fc.rt("PolicyVerdict");
    let idents = idents(policy.params.iter().map(|p| p.name.as_str()), &LOCALS);
    let params: Vec<String> = policy
        .params
        .iter()
        .zip(&idents)
        .map(|(p, id)| format!("{id}: {}", ts_type(fc, &p.ty)))
        .collect();
    let names = idents.join(", ");
    let docs = policy
        .docs
        .clone()
        .unwrap_or_else(|| format!("The policy `{}`.", policy.name));
    doc_comment(
        w,
        Some(&format!(
            "{docs}\n\nWhether the policy permits, read at the system clock. A clause that cannot be evaluated fails closed."
        )),
        None,
    );
    // A policy with a flagged clause takes the caller's answers as a last argument.
    let flagged = policy.clauses.iter().any(|c| c.flag.is_some());
    let mut own = params.clone();
    if flagged {
        own.push("flags?: (name: string) => boolean".to_string());
    }
    w.open(format!(
        "export function {name}({}): boolean",
        own.join(", ")
    ));
    let call = if names.is_empty() {
        "BigInt(Date.now())".to_string()
    } else {
        format!("{names}, BigInt(Date.now())")
    };
    if flagged {
        let ns = fc.flags_ns();
        w.line(format!(
            "return {ns}.withAnswers(flags, () => {name}Verdict({call}).permitted);"
        ));
    } else {
        w.line(format!("return {name}Verdict({call}).permitted;"));
    }
    w.close();
    w.blank();
    w.line(format!(
        "/** The policy `{}` at the instant `now` (milliseconds since the epoch), with the message of the first `forbid` that was true. */",
        policy.name
    ));
    let mut with_now = params;
    with_now.push("now: bigint".to_string());
    w.open(format!(
        "export function {name}Verdict({}): {veld}",
        with_now.join(", ")
    ));
    write_clauses(w, fc, policy, &idents);
    w.close();
}

fn write_clauses(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, policy: &Policy, idents: &[String]) {
    let ns = fc.rules_ns();
    let scope = EndpointScope {
        locals: policy
            .params
            .iter()
            .zip(idents)
            .map(|(p, id)| (Root::Param(p.name.clone()), id.clone()))
            .collect(),
        ..EndpointScope::default()
    };
    let mut printer = EndpointPrinter::new(fc.tree.ir, &ns, scope);
    let (permits, forbids) = (fc.rt("permits"), fc.rt("forbids"));
    let mut permit_code = Vec::new();
    let mut forbids_seen = 0;
    for clause in &policy.clauses {
        let code = printer.print(&clause.expr);
        // A flagged clause exists only while its flag is on; a failing provider keeps a `forbid`
        // and drops a `permit`.
        let flag = clause.flag.as_deref().map(|f| (fc.flags_ns(), ts_str(f)));
        let on = |when_failed: bool| {
            flag.as_ref()
                .map(|(ns, name)| format!("{ns}.flagOn({name}, {when_failed})"))
        };
        match clause.effect {
            PolicyEffect::Permit => permit_code.push(match on(false) {
                Some(on) => format!("({on} && {permits}(() => {code}))"),
                None => format!("{permits}(() => {code})"),
            }),
            PolicyEffect::Forbid => {
                forbids_seen += 1;
                let test = match on(true) {
                    Some(on) => format!("{on} && {forbids}(() => {code})"),
                    None => format!("{forbids}(() => {code})"),
                };
                let mut message = clause
                    .message
                    .as_deref()
                    .map_or_else(String::new, |m| format!(", message: {}", ts_str(m)));
                // A catalog translates the message of a `forbid` by the key of that clause
                // (`spec/platform.md` section 22): the verdict carries it.
                if clause.message.is_some()
                    && fc.tree.uses_messages_server()
                    && let Some(key) = fc.tree.ir.violation_key(&policy.id, Some(forbids_seen))
                {
                    message.push_str(&format!(", key: {}", ts_str(&key)));
                }
                // Every `forbid` is asked first, in source order: the first one that is true ends
                // the verdict with its message.
                w.line(format!(
                    "if ({test}) return {{ permitted: false{message} }};"
                ));
            }
        }
    }
    let permitted = if permit_code.is_empty() {
        "false".to_string()
    } else {
        permit_code.join(" || ")
    };
    w.line(format!("return {{ permitted: {permitted} }};"));
}

/// The call of the verdict function of the policy an `allow` names, with its arguments in the
/// developer-facing representation. `None` (with a note in `unsupported`) when an argument cannot
/// be printed that way.
pub fn verdict_call(
    fc: &mut FileCtx<'_, '_>,
    printer: &mut EndpointPrinter<'_>,
    plan: &Plan,
    allow: &veld_ir::EndpointAllow,
    unsupported: &mut Vec<&'static str>,
) -> Option<String> {
    let Some(entry) = plan.entry(allow.policy.index()) else {
        unsupported.push("an `allow` of a policy it cannot find");
        return None;
    };
    let mut args = Vec::new();
    for arg in &allow.args {
        match app_value(fc, printer, arg) {
            Some(code) => args.push(code),
            None => unsupported.push("a duration as the argument of a policy"),
        }
    }
    args.push("now".to_string());
    let function = format!("{}Verdict", entry.name);
    fc.import(&entry.module, &function, true);
    Some(format!("{function}({})", args.join(", ")))
}

/// A value in the developer-facing representation: a plain path as it is, anything else computed
/// in the rule representation and converted back. `None` when it cannot be converted.
pub fn app_value(
    fc: &mut FileCtx<'_, '_>,
    printer: &mut EndpointPrinter<'_>,
    expr: &Expr,
) -> Option<String> {
    if let Some(raw) = printer.raw_path(expr) {
        return Some(raw);
    }
    let code = printer.print(expr);
    from_rule(fc, &code, &expr.t)
}

/// The names an expression reads, by root.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Roots {
    pub params: BTreeSet<String>,
    pub facts: BTreeSet<String>,
    pub principal: bool,
}

impl Roots {
    /// Adds what `expr` reads (inside `has(...)`, `old(...)` and comprehensions too).
    pub fn add(&mut self, expr: &Expr) {
        match &expr.e {
            ExprKind::Lit(_) | ExprKind::Now(_) => {}
            ExprKind::Path(p) | ExprKind::Has(p) => match &p.root {
                Root::Param(n) => {
                    self.params.insert(n.clone());
                }
                Root::Fact(n) => {
                    self.facts.insert(n.clone());
                }
                Root::Principal => self.principal = true,
                Root::SelfValue | Root::Result | Root::Var(_) => {}
            },
            ExprKind::Not(x) | ExprKind::Neg(x) | ExprKind::Widen(x) | ExprKind::Old(x) => {
                self.add(x);
            }
            ExprKind::Bin(b) => {
                self.add(&b.l);
                self.add(&b.r);
            }
            ExprKind::Call(c) => c.args.iter().for_each(|a| self.add(a)),
            ExprKind::Quant(q) => {
                self.add(&q.list);
                self.add(&q.body);
            }
            ExprKind::List(items) => items.iter().for_each(|a| self.add(a)),
        }
    }
}

use veld_ir::{ExprType, Scalar, Type};

/// Turns a value of the rule representation back into the developer-facing one.
fn from_rule(fc: &mut FileCtx<'_, '_>, code: &str, ty: &ExprType) -> Option<String> {
    match ty {
        ExprType::Duration => None,
        ExprType::Type(t) => from_rule_type(fc, code, t, 0),
    }
}

fn from_rule_type(fc: &mut FileCtx<'_, '_>, code: &str, t: &Type, depth: usize) -> Option<String> {
    let helper = match t {
        Type::Scalar(s) => match s {
            Scalar::Long => return Some(format!("String({code})")),
            Scalar::Decimal => "decimalText",
            Scalar::Date => "dateText",
            Scalar::Datetime => "datetimeText",
            Scalar::Time => "timeText",
            Scalar::Int
            | Scalar::Float
            | Scalar::Bool
            | Scalar::Json
            | Scalar::String
            | Scalar::Uuid
            | Scalar::Bytes => return Some(code.to_owned()),
        },
        Type::List(inner) => {
            let name = format!("e{depth}");
            let item = from_rule_type(fc, &name, inner, depth + 1)?;
            return Some(if item == name {
                code.to_owned()
            } else {
                format!("{code}.map(({name}) => {item})")
            });
        }
        Type::Map(_) | Type::Page(_) | Type::Decl(_) => return Some(code.to_owned()),
    };
    Some(format!("{}({code})", fc.rt(helper)))
}
