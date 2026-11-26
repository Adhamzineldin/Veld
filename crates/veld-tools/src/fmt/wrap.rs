//! Line wrapping at [`WIDTH`](super::WIDTH) for the constructs that can grow long: the call and
//! the record values of an example, the outcome after `->`, the steps of a saga, and the
//! expression of a rule, `require`, `ensure`, `permit` or `forbid`.
//!
//! Nothing here changes a line that fits. A group (a call, a record or a list) that is too wide
//! is written one member per line, indented two spaces, with the closing bracket on a line of its
//! own; a member that fits stays on one line. An expression is broken before a binary operator
//! of the lowest precedence it contains, on continuation lines indented four spaces. The layout
//! is a function of the syntax tree alone, so formatting twice gives the same text.

use veld_compiler::syntax::ast::{BinOp, Expr, ExprKind, SagaValueDecl, Value, ValueKind};

use super::WIDTH;
use super::expr::{bin_prec, child, expr_text, operand_levels};

/// A value, call or record that may be written on one line or one member per line.
pub(super) enum Node {
    /// Text that is never broken.
    Leaf(String),
    /// `head`, members, `close`. `spaced` puts a space inside the brackets when flat (`{ a: 1 }`).
    Group {
        head: String,
        close: char,
        spaced: bool,
        items: Vec<(Option<String>, Node)>,
    },
}

impl Node {
    pub(super) fn record(items: Vec<(Option<String>, Node)>) -> Node {
        Node::Group {
            head: "{".to_owned(),
            close: '}',
            spaced: true,
            items,
        }
    }

    pub(super) fn call(head: String, items: Vec<(Option<String>, Node)>) -> Node {
        Node::Group {
            head: format!("{head}("),
            close: ')',
            spaced: false,
            items,
        }
    }

    pub(super) fn flat_text(&self) -> String {
        self.flat()
    }

    fn flat(&self) -> String {
        match self {
            Node::Leaf(s) => s.clone(),
            Node::Group {
                head,
                close,
                spaced,
                items,
            } => {
                if items.is_empty() {
                    return format!("{head}{close}");
                }
                let inner: Vec<String> = items.iter().map(|(n, v)| named(n, &v.flat())).collect();
                let gap = if *spaced { " " } else { "" };
                format!("{head}{gap}{}{gap}{close}", inner.join(", "))
            }
        }
    }
}

fn named(name: &Option<String>, text: &str) -> String {
    match name {
        Some(n) => format!("{n}: {text}"),
        None => text.to_owned(),
    }
}

fn pad(n: usize) -> String {
    " ".repeat(n)
}

fn width_of(s: &str) -> usize {
    s.chars().count()
}

/// Whether the node fits on one line at `ind`.
pub(super) fn node_flat_fits(n: &Node, ind: usize) -> bool {
    ind + width_of(&n.flat()) <= WIDTH
}

/// `prefix`, the node and `suffix` as lines with absolute indentation, the first at `ind`.
pub(super) fn render(prefix: &str, node: &Node, ind: usize, suffix: &str) -> Vec<String> {
    let flat = format!("{}{prefix}{}{suffix}", pad(ind), node.flat());
    let Node::Group {
        head, close, items, ..
    } = node
    else {
        return vec![flat];
    };
    if items.is_empty() || width_of(&flat) <= WIDTH {
        return vec![flat];
    }
    let mut lines = vec![format!("{}{prefix}{head}", pad(ind))];
    for (i, (name, value)) in items.iter().enumerate() {
        let p = name.as_ref().map_or_else(String::new, |n| format!("{n}: "));
        let comma = if i + 1 < items.len() { "," } else { "" };
        lines.extend(render(&p, value, ind + 2, comma));
    }
    lines.push(format!("{}{close}{suffix}", pad(ind)));
    lines
}

/// The node of an example value.
pub(super) fn value_node(v: &Value) -> Node {
    match &v.kind {
        ValueKind::Object(fields) => Node::record(
            fields
                .iter()
                .map(|f| (Some(f.name.name.clone()), value_node(&f.value)))
                .collect(),
        ),
        ValueKind::List(items) => Node::Group {
            head: "[".to_owned(),
            close: ']',
            spaced: false,
            items: items.iter().map(|i| (None, value_node(i))).collect(),
        },
        ValueKind::Bool(_)
        | ValueKind::Int(_)
        | ValueKind::Float { .. }
        | ValueKind::Str(_)
        | ValueKind::EnumLit(_)
        | ValueKind::Path(_) => Node::Leaf(super::stmt::value_text(v)),
    }
}

