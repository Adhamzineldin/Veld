//! Policies and the principal (`spec/rules.md` sections 11.1 and 11.2): what the compiler
//! learns about them and the table that names them.
//!
//! A policy is named and imported like a declaration, but it is not one: it has no `DeclId` and
//! no type. Its names live in `Scope::policy_names`, beside the declarations of the module, and
//! the package-wide duplicate check of `collect` covers both.

use veld_ir::Type;
use veld_ir::rules::Expr;

use super::Program;
use crate::diagnostics::Span;
use crate::syntax::ast;

/// One `policy` declaration, once it is checked.
#[derive(Debug)]
pub struct PolicyInfo<'a> {
    pub pkg: usize,
    pub module: usize,
    pub name: String,
    /// `package:module.Name`, the `d` of the identity.
    pub qualified: String,
    pub ast: &'a ast::PolicyDecl,
    /// `None` when the policy has an error; every use of it then stays silent.
    pub res: Option<RPolicy>,
}

#[derive(Debug, Clone)]
pub struct RPolicy {
    pub params: Vec<RPolicyParam>,
    pub clauses: Vec<RClause>,
}

#[derive(Debug, Clone)]
pub struct RPolicyParam {
    pub name: String,
    pub ty: Type,
}

/// A checked `permit` or `forbid` clause.
#[derive(Debug, Clone)]
pub struct RClause {
    pub forbid: bool,
    pub expr: Expr,
    pub message: Option<String>,
    pub span: Span,
}

/// The principal of a package (`spec/rules.md` section 11.1).
#[derive(Debug, Clone)]
pub struct RPrincipal {
    /// The model, as an index into `Program::decls`.
    pub decl: usize,
    /// The name the caller has in endpoint blocks.
    pub name: String,
    pub span: Span,
}

/// A policy name in a scope: declared here, or imported.
#[derive(Debug, Clone, Copy)]
pub struct PolicyEntry {
    pub policy: usize,
    pub import: Option<usize>,
}

impl Program<'_> {
    /// Looks the policy `name` up as seen from `module`, marking its import as used.
    #[must_use]
    pub fn lookup_policy(&self, module: usize, name: &str) -> Option<usize> {
        let scope = self.scopes.get(module)?;
        let e = scope.policy_names.get(name)?;
        if let Some(i) = e.import {
            scope.imports[i].used.set(true);
        }
        Some(e.policy)
    }
}
