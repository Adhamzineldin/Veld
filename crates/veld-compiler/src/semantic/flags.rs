//! Flags (`spec/platform.md` section 11): which names the workspace uses, and `E0938`, a
//! `given flag` in an example that names a flag no statement uses.

use std::collections::BTreeSet;

use super::Program;
use super::rules::ast_rules;
use crate::diagnostics::Diagnostic;
use crate::syntax::ast::{self, DeclKind, Stmt};

/// The flag names that some statement of the workspace uses: a rule, a `require`, an `ensure`, a
/// `permit` or `forbid` clause or a `flow`.
pub(super) fn used_flags(prog: &Program<'_>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for module in &prog.modules {
        let file = module.ast;
        for p in &file.policies {
            names.extend(
                p.clauses
                    .iter()
                    .filter_map(|c| c.flag.as_ref())
                    .map(|f| f.name.clone()),
            );
        }
        for d in &file.decls {
            names.extend(
                ast_rules(d)
                    .iter()
                    .filter_map(|r| r.flag.as_ref())
                    .map(|f| f.name.clone()),
            );
            match &d.kind {
                DeclKind::Enum(e) => names.extend(
                    e.flows
                        .iter()
                        .filter_map(|f| f.flag.as_ref())
                        .map(|f| f.name.clone()),
                ),
                DeclKind::Service(s) => block_flags(&s.endpoints, &mut names),
                DeclKind::Model(_)
                | DeclKind::Error(_)
                | DeclKind::Union(_)
                | DeclKind::Channel(_)
                | DeclKind::Const(_)
                | DeclKind::Event(_) => {}
            }
        }
    }
    names
}

fn block_flags(endpoints: &[ast::Endpoint], names: &mut BTreeSet<String>) {
    for stmt in endpoints
        .iter()
        .filter_map(|e| e.block.as_ref())
        .flat_map(|b| &b.stmts)
    {
        let flag = match stmt {
            Stmt::Require(r) => r.flag.as_ref(),
            Stmt::Ensure(e) => e.flag.as_ref(),
            Stmt::Fact(_) | Stmt::Transition(_) | Stmt::Allow(_) => None,
        };
        names.extend(flag.map(|f| f.name.clone()));
    }
}

/// The flags an example turns on, in order of the `given flag` lines: `off` takes a flag out
/// again, so the last line about a name decides. A name that no statement uses is `E0938`.
pub(super) fn example_flags(
    prog: &Program<'_>,
    ex: &ast::Example,
    diags: &mut Vec<Diagnostic>,
) -> Option<Vec<String>> {
    let wanted: Vec<&ast::Given> = ex.givens.iter().filter(|g| g.flag.is_some()).collect();
    if wanted.is_empty() {
        return Some(Vec::new());
    }
    let used = used_flags(prog);
    let mut on = BTreeSet::new();
    let mut ok = true;
    for g in wanted {
        if !used.contains(&g.name.name) {
            diags.push(
                Diagnostic::error(
                    "E0938",
                    format!("no statement uses the flag `{}`", g.name.name),
                    g.name.span,
                )
                .did_you_mean(
                    &g.name.name,
                    used.iter().map(String::as_str),
                    Some(format!(
                        "flags in use: {}",
                        used.iter().cloned().collect::<Vec<_>>().join(", ")
                    )),
                ),
            );
            ok = false;
        } else if g.flag == Some(true) {
            on.insert(g.name.name.clone());
        } else {
            on.remove(&g.name.name);
        }
    }
    ok.then(|| on.into_iter().collect())
}
