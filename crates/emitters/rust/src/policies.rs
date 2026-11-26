//! Policies (`spec/rules.md` section 11), in the client and the server output: one function per
//! policy that an `allow` of the generated services names, in a module of its own next to the
//! policy's contract module, and `can_<endpoint>` in the client.
//!
//! A policy permits when at least one `permit` is true and no `forbid` is true. A clause that
//! cannot be evaluated fails closed: such a `permit` does not permit and such a `forbid` forbids.

use std::collections::{BTreeMap, BTreeSet};

use veld_codegen::{CodeWriter, FileSet, GenDiagnostic, print_expr};
use veld_ir::{
    Endpoint, EndpointFact, Expr, ExprKind, Param, Policy, PolicyEffect, Root, Service, Type,
};

use crate::guards::has_guards;
use crate::models::with_imports;
use crate::rules::{RustPrinter, Scope, local_ident};
use crate::tree::Tree;
use crate::types::{Ctx, rust_type};
use crate::util::{add_file, doc, lit, module, snake};

const ALLOW: &str = "#[allow(clippy::all, unused_imports, unused_variables, unused_mut, unused_parens, unused_braces, unused_comparisons)]";

/// Where the functions of one policy are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The package module the file is in.
    pub pkg: String,
    /// The module of the file: `<contract module>_policies`.
    pub file: String,
    /// The boolean function; the verdict function is `<func>_verdict`.
    pub func: String,
}

/// The policies that an `allow` of the tree's services names, by index into `Ir.policies`.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    entries: BTreeMap<usize, Entry>,
}

impl Plan {
    /// A policy that no `allow` of the tree's services names gets nothing.
    #[must_use]
    pub fn new(tree: &Tree<'_>) -> Self {
        let mut used = BTreeSet::new();
        for sid in &tree.services {
            for ep in &tree.ir.service(*sid).endpoints {
                used.extend(ep.rules.allows.iter().map(|a| a.policy.index()));
            }
        }
        let mut entries: BTreeMap<usize, Entry> = BTreeMap::new();
        let mut taken: BTreeSet<(String, String, String)> = BTreeSet::new();
        for i in used {
            let Some(p) = tree.ir.policies.get(i) else {
                continue;
            };
            let pkg = tree.pkg_mod(p.package);
            let file = file_module(tree, p);
            let base = snake(&p.name);
            let mut func = base.clone();
            while taken.contains(&(pkg.clone(), file.clone(), func.clone()))
                || taken.contains(&(pkg.clone(), file.clone(), format!("{func}_verdict")))
            {
                func.push('_');
            }
            taken.insert((pkg.clone(), file.clone(), func.clone()));
            taken.insert((pkg.clone(), file.clone(), format!("{func}_verdict")));
            entries.insert(i, Entry { pkg, file, func });
        }
        Self { entries }
    }

    /// The package and file modules to declare.
    #[must_use]
    pub fn modules(&self) -> BTreeSet<(String, String)> {
        self.entries
            .values()
            .map(|e| (e.pkg.clone(), e.file.clone()))
            .collect()
    }

    /// The path of the boolean function of policy `index`, as seen from anywhere in the crate.
    #[must_use]
    pub fn function_path(&self, tree: &Tree<'_>, index: usize) -> Option<String> {
        let e = self.entries.get(&index)?;
        Some(format!("{}::{}::{}::{}", tree.root, e.pkg, e.file, e.func))
    }

    /// The path of the verdict function of policy `index`.
    #[must_use]
    pub fn verdict_path(&self, tree: &Tree<'_>, index: usize) -> Option<String> {
        self.function_path(tree, index)
            .map(|p| format!("{p}_verdict"))
    }
}

/// `<contract module>_policies`, and a name that no contract module of the package has.
fn file_module(tree: &Tree<'_>, p: &Policy) -> String {
    let mut name = format!("{}_policies", module(&p.module));
    let taken: BTreeSet<String> = tree
        .decls()
        .iter()
        .map(|id| tree.ir.decl(*id))
        .filter(|d| d.package == p.package)
        .map(Tree::decl_mod)
        .collect();
    while taken.contains(&name) {
        name.push('_');
    }
    name
}

