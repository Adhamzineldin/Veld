//! A small printer from the rule expression IR back to readable Veld text.
//!
//! It is used by the diff report and the OpenAPI export. It prints the semantics, not the exact
//! source: `widen` nodes are transparent, a path is its dotted source names, and parentheses
//! appear only where precedence needs them. The grammar is `spec/rules.md` section 3.1.

use veld_ir::{Builtin, Expr, ExprKind, ExprLit, Op, PathRef, Quantifier, Root};

// Binding strength of a node; higher binds tighter.
const IMPLIES: u8 = 1;
const OR: u8 = 2;
const AND: u8 = 3;
const CMP: u8 = 4;
const ADD: u8 = 5;
const MUL: u8 = 6;
const UNARY: u8 = 7;
const ATOM: u8 = 8;

/// The rule expression as Veld source text.
#[must_use]
pub fn expr_text(e: &Expr) -> String {
    node(e, 0)
}

fn op_info(op: Op) -> (&'static str, u8) {
    match op {
        Op::Implies => ("implies", IMPLIES),
        Op::Or => ("||", OR),
        Op::And => ("&&", AND),
        Op::Eq => ("==", CMP),
        Op::Ne => ("!=", CMP),
        Op::Lt => ("<", CMP),
        Op::Le => ("<=", CMP),
        Op::Gt => (">", CMP),
        Op::Ge => (">=", CMP),
        Op::In => ("in", CMP),
        Op::Add => ("+", ADD),
        Op::Sub => ("-", ADD),
        Op::Mul => ("*", MUL),
        Op::Div => ("/", MUL),
        Op::Rem => ("%", MUL),
    }
}

fn builtin_name(f: Builtin) -> &'static str {
    match f {
        Builtin::Len => "len",
        Builtin::Matches => "matches",
        Builtin::StartsWith => "startsWith",
        Builtin::EndsWith => "endsWith",
        Builtin::Contains => "contains",
        Builtin::Days => "days",
        Builtin::Hours => "hours",
        Builtin::Minutes => "minutes",
        Builtin::Seconds => "seconds",
        Builtin::Ms => "ms",
    }
}

fn quantifier_name(q: Quantifier) -> &'static str {
    match q {
        Quantifier::All => "all",
        Quantifier::Any => "any",
        Quantifier::Count => "count",
        Quantifier::Sum => "sum",
    }
}

/// Prints `e`, wrapping it in parentheses when it binds looser than `min`.
fn node(e: &Expr, min: u8) -> String {
    let (text, prec) = raw(e);
    if prec < min {
        format!("({text})")
    } else {
        text
    }
}

fn list(items: &[Expr]) -> String {
    items
        .iter()
        .map(|a| node(a, 0))
        .collect::<Vec<_>>()
        .join(", ")
}

fn raw(e: &Expr) -> (String, u8) {
    match &e.e {
        ExprKind::Lit(l) => lit(l),
        ExprKind::Path(p) => (path(p), ATOM),
        ExprKind::Now(_) => ("now".to_string(), ATOM),
        ExprKind::Not(x) => (format!("not {}", node(x, UNARY)), UNARY),
        ExprKind::Neg(x) => (format!("-{}", node(x, UNARY)), UNARY),
        ExprKind::Bin(b) => bin(b.op, &b.l, &b.r),
        ExprKind::Has(p) => (format!("has({})", path(p)), ATOM),
        ExprKind::Call(c) => (format!("{}({})", builtin_name(c.f), list(&c.args)), ATOM),
        ExprKind::Quant(q) => (
            format!(
                "{}({}, {} => {})",
                quantifier_name(q.q),
                node(&q.list, 0),
                q.var,
                node(&q.body, 0)
            ),
            ATOM,
        ),
        ExprKind::List(items) => (format!("[{}]", list(items)), ATOM),
        ExprKind::Widen(x) => raw(x),
        ExprKind::Old(x) => (format!("old({})", node(x, 0)), ATOM),
    }
}

fn bin(op: Op, l: &Expr, r: &Expr) -> (String, u8) {
    let (sym, prec) = op_info(op);
    // `implies` is right associative, a comparison does not associate, the rest are left
    // associative.
    let (lmin, rmin) = match op {
        Op::Implies => (prec + 1, prec),
        Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge | Op::In => (prec + 1, prec + 1),
        Op::And | Op::Or | Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Rem => (prec, prec + 1),
    };
    (format!("{} {sym} {}", node(l, lmin), node(r, rmin)), prec)
}

fn path(p: &PathRef) -> String {
    let head: Option<&str> = match &p.root {
        Root::SelfValue => None,
        Root::Principal => Some("principal"),
        Root::Result => Some("result"),
        Root::Var(n) | Root::Param(n) | Root::Fact(n) => Some(n.as_str()),
    };
    let mut parts: Vec<&str> = head.into_iter().collect();
    parts.extend(p.steps.iter().map(|s| s.name.as_str()));
    if parts.is_empty() {
        "self".to_string()
    } else {
        parts.join(".")
    }
}

fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn lit(l: &ExprLit) -> (String, u8) {
    let text = match l {
        ExprLit::Bool(b) => b.to_string(),
        ExprLit::Int(n) | ExprLit::Long(n) => n.to_string(),
        ExprLit::Float(f) => format!("{f:?}"),
        ExprLit::Decimal(d) => d.clone(),
        ExprLit::String(s) => quote(s),
        ExprLit::Enum(e) => format!(".{}", e.name),
    };
    // A negative number is a literal, but it binds like a unary minus when printed.
    let prec = if text.starts_with('-') { UNARY } else { ATOM };
    (text, prec)
}

#[cfg(test)]
mod tests {
    use super::expr_text;
    use crate::testutil::ir;

    const HEAD: &str = "enum St { open, paid }\nmodel Line { qty: int }\nmodel M {\n a: int\n b: int\n c: int\n s: string\n ok: bool\n d: decimal\n st: St\n xs: int[]\n lines: Line[]\n from: datetime\n until: datetime\n opt?: Line\n";

    fn compiled(rule: &str) -> (String, String) {
        let src =
            format!("{HEAD} rule r: {rule}\n}}\nservice S at \"/s\" {{ get \"/\" f() -> M }}");
        let ir = ir(&src);
        let m = ir.decls.iter().find(|d| d.name == "M");
        match m.and_then(|m| m.rules().first()) {
            Some(r) => (expr_text(&r.expr), r.id.clone()),
            None => panic!("no rule"),
        }
    }

    /// Prints the rule, checks the text, and checks that the text means the same rule.
    fn check(rule: &str, want: &str) {
        let (text, id) = compiled(rule);
        assert_eq!(text, want, "printing `{rule}`");
        let (_, again) = compiled(&text);
        assert_eq!(id, again, "`{text}` must mean the same rule as `{rule}`");
    }

    #[test]
    fn comparisons_literals_and_paths() {
        check("a > 0", "a > 0");
        check(r#"s == "x\"y""#, r#"s == "x\"y""#);
        check("d >= 1.50", "d >= 1.50");
        check("ok == true", "ok == true");
        check("!has(opt) || opt.qty > 1", "not has(opt) || opt.qty > 1");
    }

    #[test]
    fn precedence_and_parentheses() {
        check("(a + b) * c > 0", "(a + b) * c > 0");
        check("a + b * c > 0", "a + b * c > 0");
        check("a - (b - c) > 0", "a - (b - c) > 0");
        check("a - b - c > 0", "a - b - c > 0");
        check("a / (b * c) > 0", "a / (b * c) > 0");
        check("(a > 0 || b > 0) && c > 0", "(a > 0 || b > 0) && c > 0");
        check("a > 0 || b > 0 && c > 0", "a > 0 || b > 0 && c > 0");
        check("a > 0 && (b > 0 || c > 0)", "a > 0 && (b > 0 || c > 0)");
        check("!(a > 0 && b > 0)", "not (a > 0 && b > 0)");
        check(
            "(a > 0 implies b > 0) implies c > 0",
            "(a > 0 implies b > 0) implies c > 0",
        );
        check(
            "a > 0 implies b > 0 implies c > 0",
            "a > 0 implies b > 0 implies c > 0",
        );
        check(
            "(a > 0 || b > 0) implies c > 0",
            "a > 0 || b > 0 implies c > 0",
        );
        check("-(a + b) < 0", "-(a + b) < 0");
    }

    #[test]
    fn calls_lists_enums_and_durations() {
        check(
            r#"len(s) <= 3 && matches(s, "^a")"#,
            r#"len(s) <= 3 && matches(s, "^a")"#,
        );
        check(
            r#"startsWith(s, "a") || contains(s, "b")"#,
            r#"startsWith(s, "a") || contains(s, "b")"#,
        );
        check("st in [.open, .paid]", "st in [.open, .paid]");
        check(
            "st == .paid implies from < until",
            "st == .paid implies from < until",
        );
        check("until - from <= days(30)", "until - from <= days(30)");
        check("len(xs) >= 0", "len(xs) >= 0");
    }

    #[test]
    fn comprehensions() {
        check("all(lines, l => l.qty > 0)", "all(lines, l => l.qty > 0)");
        check(
            "sum(lines, l => l.qty) <= 10",
            "sum(lines, l => l.qty) <= 10",
        );
        check("count(xs, x => x > a) == 1", "count(xs, x => x > a) == 1");
        check(
            "any(xs, x => x > 0 && x < 9)",
            "any(xs, x => x > 0 && x < 9)",
        );
    }

    #[test]
    fn widening_is_invisible() {
        check("d > a", "d > a");
    }
}
