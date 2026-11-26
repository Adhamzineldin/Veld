//! The principal and policies (`spec/rules.md` sections 11.1 and 11.2): `E0940`, `E0941`,
//! `W0940` and the typing of every clause.
//!
//! A clause is checked by the checker of the invariants. A policy only changes what a name
//! means: its parameters are the roots (`Root::Param`), `now` is the clock, and nothing else is
//! in scope, so a fact or the principal named in a clause is an unknown name (`E0901`).

use std::collections::BTreeSet;

use super::checker::{Checker, EpScope, Phase};
use super::depth;
use crate::diagnostics::{Diagnostic, Span, has_errors};
use crate::semantic::{DKind, Program, RClause, RField, RPolicy, RPolicyParam, RPrincipal};
use veld_ir::Type;

/// The name of the principal when `principal` has no `as`.
const DEFAULT_NAME: &str = "user";

pub(in crate::semantic) fn check_policies(prog: &mut Program<'_>, diags: &mut Vec<Diagnostic>) {
    let principals = check_principals(prog, diags);
    prog.principals = principals;
    let results: Vec<Option<RPolicy>> = (0..prog.policies.len())
        .map(|i| check_policy(prog, i, diags))
        .collect();
    for (p, r) in prog.policies.iter_mut().zip(results) {
        p.res = r;
    }
}

// ---- the principal -------------------------------------------------------------------------

/// The principal of every package that has one. A second one is `E0941`.
fn check_principals(
    prog: &Program<'_>,
    diags: &mut Vec<Diagnostic>,
) -> std::collections::BTreeMap<usize, RPrincipal> {
    let mut found: std::collections::BTreeMap<usize, RPrincipal> = Default::default();
    for (mi, m) in prog.modules.iter().enumerate() {
        for p in &m.ast.principals {
            if let Some(first) = found.get(&m.pkg) {
                diags.push(
                    Diagnostic::error(
                        "E0941",
                        format!(
                            "package `{}` already declares a principal",
                            prog.packages[m.pkg].name
                        ),
                        p.span,
                    )
                    .with_label(first.span, "first declared here")
                    .with_help("a package has one caller type: keep one `principal`"),
                );
                continue;
            }
            if let Some(r) = principal(prog, mi, p, diags) {
                found.insert(m.pkg, r);
            }
        }
    }
    found
}

fn principal(
    prog: &Program<'_>,
    module: usize,
    p: &crate::syntax::ast::PrincipalDecl,
    diags: &mut Vec<Diagnostic>,
) -> Option<RPrincipal> {
    let ty = prog.resolve_type(module, &p.ty, true, diags)?;
    let decl = match &ty {
        Type::Decl(d) if prog.decls[d.index()].kind == DKind::Model => d.index(),
        Type::Decl(_) | Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) => {
            diags.push(
                Diagnostic::error(
                    "E0940",
                    format!(
                        "the principal must be a model, found `{}`",
                        prog.type_name(&ty)
                    ),
                    p.ty.span,
                )
                .with_help("declare the caller as a `model` and name it: `principal User`"),
            );
            return None;
        }
    };
    let name = p
        .name
        .as_ref()
        .map_or(DEFAULT_NAME, |n| n.name.as_str())
        .to_string();
    if name == "now" || name == "result" {
        let at = p.name.as_ref().map_or(p.span, |n| n.span);
        diags.push(
            Diagnostic::error(
                "E0920",
                format!("the principal cannot be called `{name}`: a name of every endpoint block"),
                at,
            )
            .with_help("name it with `as`: `principal User as caller`"),
        );
        return None;
    }
    Some(RPrincipal {
        decl,
        name,
        span: p.name.as_ref().map_or(p.span, |n| n.span),
    })
}

// ---- policies ------------------------------------------------------------------------------

fn param_field(name: &str, ty: &Type) -> RField {
    RField {
        name: name.to_string(),
        wire: name.to_string(),
        ty: ty.clone(),
        optional: false,
        default: None,
        readonly: false,
        version: false,
        pii: false,
        secret: false,
        inherited_from: None,
        constraints: veld_ir::Constraints::default(),
        example: None,
        docs: None,
        deprecated: None,
        span: Span::none(),
    }
}

