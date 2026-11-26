//! `@visible(when: e)` (`spec/platform.md` section 2): `E1101` and `W1101`, and the typing of
//! the expression.
//!
//! The scope of `e` is that of an invariant (the fields of the declaration, inherited ones
//! included) plus the caller, under the name the package's principal has, plus `now`. The
//! checker is the one of the endpoint blocks with the fields added to its scope, so `result`
//! and `old(...)` stay what they are everywhere else: refused.

use std::collections::BTreeSet;

use super::checker::{Checker, EpScope, Phase, PrincipalScope};
use super::{ast_fields, depth};
use crate::diagnostics::{Diagnostic, Span, has_errors};
use crate::semantic::{DKind, Program, RField};
use crate::syntax::ast;
use veld_ir::rules::Expr;
use veld_ir::{DeclId, Type};

/// A checked `@visible` of one field.
#[derive(Debug, Clone)]
pub struct RVisible {
    pub expr: Expr,
    /// The principal model of the package, as an index into `Program::decls`.
    pub principal: usize,
    pub span: Span,
}

/// The `@visible` annotations the declaration writes itself: field and expression.
fn own_visible<'a>(prog: &'a Program<'_>, i: usize) -> Vec<(&'a ast::Field, &'a ast::Expr, Span)> {
    let d = &prog.decls[i];
    ast_fields(d.ast)
        .iter()
        .filter_map(|f| {
            let a = f.annotations.iter().find(|a| a.name.name == "visible")?;
            a.when.as_ref().map(|e| (f, e, a.span))
        })
        .collect()
}

/// Checks every `@visible` of the workspace and stores the typed expressions in the declaration
/// that wrote them. Runs after the principals are known.
pub(in crate::semantic) fn check(prog: &mut Program<'_>, diags: &mut Vec<Diagnostic>) {
    let mut found = Vec::new();
    for i in 0..prog.decls.len() {
        let mut out = std::collections::BTreeMap::new();
        for (f, e, at) in own_visible(prog, i) {
            if let Some(r) = check_field(prog, i, f, e, at, diags) {
                out.insert(f.name.name.clone(), r);
            }
        }
        found.push(out);
    }
    for (d, r) in prog.decls.iter_mut().zip(found) {
        d.res.own_visible = r;
    }
    check_events(prog, diags);
}

fn err(diags: &mut Vec<Diagnostic>, msg: String, span: Span, help: &str) {
    diags.push(Diagnostic::error("E1101", msg, span).with_help(help.to_string()));
}

fn check_field(
    prog: &Program<'_>,
    i: usize,
    f: &ast::Field,
    src: &ast::Expr,
    at: Span,
    diags: &mut Vec<Diagnostic>,
) -> Option<RVisible> {
    let d = &prog.decls[i];
    let name = &f.name.name;
    if d.kind == DKind::Event {
        err(
            diags,
            format!("field `{name}` of an event cannot be `@visible`"),
            at,
            "a published event has no caller to decide for: use another model for the event",
        );
        return None;
    }
    if !f.optional {
        err(
            diags,
            format!("`@visible` field `{name}` must be optional"),
            at,
            "a client already handles an absent optional field, so hiding it breaks no caller: write `name?: T`",
        );
        return None;
    }
    let Some(principal) = prog.principals.get(&d.pkg) else {
        err(
            diags,
            format!(
                "`@visible` needs a caller, and package `{}` declares no principal",
                prog.packages[d.pkg].name
            ),
            at,
            "declare one: `principal User`",
        );
        return None;
    };
    let clash = d
        .res
        .fields
        .iter()
        .find(|x| x.name == principal.name || x.name == "now");
    if let Some(x) = clash {
        diags.push(
            Diagnostic::error(
                "E0920",
                format!(
                    "field `{}` has the name of the caller or of `now`, which `@visible` reads",
                    x.name
                ),
                at,
            )
            .with_label(x.span, "the field")
            .with_help("name the principal with `as`: `principal User as caller`"),
        );
        return None;
    }
    type_expression(prog, i, name, src, principal, diags)
}