/// Writes one file for every contract module that has a used policy.
///
/// # Errors
/// `E0801` for a clause that the printer cannot express.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>) -> Result<(), Vec<GenDiagnostic>> {
    let plan = Plan::new(tree);
    let mut by_file: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
    for (i, e) in &plan.entries {
        by_file
            .entry((e.pkg.clone(), e.file.clone()))
            .or_default()
            .push(*i);
    }
    for ((pkg, file), indexes) in by_file {
        let here = format!("{}::{pkg}::{file}", tree.root);
        let ctx = Ctx { tree, here: &here };
        let mut w = CodeWriter::with_unit("    ");
        w.line("//! The policies of the contract (`spec/rules.md` section 11): a boolean function and a");
        w.line("//! verdict function for each policy that an `allow` names.")
            .blank();
        let mut problems = Vec::new();
        for i in indexes {
            if let (Some(p), Some(e)) = (tree.ir.policies.get(i), plan.entries.get(&i)) {
                write_policy(&mut w, &ctx, p, e, &mut problems);
            }
        }
        if !problems.is_empty() {
            return Err(problems);
        }
        add_file(
            files,
            &format!("{}/{pkg}/{file}.rs", tree.dir),
            with_imports(&w.finish()),
        )?;
    }
    Ok(())
}

fn policy_scope(p: &Policy) -> Scope {
    Scope {
        params: p.params.iter().map(|x| (x.name.clone(), false)).collect(),
        locals: true,
        ..Scope::default()
    }
}

fn write_policy(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    p: &Policy,
    e: &Entry,
    problems: &mut Vec<GenDiagnostic>,
) {
    let params: Vec<String> = p
        .params
        .iter()
        .map(|x| {
            format!(
                "{}: &{}",
                local_ident(&x.name),
                rust_type(ctx, &x.ty, false)
            )
        })
        .collect();
    let names: Vec<String> = p.params.iter().map(|x| local_ident(&x.name)).collect();
    doc(
        w,
        Some(
            p.docs
                .clone()
                .unwrap_or_else(|| format!("The policy `{}`.", p.name))
                .as_str(),
        ),
    );
    w.line("///");
    w.line("/// Whether the policy permits, at the system clock. A clause that cannot be evaluated fails");
    w.line("/// closed.");
    let flags = policy_flags(p);
    let answers = if flags.is_empty() {
        ""
    } else {
        ", flag_answers: Option<&dyn Fn(&str) -> bool>"
    };
    if !flags.is_empty() {
        w.line("/// `flag_answers` says which flags are on (`spec/platform.md` section 11); without it a clause");
        w.line("/// with a flag counts as it does when a provider fails: a `forbid` applies, a `permit` does not.");
    }
    w.line(ALLOW);
    w.open(format!(
        "pub fn {}({}{answers}) -> bool",
        e.func,
        params.join(", ")
    ));
    if flags.is_empty() {
        w.line(format!(
            "{}_verdict({}, veld::rules::system_now_ms()).is_ok()",
            e.func,
            names.join(", ")
        ));
    } else {
        let list: Vec<String> = flags.iter().map(|f| lit(f)).collect();
        w.line(format!(
            "veld::rules::with_view(veld::rules::answers(flag_answers, &[{}]).as_ref(), || {}_verdict({}, veld::rules::system_now_ms()).is_ok())",
            list.join(", "),
            e.func,
            names.join(", ")
        ));
    }
    w.close().blank();
    w.line(format!(
        "/// The policy `{}` at the instant `now` (milliseconds since the epoch): `Ok` when it permits,",
        p.name
    ));
    w.line("/// else the denial: the message of the first `forbid` that was true if it has one, otherwise");
    w.line(
        "/// `not permitted`. The server calls this form with the clock reading of the request.",
    );
    w.line(ALLOW);
    w.open(format!(
        "pub fn {}_verdict({}, now: i64) -> Result<(), String>",
        e.func,
        params.join(", ")
    ));
    w.line("use veld::rules as vr;");
    write_clauses(w, ctx, p, problems);
    w.close().blank();
}

/// The flags of the clauses of a policy, sorted.
pub(crate) fn policy_flags(p: &Policy) -> Vec<String> {
    let set: std::collections::BTreeSet<String> =
        p.clauses.iter().filter_map(|c| c.flag.clone()).collect();
    set.into_iter().collect()
}

