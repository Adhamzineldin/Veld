//! Policies (`spec/rules.md` section 11, `spec/targets/java.md` section 11): one class `Policies`
//! per Java package with a boolean method and a verdict method per policy, in the client and the
//! server output; the `allow` statements of an endpoint call the verdict method.
//!
//! A policy permits when at least one `permit` is true and no `forbid` is true; a clause that
//! cannot be evaluated fails closed (`runtime/Policy.java`). Only the policies that an `allow` of
//! the tree's services names are generated.

use std::collections::{BTreeMap, BTreeSet};

use veld_codegen::{CodeWriter, FileSet, GenDiagnostic, print_expr};
use veld_ir::{Expr, ExprKind, Policy, PolicyEffect, Root};

use crate::endpoint::{Site, list_of, prelude_error, unsupported};
use crate::rule_expr::{EndpointScope, JavaExpr, ScopeVar};
use crate::tree::{FileCtx, Tree, java_path};
use crate::types::java_type;
use crate::util::{add_file, doc_comment, java_str, member_ident, type_name, unique};

/// Where the methods of one policy are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The Java package of the class.
    pub package: String,
    /// The class (`Policies`).
    pub class: String,
    /// The boolean method; the verdict method is `<method>Verdict`.
    pub method: String,
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
        let mut by_package: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for i in used {
            if let Some(p) = tree.ir.policies.get(i) {
                by_package
                    .entry(tree.module_package(p.package, &p.module))
                    .or_default()
                    .push(i);
            }
        }
        let mut entries = BTreeMap::new();
        for (package, indexes) in by_package {
            let class = class_name(tree, &package);
            let names = unique(
                indexes
                    .iter()
                    .filter_map(|i| tree.ir.policies.get(*i))
                    .map(|p| member_ident(&p.name))
                    .collect(),
            );
            for (i, method) in indexes.into_iter().zip(names) {
                entries.insert(
                    i,
                    Entry {
                        package: package.clone(),
                        class: class.clone(),
                        method,
                    },
                );
            }
        }
        Self { entries }
    }

    /// Where the policy `index` of `Ir.policies` is generated.
    #[must_use]
    pub fn entry(&self, index: usize) -> Option<&Entry> {
        self.entries.get(&index)
    }
}

/// `Policies`, or `Policies_` when a declaration of the package already has that name.
fn class_name(tree: &Tree<'_>, package: &str) -> String {
    let taken = tree.decls().iter().any(|id| {
        let d = tree.ir.decl(*id);
        tree.decl_package(d) == package && type_name(d) == "Policies"
    });
    if taken { "Policies_" } else { "Policies" }.to_string()
}

/// Writes the `Policies` class of every package that has a used policy.
///
/// # Errors
/// `E0801` for a clause the printer cannot express.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<(), Vec<GenDiagnostic>> {
    if !tree.policies {
        return Ok(());
    }
    let plan = Plan::new(tree);
    let mut by_class: BTreeMap<(String, String), Vec<(usize, String)>> = BTreeMap::new();
    for (i, entry) in &plan.entries {
        by_class
            .entry((entry.package.clone(), entry.class.clone()))
            .or_default()
            .push((*i, entry.method.clone()));
    }
    for ((package, class), policies) in by_class {
        let mut fc = FileCtx::new(tree, &package, &class);
        let mut w = CodeWriter::new();
        let mut problems = Vec::new();
        write_class(&mut w, &mut fc, &class, &policies, &mut problems);
        if !problems.is_empty() {
            return Err(problems);
        }
        add_file(files, &java_path(&package, &class), fc.finish(&w.finish()))?;
    }
    Ok(())
}

