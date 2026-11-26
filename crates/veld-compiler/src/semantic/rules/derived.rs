//! Derived fields (`spec/rules.md` section 17.2): `total: decimal = subtotal - discount`.
//!
//! The scope is that of an invariant, so the expression goes through the same [`Checker`]. The
//! type of the expression must fit the type of the field (`E0902`), a derived field may read
//! other derived fields, and a cycle among them is `E0960`. The shape of the field itself
//! (required, no `@readonly`, no `@example`, `E0961`) is checked where the field is resolved.

use std::collections::{BTreeMap, BTreeSet};

use super::checker::Checker;
use super::{ancestors, ast_fields, depth, unresolved_fields};
use crate::diagnostics::{Diagnostic, Span, has_errors};
use crate::semantic::{DKind, Program, RField};
use crate::syntax::ast;
use veld_ir::rules::{Expr, ExprType};

/// A checked derived field of one declaration.
#[derive(Debug, Clone)]
pub struct RDerived {
    pub expr: Expr,
    /// Source names of the fields of the declaration the expression reads, first use first.
    pub reads: Vec<String>,
    pub span: Span,
}

/// The derived fields a declaration writes itself, by field name.
pub(super) fn check_derived(
    prog: &Program<'_>,
    i: usize,
    diags: &mut Vec<Diagnostic>,
) -> BTreeMap<String, RDerived> {
    let d = &prog.decls[i];
    let mut out = BTreeMap::new();
    if d.res.cyclic || !matches!(d.kind, DKind::Model | DKind::Error | DKind::Event) {
        return out;
    }
    let own: Vec<&ast::Field> = ast_fields(d.ast)
        .iter()
        .filter(|f| f.derived.is_some())
        .collect();
    if own.is_empty() {
        return out;
    }
    let poisoned = unresolved_fields(prog, i, &ancestors(prog, i));
    for f in &own {
        let Some(field) = d.res.own_fields.iter().find(|r| r.name == f.name.name) else {
            continue;
        };
        if let Some(r) = check_field(prog, i, f, field, &poisoned, diags) {
            out.insert(f.name.name.clone(), r);
        }
    }
    report_cycles(&own, &out, diags);
    out
}

fn check_field(
    prog: &Program<'_>,
    i: usize,
    f: &ast::Field,
    field: &RField,
    poisoned: &BTreeSet<String>,
    diags: &mut Vec<Diagnostic>,
) -> Option<RDerived> {
    let src = f.derived.as_ref()?;
    let d = &prog.decls[i];
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
    let want = ExprType::Type(field.ty.clone());
    let name = &f.name.name;
    let typed = c.expr(src, Some(&want)).and_then(|typed| {
        c.fit(typed, &want, src.span, |found, expected| {
            format!("derived field `{name}` is `{expected}`, its expression is `{found}`")
        })
    });
    let Some(expr) = typed else {
        // A derived field must never disappear quietly: without any error the IR would just
        // lower it to a field that nobody computes.
        if !has_errors(c.diags) {
            c.err(
                "E0902",
                format!("derived field `{name}` could not be typed"),
                src.span,
            );
        }
        return None;
    };
    if depth::json_depth(&expr) > depth::MAX_EXPR_JSON_DEPTH {
        c.err(
            "E0005",
            format!(
                "derived field `{name}` nests too deeply to be written as IR JSON (a type inside it is too deep)"
            ),
            src.span,
        );
        return None;
    }
    let reads = expr.self_reads();
    Some(RDerived {
        expr,
        reads,
        span: f.name.span,
    })
}

/// `E0960` once for every cycle of derived fields that read each other, at the first field of it.
fn report_cycles(
    own: &[&ast::Field],
    checked: &BTreeMap<String, RDerived>,
    diags: &mut Vec<Diagnostic>,
) {
    let edges = |name: &str| -> Vec<&str> {
        checked.get(name).map_or_else(Vec::new, |r| {
            r.reads
                .iter()
                .map(String::as_str)
                .filter(|n| checked.contains_key(*n))
                .collect()
        })
    };
    let mut reported: BTreeSet<&str> = BTreeSet::new();
    for f in own {
        let start = f.name.name.as_str();
        if reported.contains(start) || !checked.contains_key(start) {
            continue;
        }
        let Some(cycle) = cycle_through(start, &edges) else {
            continue;
        };
        reported.extend(cycle.iter().copied());
        let list = cycle
            .iter()
            .map(|n| format!("`{n}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut diag = Diagnostic::error(
            "E0960",
            format!("derived fields depend on each other in a cycle: {list}"),
            f.name.span,
        )
        .with_help(
            "a derived field may read other derived fields, but not itself, directly or not",
        );
        for other in own
            .iter()
            .filter(|o| o.name.name != start && cycle.contains(&o.name.name.as_str()))
        {
            diag = diag.with_label(other.name.span, "part of the cycle");
        }
        diags.push(diag);
    }
}

/// The names on a cycle that goes through `start`, in the order the walk meets them, or `None`.
fn cycle_through<'a>(
    start: &'a str,
    edges: &impl Fn(&str) -> Vec<&'a str>,
) -> Option<Vec<&'a str>> {
    let mut path = vec![start];
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    walk(start, start, edges, &mut path, &mut seen).then_some(path)
}

fn walk<'a>(
    start: &str,
    at: &'a str,
    edges: &impl Fn(&str) -> Vec<&'a str>,
    path: &mut Vec<&'a str>,
    seen: &mut BTreeSet<&'a str>,
) -> bool {
    for next in edges(at) {
        if next == start {
            return true;
        }
        if seen.insert(next) {
            path.push(next);
            if walk(start, next, edges, path, seen) {
                return true;
            }
            path.pop();
        }
    }
    false
}
