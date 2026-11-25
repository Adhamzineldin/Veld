//! Pass 5: declaration-level type checks. E0404 discriminator clash and
//! E0407 required recursion (E0403/E0405/E0406 and default checks happen while
//! resolving).

use super::{DKind, Program, RField};
use crate::diagnostics::Diagnostic;
use veld_ir::Type;

pub fn check(prog: &mut Program<'_>, diags: &mut Vec<Diagnostic>) {
    check_discriminators(prog, diags);
    check_required_recursion(prog, diags);
}

fn check_discriminators(prog: &Program<'_>, diags: &mut Vec<Diagnostic>) {
    for u in prog.decls.iter().filter(|d| d.kind == DKind::Union) {
        for arm in &u.res.arms {
            let model = &prog.decls[arm.decl];
            if let Some(f) = model
                .res
                .fields
                .iter()
                .find(|f| f.wire == u.res.discriminator)
            {
                diags.push(
                    Diagnostic::error(
                        "E0404",
                        format!("arm model `{}` already has a field named `{}`, which is the discriminator of union `{}`", model.name, f.wire, u.name),
                        arm.span,
                    )
                    .with_label(f.span, "conflicting field")
                    .with_help("rename the field or choose another discriminator with `@discriminator(\"prop\")`"),
                );
            }
        }
    }
}

/// Declarations that a required field of `d` contains directly.
fn required_edges<'p>(prog: &'p Program<'_>, d: usize) -> Vec<(usize, &'p RField)> {
    let decl = &prog.decls[d];
    if !matches!(decl.kind, DKind::Model | DKind::Error | DKind::Event) {
        return Vec::new();
    }
    decl.res
        .fields
        .iter()
        .filter(|f| !f.optional)
        .filter_map(|f| match &f.ty {
            Type::Decl(t) if matches!(prog.decls[t.index()].kind, DKind::Model | DKind::Error) => {
                Some((t.index(), f))
            }
            Type::Decl(_) | Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) => None,
        })
        .collect()
}

fn reach_from(prog: &Program<'_>, start: usize) -> Vec<bool> {
    let mut seen = vec![false; prog.decls.len()];
    let mut stack: Vec<usize> = required_edges(prog, start)
        .into_iter()
        .map(|e| e.0)
        .collect();
    while let Some(n) = stack.pop() {
        if std::mem::replace(&mut seen[n], true) {
            continue;
        }
        stack.extend(required_edges(prog, n).into_iter().map(|e| e.0));
    }
    seen
}

fn check_required_recursion(prog: &Program<'_>, diags: &mut Vec<Diagnostic>) {
    let reach: Vec<Vec<bool>> = (0..prog.decls.len()).map(|i| reach_from(prog, i)).collect();
    for n in 0..prog.decls.len() {
        if !reach[n][n] {
            continue;
        }
        let smallest = (0..n).all(|m| !(reach[n][m] && reach[m][n]));
        if !smallest {
            continue;
        }
        let Some((target, field)) = required_edges(prog, n)
            .into_iter()
            .find(|(t, _)| *t == n || reach[*t][n])
        else {
            continue;
        };
        let path = cycle_path(prog, n, target, &reach);
        diags.push(
            Diagnostic::error(
                "E0407",
                format!("type `{}` contains itself through required fields: {}", prog.decls[n].name, path.join(" -> ")),
                field.span,
            )
            .with_note("no finite value of this type can exist")
            .with_help("break the cycle with an optional field (`x?: T`), a list (`T[]`), a map or a union arm"),
        );
    }
}

fn cycle_path(prog: &Program<'_>, n: usize, first: usize, reach: &[Vec<bool>]) -> Vec<String> {
    let mut path = vec![prog.decls[n].name.clone()];
    let mut cur = first;
    let mut guard = 0;
    while cur != n && guard < prog.decls.len() {
        path.push(prog.decls[cur].name.clone());
        let next = required_edges(prog, cur)
            .into_iter()
            .find(|(t, _)| *t == n || reach[*t][n])
            .map(|e| e.0);
        match next {
            Some(t) => cur = t,
            None => break,
        }
        guard += 1;
    }
    path.push(prog.decls[n].name.clone());
    path
}
