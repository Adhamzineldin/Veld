//! What an expression reads below the first level of its declaration (`spec/rules.md` section
//! 17.2): the derived fields of nested values, which may have no value, and whether a `@readonly`
//! field is among them.
//!
//! A read is named by a **pattern**: the wire names from the declaration down, with `[]` for a
//! list (`total`, `customer.total`, `items[].total`). The runtime collects the patterns of the
//! derived fields that have no value in a value (`veld::rules::collect_absent`), and generated
//! code skips a rule or a derived field whose reads include one of them, as the reference
//! evaluator skips a read of a field that has no value.

use std::collections::BTreeSet;

use veld_ir::{Decl, Expr, ExprKind, ExprType, Ir, PathRef, Root, Type};

/// The result of [`reach`].
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Reach {
    /// The patterns of the derived fields that the expression reads, at any depth.
    pub patterns: BTreeSet<String>,
    /// The expression reads a `@readonly` field of a value below the first level.
    pub readonly_nested: bool,
}

/// Where a lambda variable comes from: the pattern of the list it ranges over (when that is a
/// path) and the type of its elements.
struct Var {
    name: String,
    pattern: Option<String>,
    ty: Option<Type>,
}

struct Walker<'a> {
    ir: &'a Ir,
    decl: &'a Decl,
    vars: Vec<Var>,
    out: Reach,
}

/// What the expression `expr`, an expression of the declaration `decl`, reads below its first
/// level.
#[must_use]
pub fn reach(ir: &Ir, decl: &Decl, expr: &Expr) -> Reach {
    let mut w = Walker {
        ir,
        decl,
        vars: Vec::new(),
        out: Reach::default(),
    };
    w.expr(expr);
    w.out
}

fn join(base: &str, wire: &str) -> String {
    if base.is_empty() {
        wire.to_string()
    } else {
        format!("{base}.{wire}")
    }
}

impl Walker<'_> {
    fn expr(&mut self, e: &Expr) {
        match &e.e {
            ExprKind::Path(p) => {
                self.path(p);
            }
            ExprKind::Lit(_) | ExprKind::Has(_) | ExprKind::Now(_) => {}
            ExprKind::Not(x) | ExprKind::Neg(x) | ExprKind::Widen(x) | ExprKind::Old(x) => {
                self.expr(x)
            }
            ExprKind::Bin(b) => {
                self.expr(&b.l);
                self.expr(&b.r);
            }
            ExprKind::Call(c) => c.args.iter().for_each(|a| self.expr(a)),
            ExprKind::List(items) => items.iter().for_each(|a| self.expr(a)),
            ExprKind::Quant(q) => {
                self.expr(&q.list);
                let (pattern, ty) = self.element_of(&q.list);
                self.vars.push(Var {
                    name: q.var.clone(),
                    pattern,
                    ty,
                });
                self.expr(&q.body);
                self.vars.pop();
            }
        }
    }

    /// The pattern and the element type of the list an expression stands for.
    fn element_of(&mut self, list: &Expr) -> (Option<String>, Option<Type>) {
        let ty = match &list.t {
            ExprType::Type(Type::List(t) | Type::Page(t)) => Some((**t).clone()),
            ExprType::Type(_) | ExprType::Duration => None,
        };
        let pattern = match &list.e {
            ExprKind::Path(p) => self.resolve(p).map(|(pattern, _)| format!("{pattern}[]")),
            ExprKind::Lit(_)
            | ExprKind::Now(_)
            | ExprKind::Not(_)
            | ExprKind::Neg(_)
            | ExprKind::Bin(_)
            | ExprKind::Has(_)
            | ExprKind::Call(_)
            | ExprKind::Quant(_)
            | ExprKind::List(_)
            | ExprKind::Widen(_)
            | ExprKind::Old(_) => None,
        };
        (pattern, ty)
    }

    /// Records the reads of a path and gives its pattern and type, when it can be named.
    fn path(&mut self, p: &PathRef) {
        let _ = self.resolve(p);
    }

    fn resolve(&mut self, p: &PathRef) -> Option<(String, Type)> {
        let (mut pattern, mut ty, mut nested) = match &p.root {
            Root::SelfValue => (Some(String::new()), Some(Type::Decl(self.decl.id)), false),
            Root::Var(name) => {
                let v = self.vars.iter().rev().find(|v| &v.name == name)?;
                (v.pattern.clone(), v.ty.clone(), true)
            }
            Root::Principal | Root::Result | Root::Param(_) | Root::Fact(_) => return None,
        };
        for step in &p.steps {
            let Some(Type::Decl(id)) = ty.clone() else {
                return None;
            };
            let field = self
                .ir
                .decl(id)
                .fields()
                .iter()
                .find(|f| f.name == step.name)?;
            pattern = pattern.map(|base| join(&base, &field.wire));
            if nested && field.readonly {
                self.out.readonly_nested = true;
            }
            let below = nested || pattern.as_deref().is_some_and(|s| s.contains('.'));
            if let (true, true, Some(pattern)) = (field.derived.is_some(), below, &pattern) {
                self.out.patterns.insert(pattern.clone());
            }
            ty = Some(field.ty.clone());
            nested = true;
        }
        Some((pattern?, ty?))
    }
}