fn write_class(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    class: &str,
    policies: &[(usize, String)],
    problems: &mut Vec<GenDiagnostic>,
) {
    doc_comment(
        w,
        Some(
            "The policies of the contract (spec/rules.md section 11). Each has a boolean method and a verdict method; the server calls the verdict method.",
        ),
        &[],
        None,
    );
    w.line("@SuppressWarnings(\"deprecation\")");
    w.open(format!("public final class {class}"));
    w.line(format!("private {class}() {{}}"));
    let ir = fc.tree.ir;
    for (i, method) in policies {
        let Some(policy) = ir.policies.get(*i) else {
            continue;
        };
        w.blank();
        write_policy(w, fc, policy, method, problems);
    }
    w.close();
}

/// The overload of the boolean policy function with an optional last argument, a function from a
/// flag name to a boolean (`spec/platform.md` section 11); the verdict function reads the view that
/// this puts in force, or the server's.
fn write_with_flags(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    policy: &Policy,
    method: &str,
    (params, idents): (&[String], &[String]),
    problems: &mut Vec<GenDiagnostic>,
) {
    if idents.iter().any(|i| i == "flags") {
        problems.push(GenDiagnostic {
            code: "E0801",
            message: format!(
                "policy `{}`: generator `java` cannot add the flags argument to `{method}`: a parameter is called `flags`",
                policy.name
            ),
            origin: Some(policy.origin.clone()),
        });
        return;
    }
    let (function, flags) = (fc.name("java.util.function.Function"), fc.rt("Flags"));
    let mut signature = params.join(", ");
    if !signature.is_empty() {
        signature.push_str(", ");
    }
    w.line(format!(
        "/** Like `{method}`, with the answers of `flags` for the flags of the clauses; without it the method above decides as a failing provider does. */"
    ));
    w.open(format!(
        "public static boolean {method}({signature}{function}<String, Boolean> flags)"
    ));
    w.line(format!(
        "return {flags}.withAnswers(flags, () -> {method}({}));",
        idents.join(", ")
    ));
    w.close();
    w.blank();
}

fn write_policy(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    policy: &Policy,
    method: &str,
    problems: &mut Vec<GenDiagnostic>,
) {
    let kit = fc.rt("Policy");
    let mut idents = unique(
        std::iter::once("now".to_string())
            .chain(policy.params.iter().map(|p| member_ident(&p.name)))
            .collect(),
    );
    let now = idents.remove(0);
    let params: Vec<String> = policy
        .params
        .iter()
        .zip(&idents)
        .map(|(p, id)| format!("{} {id}", java_type(fc, &p.ty, false)))
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
        &[],
        None,
    );
    w.open(format!(
        "public static boolean {method}({})",
        params.join(", ")
    ));
    let call_args = if names.is_empty() {
        "System.currentTimeMillis()".to_string()
    } else {
        format!("{names}, System.currentTimeMillis()")
    };
    w.line(format!("return {method}Verdict({call_args}).permitted();"));
    w.close();
    w.blank();
    if policy.clauses.iter().any(|c| c.flag.is_some()) {
        write_with_flags(w, fc, policy, method, (&params, &idents), problems);
    }
    w.line(format!(
        "/** The policy `{}` at the instant `now` (epoch milliseconds), with the message of the first `forbid` that was true. */",
        policy.name
    ));
    let mut with_now = params.clone();
    with_now.push(format!("long {now}"));
    w.open(format!(
        "public static {kit}.Verdict {method}Verdict({})",
        with_now.join(", ")
    ));
    write_clauses(w, fc, policy, &idents, &kit, problems);
    w.close();
}