/// The node of a saga argument value.
pub(super) fn saga_node(v: &SagaValueDecl) -> Node {
    match v {
        SagaValueDecl::Expr(e) => Node::Leaf(expr_text(e)),
        SagaValueDecl::Object { fields, .. } => Node::record(
            fields
                .iter()
                .map(|f| (Some(f.name.name.clone()), saga_node(&f.value)))
                .collect(),
        ),
    }
}

/// The pieces of `e` when it is broken before its lowest-precedence operator: the first operand,
/// then `op operand` for every further one. Their texts joined by spaces are `expr_text(e)`.
fn segments(e: &Expr) -> Vec<String> {
    let ExprKind::Binary { op, l, r } = &e.kind else {
        return vec![expr_text(e)];
    };
    let (lo, ro) = operand_levels(*op);
    match op {
        BinOp::Implies => {
            let mut segs = vec![child(l, lo)];
            let mut cur: &Expr = r;
            while let ExprKind::Binary {
                op: BinOp::Implies,
                l: l2,
                r: r2,
            } = &cur.kind
            {
                segs.push(format!("{} {}", op.text(), child(l2, lo)));
                cur = r2;
            }
            segs.push(format!("{} {}", op.text(), child(cur, ro)));
            segs
        }
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::In => {
            vec![child(l, lo), format!("{} {}", op.text(), child(r, ro))]
        }
        BinOp::Or | BinOp::And | BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
            left_chain(e, *op)
        }
    }
}

/// A left-associative chain: `a + b - c` is `a`, `+ b`, `- c`.
fn left_chain(e: &Expr, top: BinOp) -> Vec<String> {
    let p = bin_prec(top);
    let (lo, _) = operand_levels(top);
    let mut tail = Vec::new();
    let mut cur = e;
    while let ExprKind::Binary { op, l, r } = &cur.kind {
        if bin_prec(*op) != p {
            break;
        }
        let (_, ro) = operand_levels(*op);
        tail.push(format!("{} {}", op.text(), child(r, ro)));
        cur = l;
    }
    let mut segs = vec![child(cur, lo)];
    segs.extend(tail.into_iter().rev());
    segs
}

/// `prefix`, the expression and `suffix`. On one line when that fits; otherwise broken before the
/// operators of the lowest level, the continuation lines at `ind + 4`. An expression that is not
/// a binary one stays on one line.
pub(super) fn expr_lines(prefix: &str, e: &Expr, suffix: &str, ind: usize) -> Vec<String> {
    let flat = format!("{}{prefix}{}{suffix}", pad(ind), expr_text(e));
    if width_of(&flat) <= WIDTH || !matches!(e.kind, ExprKind::Binary { .. }) {
        return vec![flat];
    }
    let segs = segments(e);
    let last = segs.len().saturating_sub(1);
    segs.iter()
        .enumerate()
        .map(|(i, s)| {
            let head = if i == 0 {
                format!("{}{prefix}", pad(ind))
            } else {
                pad(ind + 4)
            };
            let tail = if i == last { suffix } else { "" };
            format!("{head}{s}{tail}")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_group_that_fits_is_one_line() {
        let n = Node::record(vec![
            (Some("a".into()), Node::Leaf("1".into())),
            (Some("b".into()), Node::Leaf("2".into())),
        ]);
        assert_eq!(render("x = ", &n, 2, ""), vec!["  x = { a: 1, b: 2 }"]);
    }

    #[test]
    fn a_wide_group_has_one_member_per_line_and_nests() {
        let long = "v".repeat(60);
        let inner = Node::record(vec![
            (Some("p".into()), Node::Leaf(long.clone())),
            (Some("q".into()), Node::Leaf(long.clone())),
        ]);
        let n = Node::call(
            "S.e".into(),
            vec![
                (Some("body".into()), inner),
                (Some("n".into()), Node::Leaf("1".into())),
            ],
        );
        let lines = render("", &n, 2, "");
        assert_eq!(lines[0], "  S.e(");
        assert_eq!(lines[1], "    body: {");
        assert_eq!(lines[2], format!("      p: {long},"));
        assert_eq!(lines[4], "    },");
        assert_eq!(lines[5], "    n: 1");
        assert_eq!(lines[6], "  )");
    }
}
