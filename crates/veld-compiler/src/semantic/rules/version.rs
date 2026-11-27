//! The version guard of an endpoint (`spec/rules.md` sections 11.7 and 11.9): the first fact,
//! in declaration order, whose type is a model with a `@version` field.
//!
//! The guard is found twice, on purpose: the endpoint's implicit errors (`PreconditionFailed`,
//! `ValidationError`) are decided before the block is checked, because `require .. else
//! PreconditionFailed` must find them declared, and only this module needs the resolved type.

use crate::diagnostics::{Diagnostic, Span};
use crate::semantic::{DKind, Program};
use crate::syntax::ast::{self, Stmt};
use veld_ir::Type;

/// The checked guard: which fact is compared, and with which field of its model.
#[derive(Debug, Clone)]
pub struct RVersion {
    /// The fact, by name.
    pub fact: String,
    /// The version field by its source name.
    pub field: String,
    /// The same field by its wire name.
    pub wire: String,
    /// The fact's name in the block.
    pub span: Span,
}

/// The guard of the block of `e`, or `None` when no fact is a model with a version field. Types
/// that do not resolve are skipped: their own diagnostics come from the block check.
pub(crate) fn guard(prog: &Program<'_>, module: usize, e: &ast::Endpoint) -> Option<RVersion> {
    let stmts = e.block.as_ref().map_or(&[][..], |b| &b.stmts[..]);
    for s in stmts {
        let Stmt::Fact(f) = s else { continue };
        let mut scratch = Vec::new();
        let Some(Type::Decl(id)) = prog.resolve_type(module, &f.ty, false, &mut scratch) else {
            continue;
        };
        let decl = &prog.decls[id.index()];
        if decl.kind != DKind::Model {
            continue;
        }
        if let Some(field) = decl.res.fields.iter().find(|x| x.version) {
            return Some(RVersion {
                fact: f.name.name.clone(),
                field: field.name.clone(),
                wire: field.wire.clone(),
                span: f.name.span,
            });
        }
    }
    None
}

/// `E0920`: on an endpoint with a version guard, a parameter or a fact is not called `version`,
/// because the handler receives the version that was checked under that name (`spec/rules.md`
/// section 11.9), as it receives the transition under `transition`.
pub(super) fn reserved_name(
    prog: &Program<'_>,
    module: usize,
    e: &ast::Endpoint,
    diags: &mut Vec<Diagnostic>,
) {
    if guard(prog, module, e).is_none() {
        return;
    }
    let params = e.params.iter().map(|p| (&p.name, "parameter"));
    let facts = e
        .block
        .iter()
        .flat_map(|b| &b.stmts)
        .filter_map(|s| match s {
            Stmt::Fact(f) => Some((&f.name, "fact")),
            Stmt::Require(_) | Stmt::Ensure(_) | Stmt::Transition(_) | Stmt::Allow(_) => None,
        });
    for (name, what) in params.chain(facts).filter(|(n, _)| n.name == "version") {
        diags.push(
            Diagnostic::error(
                "E0920",
                format!("a {what} is called `version`, the name the handler gets the checked version under"),
                name.span,
            )
            .with_help("rename it: an endpoint with a version guard reserves the name"),
        );
    }
}