fn check_policy(prog: &Program<'_>, i: usize, diags: &mut Vec<Diagnostic>) -> Option<RPolicy> {
    let info = &prog.policies[i];
    let (params, poisoned) = resolve_params(prog, i, diags);
    let mut ok = poisoned.is_empty();
    let fields: Vec<RField> = params.iter().map(|p| param_field(&p.name, &p.ty)).collect();
    let mut clauses = Vec::new();
    for clause in &info.ast.clauses {
        match check_clause(prog, i, clause, &fields, &poisoned, diags) {
            Some(c) => clauses.push(c),
            None => ok = false,
        }
    }
    if ok && !clauses.iter().any(|c| !c.forbid) {
        diags.push(
            Diagnostic::warning(
                "W0940",
                format!(
                    "policy `{}` has no `permit` clause and permits nobody",
                    info.name
                ),
                info.ast.name.span,
            )
            .with_help("add a `permit when ...` clause"),
        );
    }
    ok.then_some(RPolicy { params, clauses })
}

/// The typed parameters, and the names of those that did not resolve or repeat a name.
fn resolve_params(
    prog: &Program<'_>,
    i: usize,
    diags: &mut Vec<Diagnostic>,
) -> (Vec<RPolicyParam>, BTreeSet<String>) {
    let info = &prog.policies[i];
    let mut params: Vec<RPolicyParam> = Vec::new();
    let mut poisoned = BTreeSet::new();
    let mut seen: Vec<(&str, Span)> = Vec::new();
    for p in &info.ast.params {
        let name = p.name.name.clone();
        if let Some((_, first)) = seen.iter().find(|(n, _)| *n == name) {
            diags.push(
                Diagnostic::error(
                    "E0907",
                    format!(
                        "parameter `{name}` is already declared in policy `{}`",
                        info.name
                    ),
                    p.name.span,
                )
                .with_label(*first, "first declared here"),
            );
            poisoned.insert(format!("{name}#dup"));
            continue;
        }
        seen.push((&p.name.name, p.name.span));
        match prog.resolve_type(info.module, &p.ty, false, diags) {
            Some(ty) => params.push(RPolicyParam { name, ty }),
            None => {
                poisoned.insert(name);
            }
        }
    }
    (params, poisoned)
}

fn check_clause(
    prog: &Program<'_>,
    i: usize,
    clause: &crate::syntax::ast::PolicyClause,
    params: &[RField],
    poisoned: &BTreeSet<String>,
    diags: &mut Vec<Diagnostic>,
) -> Option<RClause> {
    let info = &prog.policies[i];
    let mut c = Checker {
        prog,
        module: info.module,
        fields: &[],
        poisoned: poisoned.clone(),
        diags,
        lambdas: Vec::new(),
        known: BTreeSet::new(),
        bounded: Vec::new(),
        reads: Vec::new(),
        reads_readonly: false,
        ep: Some(EpScope {
            params: params.to_vec(),
            facts: Vec::new(),
            result: None,
            phase: Phase::Require,
            in_old: false,
            lenient: false,
            principal: None,
        }),
    };
    let what = if clause.forbid {
        "a `forbid` clause"
    } else {
        "a `permit` clause"
    };
    let Some(expr) = c.bool_expr(&clause.expr, what) else {
        if !has_errors(c.diags) {
            c.err("E0902", "a clause could not be typed", clause.expr.span);
        }
        return None;
    };
    if depth::json_depth(&expr) > depth::MAX_EXPR_JSON_DEPTH {
        c.err(
            "E0005",
            "this clause nests too deeply to be written as IR JSON",
            clause.expr.span,
        );
        return None;
    }
    Some(RClause {
        forbid: clause.forbid,
        expr,
        message: clause.message.as_ref().map(|m| m.value.clone()),
        span: clause.span,
    })
}
