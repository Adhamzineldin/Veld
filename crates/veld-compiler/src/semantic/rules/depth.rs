//! The JSON nesting depth of a rule's IR (`spec/rules.md` section 3.1).
//!
//! The parser limits an expression to 32 levels, which keeps every rule readable with
//! `serde_json`'s default recursion limit of 128. A type inside an expression node
//! (`{"list": {"list": ...}}`) adds JSON depth the parser never sees, so this module measures
//! the real thing after typing and the checker reports `E0005` when a rule would not read back.
//!
//! The arithmetic. `serde_json` fails when containers nest more than 127 deep. In the IR
//! document the `expr` of a rule sits under five containers: the document object, `decls`, the
//! declaration, `rules` and the rule object. So the expression itself may be at most
//! `127 - 5 = 122` containers deep ([`MAX_EXPR_JSON_DEPTH`]). An expression at the parser limit
//! needs at most 3 containers per level (`{"t", "e": {"bin": {"l": ...}}}`) for 32 levels plus a
//! leaf of 5, which is 101 and leaves 21 containers for the types of the nodes. Nested calls cost 4 per level, which only a deep type can push over the budget; the check below catches those too.

use veld_ir::Type;
use veld_ir::rules::Expr;
use veld_ir::rules::{ExprKind, ExprLit, ExprType, PathRef, Root};

/// Containers above a rule's `expr` in the IR document: document, `decls`, declaration, `rules`,
/// rule.
pub const IR_PREFIX_DEPTH: usize = 5;

/// `serde_json` refuses a document nested deeper than this many containers.
pub const SERDE_JSON_MAX_DEPTH: usize = 127;

/// The most containers the JSON of one `expr` may nest.
pub const MAX_EXPR_JSON_DEPTH: usize = SERDE_JSON_MAX_DEPTH - IR_PREFIX_DEPTH;

/// Containers nested in the JSON of `e`, counted as `serde_json` counts them (an object or an
/// array is one level; a string, number or bool is none).
#[must_use]
pub fn json_depth(e: &Expr) -> usize {
    1 + type_json(&e.t).max(kind_json(&e.e))
}

fn type_json(t: &ExprType) -> usize {
    match t {
        ExprType::Duration => 0,
        ExprType::Type(t) => ir_type_json(t),
    }
}

fn ir_type_json(t: &Type) -> usize {
    match t {
        Type::Scalar(_) | Type::Decl(_) => 1,
        Type::List(inner) | Type::Map(inner) | Type::Page(inner) => 1 + ir_type_json(inner),
    }
}

fn kind_json(k: &ExprKind) -> usize {
    match k {
        ExprKind::Lit(l) => 1 + lit_json(l),
        ExprKind::Path(p) | ExprKind::Has(p) => 1 + path_json(p),
        ExprKind::Now(_) => 1,
        ExprKind::Not(x) | ExprKind::Neg(x) | ExprKind::Widen(x) | ExprKind::Old(x) => {
            1 + json_depth(x)
        }
        ExprKind::Bin(b) => 2 + json_depth(&b.l).max(json_depth(&b.r)),
        ExprKind::Call(c) => 3 + c.args.iter().map(json_depth).max().unwrap_or(0),
        ExprKind::Quant(q) => 2 + json_depth(&q.list).max(json_depth(&q.body)),
        ExprKind::List(items) => 2 + items.iter().map(json_depth).max().unwrap_or(0),
    }
}

fn lit_json(l: &ExprLit) -> usize {
    match l {
        ExprLit::Enum(_) => 2,
        ExprLit::Bool(_)
        | ExprLit::Int(_)
        | ExprLit::Long(_)
        | ExprLit::Float(_)
        | ExprLit::Decimal(_)
        | ExprLit::String(_) => 1,
    }
}

/// `{"root": .., "steps": [{..}]}`: the object, then the `steps` array and its step objects.
fn path_json(p: &PathRef) -> usize {
    let root = match &p.root {
        Root::SelfValue | Root::Principal | Root::Result => 0,
        Root::Var(_) | Root::Param(_) | Root::Fact(_) => 1,
    };
    1 + root.max(1 + usize::from(!p.steps.is_empty()))
}
