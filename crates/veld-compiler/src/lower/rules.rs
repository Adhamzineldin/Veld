//! Lowering of checked rules to `veld_ir::Rule` (`spec/rules.md` section 13).
//!
//! The checker stores expressions with the declaration indices of the analysed program. Here
//! every index is remapped to the final `DeclId`. Rule ids are filled in afterwards by
//! `signature::rule_ids`, once every declaration has its place in the IR.

use super::Lowerer;
use crate::semantic::rules::ancestors;
use veld_ir::rules::{
    BinExpr, CallExpr, EnumLit, Expr, ExprKind, ExprLit, ExprType, QuantExpr, Rule,
};

impl Lowerer<'_, '_> {
    /// The rules of declaration `old`: inherited ones first (root-most ancestor first), then
    /// its own.
    pub(super) fn rules_of(&self, old: usize) -> Vec<Rule> {
        let mut chain = ancestors(self.prog, old);
        chain.reverse();
        chain.push(old);
        let mut out = Vec::new();
        for decl in chain {
            let info = &self.prog.decls[decl];
            for r in &info.res.own_rules {
                out.push(Rule {
                    flag: r.flag.clone(),
                    name: r.name.clone(),
                    id: String::new(),
                    expr: self.remap_expr(&r.expr),
                    message: r.message.clone(),
                    reads: r.reads.clone(),
                    reads_readonly: r.reads_readonly,
                    inherited_from: (decl != old).then(|| self.decl_id(decl)),
                    docs: r.docs.clone(),
                    deprecated: r.deprecated.clone(),
                    rollout: r.rollout,
                    origin: self.origin(r.span, info.module),
                });
            }
        }
        out
    }

    fn remap_expr_type(&self, t: &ExprType) -> ExprType {
        match t {
            ExprType::Type(t) => ExprType::Type(self.remap(t)),
            ExprType::Duration => ExprType::Duration,
        }
    }

    fn remap_exprs(&self, items: &[Expr]) -> Vec<Expr> {
        items.iter().map(|e| self.remap_expr(e)).collect()
    }

    /// Replaces every declaration index inside `e` with the final `DeclId`.
    pub(super) fn remap_expr(&self, e: &Expr) -> Expr {
        let b = |x: &Expr| Box::new(self.remap_expr(x));
        let kind = match &e.e {
            ExprKind::Lit(ExprLit::Enum(l)) => ExprKind::Lit(ExprLit::Enum(EnumLit {
                decl: self.decl_id(l.decl.index()),
                name: l.name.clone(),
                wire: l.wire.clone(),
            })),
            ExprKind::Lit(l) => ExprKind::Lit(l.clone()),
            ExprKind::Path(p) => ExprKind::Path(p.clone()),
            ExprKind::Has(p) => ExprKind::Has(p.clone()),
            ExprKind::Now(a) => ExprKind::Now(*a),
            ExprKind::Not(x) => ExprKind::Not(b(x)),
            ExprKind::Neg(x) => ExprKind::Neg(b(x)),
            ExprKind::Widen(x) => ExprKind::Widen(b(x)),
            ExprKind::Old(x) => ExprKind::Old(b(x)),
            ExprKind::Bin(bin) => ExprKind::Bin(Box::new(BinExpr {
                op: bin.op,
                l: self.remap_expr(&bin.l),
                r: self.remap_expr(&bin.r),
            })),
            ExprKind::Call(c) => ExprKind::Call(CallExpr {
                f: c.f,
                args: self.remap_exprs(&c.args),
            }),
            ExprKind::Quant(q) => ExprKind::Quant(Box::new(QuantExpr {
                q: q.q,
                list: self.remap_expr(&q.list),
                var: q.var.clone(),
                body: self.remap_expr(&q.body),
            })),
            ExprKind::List(items) => ExprKind::List(self.remap_exprs(items)),
        };
        Expr {
            t: self.remap_expr_type(&e.t),
            e: kind,
        }
    }
}