fn write_clauses(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    p: &Policy,
    problems: &mut Vec<GenDiagnostic>,
) {
    let mut printer = RustPrinter::for_block(ctx, policy_scope(p));
    let mut permits = Vec::new();
    for clause in &p.clauses {
        let code = print_expr(&mut printer, &clause.expr).code;
        let test = |fallback: bool| {
            let test = format!("(|| -> vr::Res<bool> {{ Ok({code}) }})().unwrap_or({fallback})");
            // A flagged clause exists while its flag is on; a failing provider keeps a `forbid`
            // and drops a `permit` (`spec/platform.md` section 11).
            match &clause.flag {
                Some(flag) => format!("(vr::flag_on({}, {fallback}) && {test})", lit(flag)),
                None => test,
            }
        };
        match clause.effect {
            PolicyEffect::Forbid => {
                w.open(format!("if {}", test(true)));
                let message = clause.message.as_deref().unwrap_or("not permitted");
                w.line(format!("return Err({}.to_string());", lit(message)));
                w.close();
            }
            PolicyEffect::Permit => permits.push(test(false)),
        }
    }
    for what in &printer.unsupported {
        problems.push(GenDiagnostic {
            code: "E0801",
            message: format!(
                "generator `rust`: policy `{}` uses {what}, which it cannot enforce",
                p.name
            ),
            origin: Some(p.origin.clone()),
        });
    }
    if permits.is_empty() {
        w.line("Err(\"not permitted\".to_string())");
        return;
    }
    w.open(format!("if {}", permits.join(" || ")));
    w.line("Ok(())");
    w.close_with("} else {");
    w.indent();
    w.line("Err(\"not permitted\".to_string())");
    w.close();
}

// ---------------------------------------------------------------------------
// can_<endpoint>
// ---------------------------------------------------------------------------

/// Whether the client has a `can_<endpoint>` function for the endpoint.
#[must_use]
pub fn wants_can(ep: &Endpoint) -> bool {
    has_guards(ep)
}

/// The names an expression reads, by root.
#[derive(Debug, Default)]
struct Roots {
    params: BTreeSet<String>,
    facts: BTreeSet<String>,
    principal: bool,
}

impl Roots {
    fn add(&mut self, expr: &Expr) {
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

/// What `can_<endpoint>` takes, in the order of `spec/rules.md` section 11.4: the facts in
/// declaration order, the principal, the parameters in declaration order.
struct Inputs<'a> {
    facts: Vec<&'a EndpointFact>,
    principal: Option<(String, Type)>,
    params: Vec<&'a Param>,
}

fn inputs<'a>(svc: &Service, ep: &'a Endpoint) -> Inputs<'a> {
    let mut roots = Roots::default();
    for a in &ep.rules.allows {
        a.args.iter().for_each(|e| roots.add(e));
    }
    if let Some(t) = &ep.rules.transition {
        roots.facts.insert(t.fact.clone());
    }
    Inputs {
        facts: ep
            .rules
            .facts
            .iter()
            .filter(|f| roots.facts.contains(&f.name))
            .collect(),
        principal: svc
            .principal
            .as_ref()
            .filter(|_| roots.principal)
            .map(|p| (p.name.clone(), Type::Decl(p.decl))),
        params: ep
            .params
            .iter()
            .filter(|p| !p.stream && roots.params.contains(&p.name))
            .collect(),
    }
}

fn can_scope(given: &Inputs<'_>) -> Scope {
    Scope {
        params: given
            .params
            .iter()
            .map(|p| (p.name.clone(), p.optional))
            .collect(),
        facts: given
            .facts
            .iter()
            .map(|f| (f.name.clone(), f.optional))
            .collect(),
        principal: given.principal.as_ref().map(|(n, _)| local_ident(n)),
        locals: true,
        ..Scope::default()
    }
}

fn can_signature(ctx: &Ctx<'_, '_>, given: &Inputs<'_>) -> String {
    let opt = |t: String, optional: bool| {
        if optional { format!("Option<{t}>") } else { t }
    };
    let mut parts: Vec<String> = given
        .facts
        .iter()
        .map(|f| {
            format!(
                "{}: &{}",
                local_ident(&f.name),
                opt(rust_type(ctx, &f.ty, false), f.optional)
            )
        })
        .collect();
    if let Some((name, ty)) = &given.principal {
        parts.push(format!(
            "{}: &{}",
            local_ident(name),
            rust_type(ctx, ty, false)
        ));
    }
    parts.extend(given.params.iter().map(|p| {
        format!(
            "{}: &{}",
            local_ident(&p.name),
            opt(rust_type(ctx, &p.ty, false), p.optional)
        )
    }));
    parts.join(", ")
}

