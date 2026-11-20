//! Pass 4: inheritance. E0401 redeclared inherited field, E0402 cycle.
//! Fields are flattened parent-first and `inherited_from` records the declarer.

use super::{DKind, Program, RField};
use crate::diagnostics::Diagnostic;

pub fn flatten(prog: &mut Program<'_>, diags: &mut Vec<Diagnostic>) {
    let n = prog.decls.len();
    let cyclic: Vec<bool> = (0..n).map(|i| in_or_below_cycle(prog, i, diags)).collect();
    let mut memo: Vec<Option<Vec<RField>>> = vec![None; n];
    for i in 0..n {
        compute(prog, i, &cyclic, &mut memo, diags);
    }
    for (i, fields) in memo.into_iter().enumerate() {
        prog.decls[i].res.fields = fields.unwrap_or_default();
        prog.decls[i].res.cyclic = cyclic[i];
    }
}

/// The longest `extends` chain the compiler flattens. Flattening recurses along the chain, so
/// the limit keeps the stack bounded.
const MAX_INHERITANCE_DEPTH: usize = 64;

/// True when the parent chain of `i` loops or is too long to flatten. Reports `E0402` for
/// members of a cycle and `E0006` for a chain over [`MAX_INHERITANCE_DEPTH`].
fn in_or_below_cycle(prog: &Program<'_>, i: usize, diags: &mut Vec<Diagnostic>) -> bool {
    if prog.decls[i].kind != DKind::Model {
        return false;
    }
    let mut chain = vec![i];
    let mut cur = prog.decls[i].res.parent;
    while let Some(p) = cur {
        if p == i {
            let names: Vec<&str> = chain
                .iter()
                .chain(std::iter::once(&i))
                .map(|c| prog.decls[*c].name.as_str())
                .collect();
            diags.push(Diagnostic::error(
                "E0402",
                format!("inheritance cycle: {}", names.join(" -> ")),
                prog.decls[i].name_span,
            ));
            return true;
        }
        if chain.contains(&p) {
            return true;
        }
        if chain.len() >= MAX_INHERITANCE_DEPTH {
            // One report per program: every model below the first over-long one is also over.
            if !diags.iter().any(|d| d.code == "E0006") {
                diags.push(Diagnostic::error(
                "E0006",
                format!(
                    "`{}` extends too many levels of models (the limit is {MAX_INHERITANCE_DEPTH})",
                    prog.decls[i].name
                ),
                prog.decls[i].name_span,
            ));
            }
            return true;
        }
        chain.push(p);
        cur = prog.decls[p].res.parent;
    }
    false
}

fn compute(
    prog: &Program<'_>,
    i: usize,
    cyclic: &[bool],
    memo: &mut Vec<Option<Vec<RField>>>,
    diags: &mut Vec<Diagnostic>,
) -> Vec<RField> {
    if let Some(done) = &memo[i] {
        return done.clone();
    }
    let d = &prog.decls[i];
    let own = d.res.own_fields.clone();
    let parent = d
        .res
        .parent
        .filter(|_| d.kind == DKind::Model && !cyclic[i]);
    let result = match parent {
        None => own,
        Some(p) => {
            let mut fields: Vec<RField> = compute(prog, p, cyclic, memo, diags)
                .into_iter()
                .map(|mut f| {
                    f.inherited_from.get_or_insert(p);
                    f
                })
                .collect();
            for f in own {
                if let Some(prev) = fields.iter().find(|x| x.name == f.name || x.wire == f.wire) {
                    let from = prev
                        .inherited_from
                        .map_or("parent", |q| prog.decls[q].name.as_str());
                    diags.push(
                        Diagnostic::error(
                            "E0401",
                            format!("field `{}` is already inherited from `{from}`", f.name),
                            f.span,
                        )
                        .with_label(prev.span, "inherited field declared here")
                        .with_help("inherited fields cannot be redeclared"),
                    );
                    continue;
                }
                fields.push(f);
            }
            fields
        }
    };
    memo[i] = Some(result.clone());
    result
}