fn write_clauses(
    w: &mut CodeWriter,
    fc: &mut FileCtx<'_, '_>,
    policy: &Policy,
    idents: &[String],
    kit: &str,
    problems: &mut Vec<GenDiagnostic>,
) {
    let scope = EndpointScope {
        params: policy
            .params
            .iter()
            .zip(idents)
            .map(|(p, id)| ScopeVar {
                name: p.name.clone(),
                code: id.clone(),
                ty: p.ty.clone(),
                optional: false,
            })
            .collect(),
        ..EndpointScope::default()
    };
    let mut printer = JavaExpr::for_endpoint(fc, scope);
    let mut permits = Vec::new();
    let mut forbids = Vec::new();
    for clause in &policy.clauses {
        let code = print_expr(&mut printer, &clause.expr).code;
        // A flagged `permit` grants and counts as off when the provider fails; a flagged `forbid`
        // restricts and counts as on.
        let code = match (&clause.flag, clause.effect) {
            (Some(f), PolicyEffect::Permit) => {
                let flags = printer.file().rt("Flags");
                format!("{flags}.on({}, false) && ({code})", crate::flags::quoted(f))
            }
            (Some(f), PolicyEffect::Forbid) => {
                let flags = printer.file().rt("Flags");
                format!("{flags}.on({}, true) && ({code})", crate::flags::quoted(f))
            }
            (None, _) => code,
        };
        match clause.effect {
            PolicyEffect::Permit => permits.push(code),
            PolicyEffect::Forbid => forbids.push((code, clause.message.clone())),
        }
    }
    for what in printer.unsupported() {
        problems.push(GenDiagnostic {
            code: "E0801",
            message: format!(
                "policy `{}` uses {what}, which generator `java` cannot enforce",
                policy.name
            ),
            origin: Some(policy.origin.clone()),
        });
    }
    for (code, message) in &forbids {
        let message = message
            .as_deref()
            .map_or_else(|| "null".to_string(), java_str);
        w.open(format!("if ({kit}.forbids(() -> {code}))"));
        w.line(format!("return {kit}.Verdict.denied({message});"));
        w.close();
    }
    let any: Vec<String> = permits
        .iter()
        .map(|c| format!("{kit}.permits(() -> {c})"))
        .collect();
    let permitted = if any.is_empty() {
        "false".to_string()
    } else {
        any.join(" || ")
    };
    w.line(format!("return {kit}.Verdict.of({permitted});"));
}

/// The `allow` statements of an endpoint (step 6, after the facts and before the transition
/// guard), in source order: the first denial ends the request with `403 Forbidden`.
pub fn write_allows(
    w: &mut CodeWriter,
    printer: &mut JavaExpr<'_, '_, '_>,
    site: &Site<'_>,
    diags: &mut Vec<GenDiagnostic>,
) {
    let tree = printer.file().tree;
    let plan = Plan::new(tree);
    for a in &site.ep.rules.allows {
        let Some(policy) = tree.ir.policies.get(a.policy.index()) else {
            diags.push(unsupported(site, "an `allow` of a policy it cannot find"));
            continue;
        };
        let Some(call) = verdict_call(printer, site, &plan, a, diags) else {
            continue;
        };
        let json = printer.file().rt("Json");
        let data = format!("{json}.object().put(\"message\", {json}.of(reason)).build()");
        let error = prelude_error(printer.file(), site, ("Forbidden", 403), &data);
        let fields = list_of(printer.file(), &a.reads);
        // An argument that has no value (a derived field of a request that reads a `@readonly`
        // field) denies the `allow`: a skipped `allow` would permit (`spec/rules.md` 17.2).
        let call = if a.args.iter().any(|x| arg_reaches_derived(tree.ir, x)) {
            let derive = printer.file().rt("Derive");
            let permitted = printer.file().rt("Policy");
            format!("{derive}.orSkip(() -> {call}, {permitted}.Verdict.of(false))")
        } else {
            call
        };
        w.line(format!(
            "run.allow({}, {fields}, () -> {call}, reason -> {error});",
            java_str(&policy.id)
        ));
    }
}

/// Whether the value of an `allow` argument can hold a derived field.
fn arg_reaches_derived(ir: &veld_ir::Ir, arg: &Expr) -> bool {
    match &arg.t {
        veld_ir::ExprType::Type(t) => ir.type_reaches_derived(t),
        veld_ir::ExprType::Duration => false,
    }
}