/// The flags `can_<endpoint>` reads: the clauses of its policies and the edges of its transition.
fn can_flags(ctx: &Ctx<'_, '_>, ep: &Endpoint) -> Vec<String> {
    let mut set = std::collections::BTreeSet::new();
    for a in &ep.rules.allows {
        if let Some(policy) = ctx.tree.ir.policies.get(a.policy.index()) {
            set.extend(policy_flags(policy));
        }
    }
    set.extend(crate::flags::transition_edge_flags(ctx.tree.ir, ep));
    set.into_iter().collect()
}

/// The name of `can_<endpoint>`: `can_pay_booking`.
#[must_use]
pub fn can_name(ep: &Endpoint) -> String {
    format!("can_{}", crate::util::bare(&snake(&ep.name)))
}

/// Writes `can_<endpoint>` of every endpoint that has an `allow` or a `transition`.
///
/// # Errors
/// `E0801` for an argument or a state the generator cannot express.
pub fn write_can_functions(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    svc: &Service,
) -> Result<(), Vec<GenDiagnostic>> {
    let plan = Plan::new(ctx.tree);
    for ep in svc.endpoints.iter().filter(|e| wants_can(e)) {
        write_can(w, ctx, svc, ep, &plan).map_err(|what| {
            vec![GenDiagnostic::new(
                "E0801",
                format!(
                    "generator `rust`: endpoint `{}.{}`: {what}",
                    svc.name, ep.name
                ),
            )]
        })?;
    }
    Ok(())
}

fn write_can(
    w: &mut CodeWriter,
    ctx: &Ctx<'_, '_>,
    svc: &Service,
    ep: &Endpoint,
    plan: &Plan,
) -> Result<(), String> {
    let given = inputs(svc, ep);
    let scope = can_scope(&given);
    let mut checks = Vec::new();
    for a in &ep.rules.allows {
        let call = plan
            .verdict_path(ctx.tree, a.policy.index())
            .ok_or("an `allow` names a policy that is not generated")?;
        let mut args = Vec::new();
        for e in &a.args {
            args.push(crate::guards::arg_ref(ctx, &scope, e)?);
        }
        args.push("now".to_string());
        checks.push(format!("{call}({}).is_ok()", args.join(", ")));
    }
    if let Some(t) = &ep.rules.transition {
        checks.push(crate::guards::transition_holds(ctx, ep, t)?);
    }
    w.blank();
    w.line(format!(
        "/// Whether the server would let a call of `{}` past the policies and the transition guard,",
        ep.name
    ));
    w.line("/// for these facts, this caller and these parameters. It reads the system clock, never sends");
    w.line("/// or rejects anything, and answers `false` when something cannot be evaluated: use it to show");
    w.line("/// or hide a button. The server decides.");
    w.line(ALLOW);
    let flags = can_flags(ctx, ep);
    let signature = can_signature(ctx, &given);
    let signature = if flags.is_empty() {
        signature
    } else {
        w.line("/// `flag_answers` says which flags are on; without it a flagged statement counts as it does");
        w.line("/// when a provider fails (`spec/platform.md` section 11).");
        if signature.is_empty() {
            "flag_answers: Option<&dyn Fn(&str) -> bool>".to_string()
        } else {
            format!("{signature}, flag_answers: Option<&dyn Fn(&str) -> bool>")
        }
    };
    w.open(format!("pub fn {}({signature}) -> bool", can_name(ep)));
    let body = if ep.rules.allows.is_empty() {
        checks.join(" && ")
    } else {
        w.line("use veld::rules as vr;");
        w.line("let now = vr::system_now_ms();");
        format!(
            "(|| -> veld::rules::Res<bool> {{ Ok({}) }})().unwrap_or(false)",
            checks.join(" && ")
        )
    };
    if flags.is_empty() {
        w.line(body);
    } else {
        let list: Vec<String> = flags.iter().map(|f| lit(f)).collect();
        w.line(format!(
            "veld::rules::with_view(veld::rules::answers(flag_answers, &[{}]).as_ref(), || {body})",
            list.join(", ")
        ));
    }
    w.close();
    Ok(())
}
