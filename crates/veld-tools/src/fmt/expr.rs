//! Prints rule expressions with the fewest parentheses that keep the parse (`spec/rules.md`
//! section 3.1). The user's choice of `!` or `not` is kept, numbers keep their source text, and
//! a binary minus always has spaces around it. A `-` applied directly to a numeric literal is
//! part of the literal (`x implies -1`), so a negation of a literal is written `-(1)`.

use veld_compiler::syntax::ast::{BinOp, CallArg, Expr, ExprKind, Rule, UnOp};

use super::text::quote;

const P_IMPLIES: u8 = 1;
const P_OR: u8 = 2;
const P_AND: u8 = 3;
const P_CMP: u8 = 4;
const P_ADD: u8 = 5;
const P_MUL: u8 = 6;
const P_UNARY: u8 = 7;
const P_ATOM: u8 = 8;

pub(super) fn bin_prec(op: BinOp) -> u8 {
    match op {
        BinOp::Implies => P_IMPLIES,
        BinOp::Or => P_OR,
        BinOp::And => P_AND,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::In => P_CMP,
        BinOp::Add | BinOp::Sub => P_ADD,
        BinOp::Mul | BinOp::Div | BinOp::Rem => P_MUL,
    }
}

fn prec(e: &Expr) -> u8 {
    match &e.kind {
        ExprKind::Binary { op, .. } => bin_prec(*op),
        ExprKind::Unary { .. } => P_UNARY,
        ExprKind::Bool(_)
        | ExprKind::Int(_)
        | ExprKind::Float { .. }
        | ExprKind::Str(_)
        | ExprKind::EnumLit(_)
        | ExprKind::Name(_)
        | ExprKind::Field { .. }
        | ExprKind::Call { .. }
        | ExprKind::List(_) => P_ATOM,
    }
}

/// `e`, in parentheses when it binds looser than `min`.
pub(super) fn child(e: &Expr, min: u8) -> String {
    let s = expr_text(e);
    if prec(e) < min { format!("({s})") } else { s }
}

/// The minimum precedence of the (left, right) operand of a binary operator.
pub(super) fn operand_levels(op: BinOp) -> (u8, u8) {
    let p = bin_prec(op);
    match op {
        BinOp::Implies => (p + 1, p),
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::In => {
            (p + 1, p + 1)
        }
        BinOp::Or | BinOp::And | BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
            (p, p + 1)
        }
    }
}

pub(super) fn expr_text(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Bool(b) => b.to_string(),
        ExprKind::Int(n) => n.to_string(),
        ExprKind::Float { text, .. } => text.clone(),
        ExprKind::Str(s) => quote(s),
        ExprKind::EnumLit(id) => format!(".{}", id.name),
        ExprKind::Name(id) => id.name.clone(),
        ExprKind::Field { base, name } => format!("{}.{}", child(base, P_ATOM), name.name),
        ExprKind::Call { name, args } => {
            let args: Vec<String> = args.iter().map(arg_text).collect();
            format!("{}({})", name.name, args.join(", "))
        }
        ExprKind::List(items) => {
            let items: Vec<String> = items.iter().map(expr_text).collect();
            format!("[{}]", items.join(", "))
        }
        ExprKind::Unary { op, operand } => unary_text(*op, operand),
        ExprKind::Binary { op, l, r } => {
            let (lo, ro) = operand_levels(*op);
            format!("{} {} {}", child(l, lo), op.text(), child(r, ro))
        }
    }
}

fn unary_text(op: UnOp, operand: &Expr) -> String {
    let inner = child(operand, P_UNARY);
    match op {
        UnOp::Not { word: true } => format!("not {inner}"),
        UnOp::Not { word: false } => format!("!{inner}"),
        // A `-` applied directly to a numeric literal is part of the literal, so a negation
        // of one keeps its parentheses: `-(1)`.
        UnOp::Neg if matches!(operand.kind, ExprKind::Int(_) | ExprKind::Float { .. }) => {
            format!("-({inner})")
        }
        UnOp::Neg if inner.starts_with('-') => format!("- {inner}"),
        UnOp::Neg => format!("-{inner}"),
    }
}

fn arg_text(a: &CallArg) -> String {
    match a {
        CallArg::Expr(e) => expr_text(e),
        CallArg::Lambda { var, body, .. } => format!("{} => {}", var.name, expr_text(body)),
    }
}

/// The expression of a derived field (`spec/rules.md` section 17.2). Written after `=`, one literal
/// would read as a default, so a literal that was written in parentheses keeps them.
pub(super) fn derived_text(e: &Expr) -> String {
    let literal = match &e.kind {
        ExprKind::Bool(_)
        | ExprKind::Int(_)
        | ExprKind::Float { .. }
        | ExprKind::Str(_)
        | ExprKind::EnumLit(_) => true,
        ExprKind::List(items) => items.is_empty(),
        ExprKind::Name(_)
        | ExprKind::Field { .. }
        | ExprKind::Call { .. }
        | ExprKind::Unary { .. }
        | ExprKind::Binary { .. } => false,
    };
    let text = expr_text(e);
    if literal { format!("({text})") } else { text }
}

/// `rule name: expr`, then the message on a continuation line.
pub(super) fn rule_lines(r: &Rule, ind: usize) -> Vec<String> {
    let pad = " ".repeat(ind);
    let mut lines = super::wrap::expr_lines(&format!("rule {}: ", r.name.name), &r.expr, "", ind);
    if let Some(m) = &r.message {
        lines.push(format!("{pad}  message {}", quote(&m.value)));
    }
    super::stmt::with_flag(lines, r.flag.as_ref())
}