/// `Policies.manageBookingVerdict(args, now)`: the call of the verdict method of the policy an
/// `allow` names, with its arguments printed as values the developer sees.
pub fn verdict_call(
    printer: &mut JavaExpr<'_, '_, '_>,
    site: &Site<'_>,
    plan: &Plan,
    allow: &veld_ir::EndpointAllow,
    diags: &mut Vec<GenDiagnostic>,
) -> Option<String> {
    let Some(entry) = plan.entry(allow.policy.index()) else {
        diags.push(unsupported(site, "an `allow` of a policy it cannot find"));
        return None;
    };
    let mut args: Vec<String> = Vec::new();
    for arg in &allow.args {
        match printer.app_value(arg) {
            Some(code) => args.push(code),
            None => diags.push(unsupported(site, "a duration as the argument of a policy")),
        }
    }
    args.push("now".to_string());
    let class = printer
        .file()
        .name(&format!("{}.{}", entry.package, entry.class));
    Some(format!(
        "{class}.{}Verdict({})",
        entry.method,
        args.join(", ")
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"model User {
  id: uuid
  roles: string[]
}

principal User

policy Recent(user: User, since: datetime, limit: int) {
  permit when since < now
  permit when "admin" in user.roles
  forbid when limit > 3
    message "too many"
  forbid when limit < 0
}

policy Unused(user: User) {
  permit when true
}

@auth(bearer)
service S at "/s" {
  get "/" list() -> User {
    allow Recent(user, now, 2)
  }
}
"#;

    fn generated(policies: bool) -> (String, String) {
        let ir = veld_emit_conformance::compile("policy_unit", SOURCE);
        let services = ir.services.iter().map(|s| s.id).collect();
        let tree = Tree::new(&ir, "g", "g.runtime", services).with_policies(policies);
        let mut files = FileSet::new();
        emit(&mut files, &tree).unwrap();
        let paths: Vec<String> = files.paths().map(str::to_string).collect();
        let text = paths
            .iter()
            .find(|p| p.ends_with("/Policies.java"))
            .and_then(|p| files.get(p))
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_default();
        (paths.join(","), text)
    }

    #[test]
    fn a_policy_parameter_is_a_method_parameter_of_its_generated_type() {
        let (_, text) = generated(true);
        assert!(
            text.contains("public static boolean recent(User user, Instant since, int limit)"),
            "{text}"
        );
        assert!(
            text.contains(
                "public static Policy.Verdict recentVerdict(User user, Instant since, int limit, long now)"
            ),
            "{text}"
        );
        // A parameter is read by its name; a datetime is converted where it is read.
        assert!(
            text.contains("Policy.permits(() -> Rules.toDatetime(since) < now)"),
            "{text}"
        );
        assert!(text.contains("Rules.in(user.roles(), \"admin\")"), "{text}");
    }

    #[test]
    fn a_forbid_comes_first_with_its_message_and_a_policy_nobody_uses_is_not_generated() {
        let (_, text) = generated(true);
        let first = text.find("Policy.forbids(() -> limit > 3)").unwrap();
        let permit = text.find("Policy.permits(").unwrap();
        assert!(first < permit, "{text}");
        assert!(
            text.contains("return Policy.Verdict.denied(\"too many\");"),
            "{text}"
        );
        assert!(
            text.contains("return Policy.Verdict.denied(null);"),
            "{text}"
        );
        assert!(!text.contains("unused"), "{text}");
    }

    #[test]
    fn nothing_is_generated_when_the_contract_does_not_use_policies() {
        let (paths, text) = generated(false);
        assert!(text.is_empty() && !paths.contains("Policies"), "{paths}");
    }

    #[test]
    fn the_roots_an_expression_reads_are_collected() {
        let ir = veld_emit_conformance::compile("policy_unit", SOURCE);
        let allow = &ir.services[0].endpoints[0].rules.allows[0];
        let mut roots = Roots::default();
        allow.args.iter().for_each(|a| roots.add(a));
        assert!(roots.principal);
        assert!(roots.params.is_empty() && roots.facts.is_empty());
    }
}