fn type_expression(
    prog: &Program<'_>,
    i: usize,
    name: &str,
    src: &ast::Expr,
    principal: &crate::semantic::RPrincipal,
    diags: &mut Vec<Diagnostic>,
) -> Option<RVisible> {
    let d = &prog.decls[i];
    let mut c = Checker {
        prog,
        module: d.module,
        fields: &d.res.fields,
        poisoned: BTreeSet::new(),
        diags,
        lambdas: Vec::new(),
        known: BTreeSet::new(),
        bounded: Vec::new(),
        reads: Vec::new(),
        reads_readonly: false,
        ep: Some(EpScope {
            params: Vec::new(),
            facts: Vec::new(),
            result: None,
            phase: Phase::Require,
            in_old: false,
            lenient: false,
            principal: Some(PrincipalScope {
                field: principal_field(&principal.name, principal.decl),
                available: true,
            }),
            fields_in_scope: true,
        }),
    };
    let Some(expr) = c.bool_expr(src, "`@visible(when: …)`") else {
        if !has_errors(c.diags) {
            c.err(
                "E0902",
                format!("`@visible` of `{name}` could not be typed"),
                src.span,
            );
        }
        return None;
    };
    if depth::json_depth(&expr) > depth::MAX_EXPR_JSON_DEPTH {
        c.err(
            "E0005",
            format!("`@visible` of `{name}` nests too deeply to be written as IR JSON"),
            src.span,
        );
        return None;
    }
    Some(RVisible {
        expr,
        principal: principal.decl,
        span: src.span,
    })
}

fn principal_field(name: &str, decl: usize) -> RField {
    super::endpoint::scope_field(
        name,
        &Type::Decl(DeclId(u32::try_from(decl).unwrap_or(u32::MAX))),
        false,
        veld_ir::Constraints::default(),
    )
}

/// The declarations a type reaches, with unions and everything a declaration's fields reach.
pub(super) fn closure(prog: &Program<'_>, start: Vec<usize>) -> Vec<usize> {
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    let mut stack = start;
    while let Some(i) = stack.pop() {
        if !seen.insert(i) {
            continue;
        }
        let d = &prog.decls[i];
        let mut ids = Vec::new();
        for f in &d.res.fields {
            f.ty.collect_decls(&mut ids);
        }
        stack.extend(ids.into_iter().map(DeclId::index));
        stack.extend(d.res.arms.iter().map(|a| a.decl));
    }
    seen.into_iter().collect()
}

/// `E1101`: an event must not reach a field with `@visible`.
fn check_events(prog: &Program<'_>, diags: &mut Vec<Diagnostic>) {
    for (i, d) in prog.decls.iter().enumerate() {
        if d.kind != DKind::Event {
            continue;
        }
        // The event's own fields are reported where they are written.
        let mut ids = Vec::new();
        for f in &d.res.fields {
            f.ty.collect_decls(&mut ids);
        }
        let reached = closure(prog, ids.into_iter().map(DeclId::index).collect());
        let hit = reached.iter().filter(|r| **r != i).find_map(|r| {
            prog.decls[*r]
                .res
                .fields
                .iter()
                .find(|f| f.visible)
                .map(|f| (*r, f))
        });
        if let Some((r, f)) = hit {
            diags.push(
                Diagnostic::error(
                    "E1101",
                    format!(
                        "event `{}` reaches `{}.{}`, which is `@visible`",
                        d.name, prog.decls[r].name, f.name
                    ),
                    d.name_span,
                )
                .with_label(f.span, "the field")
                .with_help("a published event has no caller to decide for: use another model for the event"),
            );
        }
    }
}

/// `W1101`: an endpoint that returns a `@visible` field and has no caller of that principal
/// type, so the field is never sent there.
pub(in crate::semantic) fn check_endpoints(prog: &Program<'_>, diags: &mut Vec<Diagnostic>) {
    for s in &prog.services {
        let own = prog.principals.get(&s.pkg).map(|p| p.decl);
        for ep in &s.endpoints {
            let mut ids = Vec::new();
            for ty in ep.success_ty.iter() {
                ty.collect_decls(&mut ids);
            }
            ids.extend(
                ep.errors
                    .iter()
                    .map(|(_, d)| DeclId(u32::try_from(*d).unwrap_or(u32::MAX))),
            );
            let reached = closure(prog, ids.into_iter().map(DeclId::index).collect());
            let hit = reached.iter().find_map(|r| {
                let d = &prog.decls[*r];
                let field = d.res.fields.iter().find(|f| f.visible)?;
                let theirs = prog.principals.get(&d.pkg).map(|p| p.decl);
                let has_caller =
                    ep.auth.kind != veld_ir::AuthKind::None && own.is_some() && own == theirs;
                (!has_caller).then_some((d, field))
            });
            if let Some((d, f)) = hit {
                diags.push(
                    Diagnostic::warning(
                        "W1101",
                        format!(
                            "`{}.{}` is `@visible` and endpoint `{}` has no caller of its principal type: the field is never sent here",
                            d.name, f.name, ep.name
                        ),
                        ep.span,
                    )
                    .with_help("give the endpoint `@auth` of the package's principal, or return a model without the field"),
                );
            }
        }
    }
}
