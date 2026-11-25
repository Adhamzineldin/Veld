//! Pass 6: invariants (`spec/rules.md` sections 3 to 5): name resolution, typing and widening,
//! the guard analysis for optional values, and the diagnostics `E0901` to `E0908` and
//! `W0901` to `W0903`. The pass stores the typed expression of every rule that a declaration
//! writes itself; lowering adds the inherited ones.

mod calls;
mod checker;
mod depth;
#[cfg(test)]
mod depth_tests;
mod endpoint;
#[cfg(test)]
mod endpoint_tests;
mod ops;
mod paths;
#[cfg(test)]
mod tests;
mod types;

use std::collections::BTreeSet;

use super::{DKind, Program, annotations};
use crate::diagnostics::{Diagnostic, Span, has_errors};
use crate::language::Target;
use crate::syntax::ast::{self, DeclKind, doc_text};
use checker::Checker;
pub(crate) use endpoint::{
    RBlock, RElse, check_block, check_service_facts, has_bare_require, has_required_fact,
};
use veld_ir::rules::Expr;

/// A checked rule of one declaration.
#[derive(Debug, Clone)]
pub struct RRule {
    pub name: String,
    pub expr: Expr,
    pub message: Option<String>,
    /// Source names of the fields of the declaration that the rule reads, first use first.
    pub reads: Vec<String>,
    pub reads_readonly: bool,
    pub docs: Option<String>,
    pub deprecated: Option<String>,
    pub span: Span,
}

/// How far up the `extends` chain the pass looks for inherited rule names.
const MAX_CHAIN: usize = 64;

pub fn check(prog: &mut Program<'_>, diags: &mut Vec<Diagnostic>) {
    let results: Vec<Vec<RRule>> = (0..prog.decls.len())
        .map(|i| check_decl(prog, i, diags))
        .collect();
    for (d, r) in prog.decls.iter_mut().zip(results) {
        d.res.own_rules = r;
    }
}

/// The rules a declaration writes, straight from the syntax.
pub(crate) fn ast_rules(d: &ast::Decl) -> &[ast::Rule] {
    match &d.kind {
        DeclKind::Model(m) => &m.rules,
        DeclKind::Error(e) => &e.rules,
        DeclKind::Enum(_) | DeclKind::Union(_) | DeclKind::Service(_) | DeclKind::Const(_) => &[],
    }
}

fn ast_fields(d: &ast::Decl) -> &[ast::Field] {
    match &d.kind {
        DeclKind::Model(m) => &m.fields,
        DeclKind::Error(e) => &e.fields,
        DeclKind::Enum(_) | DeclKind::Union(_) | DeclKind::Service(_) | DeclKind::Const(_) => &[],
    }
}

/// The `extends` chain of `i`, nearest parent first. Stops at a repeat or at [`MAX_CHAIN`].
pub(crate) fn ancestors(prog: &Program<'_>, i: usize) -> Vec<usize> {
    let mut chain = Vec::new();
    let mut cur = prog.decls[i].res.parent;
    while let Some(p) = cur {
        if p == i || chain.contains(&p) || chain.len() >= MAX_CHAIN {
            break;
        }
        chain.push(p);
        cur = prog.decls[p].res.parent;
    }
    chain
}

fn check_decl(prog: &Program<'_>, i: usize, diags: &mut Vec<Diagnostic>) -> Vec<RRule> {
    let d = &prog.decls[i];
    let rules = ast_rules(d.ast);
    if rules.is_empty() || d.res.cyclic || !matches!(d.kind, DKind::Model | DKind::Error) {
        return Vec::new();
    }
    let chain = ancestors(prog, i);
    let mut seen: Vec<(String, Span)> = Vec::new();
    for &a in chain.iter().rev() {
        let inherited = ast_rules(prog.decls[a].ast);
        seen.extend(inherited.iter().map(|r| (r.name.name.clone(), r.name.span)));
    }
    let poisoned = unresolved_fields(prog, i, &chain);
    let mut out = Vec::new();
    for rule in rules {
        if let Some((_, first)) = seen.iter().find(|(n, _)| n == &rule.name.name) {
            diags.push(
                Diagnostic::error(
                    "E0907",
                    format!(
                        "rule `{}` is already declared in `{}`, counting inherited rules",
                        rule.name.name, d.name
                    ),
                    rule.name.span,
                )
                .with_label(*first, "first declared here"),
            );
            continue;
        }
        seen.push((rule.name.name.clone(), rule.name.span));
        out.extend(check_rule(prog, i, rule, &poisoned, diags));
    }
    out
}

/// Fields written in the syntax of the declaration or an ancestor that did not resolve.
fn unresolved_fields(prog: &Program<'_>, i: usize, chain: &[usize]) -> BTreeSet<String> {
    let resolved: BTreeSet<&str> = prog.decls[i]
        .res
        .fields
        .iter()
        .map(|f| f.name.as_str())
        .collect();
    std::iter::once(i)
        .chain(chain.iter().copied())
        .flat_map(|d| ast_fields(prog.decls[d].ast))
        .map(|f| f.name.name.clone())
        .filter(|n| !resolved.contains(n.as_str()))
        .collect()
}

fn check_rule(
    prog: &Program<'_>,
    i: usize,
    rule: &ast::Rule,
    poisoned: &BTreeSet<String>,
    diags: &mut Vec<Diagnostic>,
) -> Option<RRule> {
    let d = &prog.decls[i];
    let anns = annotations::parse(&rule.annotations, Target::Rule, None, diags);
    let mut c = Checker {
        prog,
        module: d.module,
        fields: &d.res.fields,
        poisoned: poisoned.clone(),
        diags,
        lambdas: Vec::new(),
        known: BTreeSet::new(),
        bounded: Vec::new(),
        reads: Vec::new(),
        reads_readonly: false,
        ep: None,
    };
    let Some(expr) = c.bool_expr(&rule.expr, "a rule") else {
        // A rule must never disappear quietly: without any error the IR would just lack it.
        if !has_errors(c.diags) {
            c.err(
                "E0902",
                format!("rule `{}` could not be typed", rule.name.name),
                rule.expr.span,
            );
        }
        return None;
    };
    if depth::json_depth(&expr) > depth::MAX_EXPR_JSON_DEPTH {
        c.err(
            "E0005",
            format!(
                "rule `{}` nests too deeply to be written as IR JSON (a type inside it is too deep)",
                rule.name.name
            ),
            rule.expr.span,
        );
        return None;
    }
    let (reads, reads_readonly) = (c.reads, c.reads_readonly);
    if reads.is_empty() {
        diags.push(
            Diagnostic::warning(
                "W0903",
                format!("rule `{}` reads no field of `{}`", rule.name.name, d.name),
                rule.name.span,
            )
            .with_help("a rule that depends on no field is always true or always false"),
        );
    }
    Some(RRule {
        name: rule.name.name.clone(),
        expr,
        message: rule.message.as_ref().map(|m| m.value.clone()),
        reads,
        reads_readonly,
        docs: doc_text(&rule.docs),
        deprecated: anns.deprecated,
        span: rule.name.span,
    })
}
