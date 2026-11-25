//! Parser tests for rules and expressions (`spec/rules.md` sections 2.2, 3.1 and 4).

use super::*;
use crate::syntax::ast::{BinOp, CallArg, DeclKind, Expr, ExprKind, UnOp};

fn p(src: &str) -> Parsed {
    parse(src, FileId(0))
}

fn codes(src: &str) -> Vec<&'static str> {
    p(src).diagnostics.iter().map(|d| d.code).collect()
}

/// The expression of `rule r: <src>` as an s-expression.
fn sexp(src: &str) -> String {
    let r = p(&format!("model M {{ rule r: {src} }}"));
    assert!(r.diagnostics.is_empty(), "{src}: {:?}", r.diagnostics);
    let DeclKind::Model(m) = &r.file.decls[0].kind else {
        panic!("not a model")
    };
    show(&m.rules[0].expr)
}

fn show(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Bool(b) => b.to_string(),
        ExprKind::Int(n) => n.to_string(),
        ExprKind::Float { text, .. } => text.clone(),
        ExprKind::Str(s) => format!("{s:?}"),
        ExprKind::EnumLit(id) => format!(".{}", id.name),
        ExprKind::Name(id) => id.name.clone(),
        ExprKind::Field { base, name } => format!("(. {} {})", show(base), name.name),
        ExprKind::Call { name, args } => {
            let args: Vec<String> = args
                .iter()
                .map(|a| match a {
                    CallArg::Expr(e) => show(e),
                    CallArg::Lambda { var, body, .. } => {
                        format!("({} => {})", var.name, show(body))
                    }
                })
                .collect();
            format!("({} {})", name.name, args.join(" "))
        }
        ExprKind::List(items) => {
            let items: Vec<String> = items.iter().map(show).collect();
            format!("[{}]", items.join(" "))
        }
        ExprKind::Unary { op, operand } => {
            let name = match op {
                UnOp::Not { word: true } => "not",
                UnOp::Not { word: false } => "!",
                UnOp::Neg => "neg",
            };
            format!("({name} {})", show(operand))
        }
        ExprKind::Binary { op, l, r } => format!("({} {} {})", op.text(), show(l), show(r)),
    }
}

#[test]
fn precedence_from_loosest_to_tightest() {
    assert_eq!(sexp("a implies b || c"), "(implies a (|| b c))");
    assert_eq!(sexp("a || b && c"), "(|| a (&& b c))");
    assert_eq!(sexp("a && b == c"), "(&& a (== b c))");
    assert_eq!(sexp("a == b + c"), "(== a (+ b c))");
    assert_eq!(sexp("a + b * c"), "(+ a (* b c))");
    assert_eq!(sexp("a * b + c"), "(+ (* a b) c)");
    assert_eq!(sexp("a + 1 < b * 2"), "(< (+ a 1) (* b 2))");
    assert_eq!(sexp("a in b && c"), "(&& (in a b) c)");
}

#[test]
fn associativity() {
    assert_eq!(sexp("a implies b implies c"), "(implies a (implies b c))");
    assert_eq!(sexp("a || b || c"), "(|| (|| a b) c)");
    assert_eq!(sexp("a && b && c"), "(&& (&& a b) c)");
    assert_eq!(sexp("a - b - c"), "(- (- a b) c)");
    assert_eq!(sexp("a / b / c"), "(/ (/ a b) c)");
    assert_eq!(sexp("a % b * c"), "(* (% a b) c)");
    assert_eq!(sexp("(a implies b) implies c"), "(implies (implies a b) c)");
    assert_eq!(sexp("a - (b - c)"), "(- a (- b c))");
}

#[test]
fn unary_operators_bind_tighter_than_binary_ones() {
    assert_eq!(sexp("not a == b"), "(== (not a) b)");
    assert_eq!(sexp("!a == b"), "(== (! a) b)");
    assert_eq!(sexp("-a * b"), "(* (neg a) b)");
    assert_eq!(sexp("not not a"), "(not (not a))");
    assert_eq!(sexp("!a && !b"), "(&& (! a) (! b))");
    assert_eq!(sexp("- a.b"), "(neg (. a b))");
    assert_eq!(sexp("not (a == b)"), "(not (== a b))");
}

#[test]
fn a_minus_is_a_literal_or_an_operator_by_the_previous_token() {
    assert_eq!(sexp("a -1"), "(- a 1)");
    assert_eq!(sexp("a - -1"), "(- a -1)");
    assert_eq!(sexp("a == -1"), "(== a -1)");
    assert_eq!(sexp("a > - 1"), "(> a -1)");
    assert_eq!(sexp("a > -(1)"), "(> a (neg 1))");
    assert_eq!(sexp("a implies -1 < b"), "(implies a (< -1 b))");
    assert_eq!(sexp("not -1 == a"), "(== (not -1) a)");
    assert_eq!(sexp("-1 < a"), "(< -1 a)");
    assert_eq!(sexp("a in [-1, -2]"), "(in a [-1 -2])");
    assert_eq!(sexp("days(30) -1"), "(- (days 30) 1)");
}

#[test]
fn every_primary_form() {
    assert_eq!(sexp("true && false"), "(&& true false)");
    assert_eq!(sexp("\"x\" == s"), "(== \"x\" s)");
    assert_eq!(sexp("n == 5"), "(== n 5)");
    assert_eq!(sexp("d == 1.50"), "(== d 1.50)");
    assert_eq!(sexp("s == .paid"), "(== s .paid)");
    assert_eq!(sexp("a.b.c > 0"), "(> (. (. a b) c) 0)");
    assert_eq!(sexp("(a + b).c > 0"), "(> (. (+ a b) c) 0)");
    assert_eq!(sexp("x in [1, 2, 3,]"), "(in x [1 2 3])");
    assert_eq!(sexp("x in []"), "(in x [])");
    assert_eq!(sexp("len(s) > 0"), "(> (len s) 0)");
    assert_eq!(sexp("f()"), "(f )");
    assert_eq!(sexp("f(a, b,)"), "(f a b)");
    assert_eq!(
        sexp("all(xs, i => i.q > 0)"),
        "(all xs (i => (> (. i q) 0)))"
    );
    assert_eq!(sexp("has(a.b)"), "(has (. a b))");
    assert_eq!(sexp("matches(s, \"^a$\")"), "(matches s \"^a$\")");
}

#[test]
fn a_float_keeps_its_source_text() {
    assert_eq!(sexp("d == 120.00"), "(== d 120.00)");
    assert_eq!(sexp("d == 1.0e308"), "(== d 1.0e308)");
    assert_eq!(sexp("d == -0.250"), "(== d -0.250)");
}

#[test]
fn spans_cover_the_whole_expression_including_parentheses() {
    let src = "model M { rule r: (a || b) && c message \"m\" }";
    let r = p(src);
    let DeclKind::Model(m) = &r.file.decls[0].kind else {
        panic!("not a model")
    };
    let rule = &m.rules[0];
    let text = |s: crate::diagnostics::Span| &src[s.start as usize..s.end as usize];
    assert_eq!(text(rule.expr.span), "(a || b) && c");
    assert_eq!(text(rule.span), "rule r: (a || b) && c message \"m\"");
    assert_eq!(text(rule.name.span), "r");
    assert_eq!(rule.message.as_ref().map(|m| m.value.as_str()), Some("m"));
}

#[test]
fn a_chained_comparison_is_e0902() {
    let r = p("model M { rule r: a < b < c\n rule s: a == b != c\n rule ok: a < b }");
    let codes: Vec<_> = r.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(codes, vec!["E0902", "E0902"]);
    let DeclKind::Model(m) = &r.file.decls[0].kind else {
        panic!("not a model")
    };
    assert_eq!(m.rules.len(), 3, "the chain is still consumed");
}

#[test]
fn rule_with_a_colon_after_it_is_a_field() {
    let r = p("model M { rule: string\n rule?: int\n rule r: len(rule) > 0 }");
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    let DeclKind::Model(m) = &r.file.decls[0].kind else {
        panic!("not a model")
    };
    assert_eq!(m.fields.len(), 2);
    assert_eq!(m.rules.len(), 1);
    assert!(m.fields[1].optional);
}

#[test]
fn in_and_implies_before_a_colon_start_a_member() {
    for member in ["in: int", "in?: int", "implies: int", "implies?: bool"] {
        let r = p(&format!("model M {{ a: bool\n rule r: a\n {member}\n}}"));
        assert!(r.diagnostics.is_empty(), "{member}: {:?}", r.diagnostics);
        let DeclKind::Model(m) = &r.file.decls[0].kind else {
            panic!("not a model")
        };
        assert_eq!((m.fields.len(), m.rules.len()), (2, 1), "{member}");
        assert_eq!(show(&m.rules[0].expr), "a", "{member}");
    }
}

#[test]
fn message_is_a_clause_only_before_a_string() {
    let r = p(
        "model M { a: bool\n rule r: a message \"text\"\n message: string\n rule s: a\n message\n   \"on its own line\" }",
    );
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    let DeclKind::Model(m) = &r.file.decls[0].kind else {
        panic!("not a model")
    };
    assert_eq!(m.fields.len(), 2);
    assert_eq!(
        m.rules[0].message.as_ref().map(|s| s.value.as_str()),
        Some("text")
    );
    assert_eq!(
        m.rules[1].message.as_ref().map(|s| s.value.as_str()),
        Some("on its own line")
    );
}

#[test]
fn not_is_a_name_when_no_operand_follows() {
    assert_eq!(sexp("not == true"), "(== not true)");
    assert_eq!(sexp("not implies implies"), "(implies not implies)");
    assert_eq!(sexp("not in && b"), "(&& (not in) b)");
    assert_eq!(sexp("in > has"), "(> in has)");
}

#[test]
fn rules_carry_docs_and_annotations_and_mix_with_fields() {
    let r = p(
        "error E 409 {\n a: int\n /// Doc one.\n /// Doc two.\n @deprecated(\"x\")\n rule r: a > 0\n b: int\n}",
    );
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    let DeclKind::Error(e) = &r.file.decls[0].kind else {
        panic!("not an error")
    };
    assert_eq!(e.fields.len(), 2);
    assert_eq!(e.rules[0].docs.len(), 2);
    assert_eq!(e.rules[0].annotations[0].name.name, "deprecated");
}

#[test]
fn an_error_in_one_rule_does_not_lose_the_next_members() {
    let r = p(
        "model M {\n a: int\n rule bad1: a >\n rule bad2: (a +\n rule bad3: a == null\n rule bad4: len(,)\n rule good: a > 0\n b: int\n}\nmodel After { n: int }",
    );
    let n = r.diagnostics.iter().filter(|d| d.code == "E0003").count();
    assert!(n >= 4, "{:?}", r.diagnostics);
    assert_eq!(r.file.decls.len(), 2);
    let DeclKind::Model(m) = &r.file.decls[0].kind else {
        panic!("not a model")
    };
    assert!(m.rules.iter().any(|x| x.name.name == "good"));
    assert!(m.fields.iter().any(|x| x.name.name == "b"));
}

#[test]
fn expression_syntax_errors_name_what_was_expected() {
    let r = p("model M { rule r: a + * b }");
    assert_eq!(r.diagnostics.len(), 1);
    assert!(r.diagnostics[0].message.contains("expected an expression"));
    assert_eq!(codes("model M { rule r: a == null }"), vec!["E0003"]);
    assert_eq!(codes("model M { rule use: a }"), vec!["E0004"]);
    assert_eq!(codes("model M { rule r: model > 1 }"), vec!["E0004"]);
}

fn nested_parens(n: usize) -> String {
    format!("model M {{ rule r: {}a{} }}", "(".repeat(n), ")".repeat(n))
}

#[test]
fn expression_nesting_is_limited_to_32_levels_with_e0005() {
    assert!(codes(&nested_parens(32)).is_empty());
    let c = codes(&nested_parens(33));
    assert!(c.contains(&"E0005"), "{c:?}");
    let c = codes(&nested_parens(10_000));
    assert_eq!(c.iter().filter(|c| **c == "E0005").count(), 1, "{c:?}");
}

#[test]
fn the_links_of_a_left_associative_chain_count_as_levels() {
    let chain = |n: usize, op: &str| format!("model M {{ rule r: {} }}", vec!["a"; n].join(op));
    assert!(codes(&chain(33, " + ")).is_empty());
    assert!(codes(&chain(34, " + ")).contains(&"E0005"));
    for op in [" + ", " && ", " || ", " * "] {
        assert!(codes(&chain(100, op)).contains(&"E0005"), "{op}");
    }
    assert!(codes(&chain(5_000, " + ")).contains(&"E0005"));
    // `implies` chains lean to the right and `.x` chains to the left.
    assert!(codes(&chain(100, " implies ")).contains(&"E0005"));
    assert!(codes(&chain(100, ".")).contains(&"E0005"));
}

#[test]
fn unary_lists_and_calls_nest_too() {
    let r = |s: String| codes(&format!("model M {{ rule r: {s} }}"));
    assert!(r(format!("{}a", "!".repeat(100))).contains(&"E0005"));
    assert!(r(format!("{}a", "not ".repeat(100))).contains(&"E0005"));
    assert!(r(format!("{}a{}", "[".repeat(100), "]".repeat(100))).contains(&"E0005"));
    assert!(r(format!("{}a{}", "f(".repeat(100), ")".repeat(100))).contains(&"E0005"));
}

#[test]
fn a_minus_before_a_numeric_literal_is_one_literal_inside_an_expression() {
    assert_eq!(sexp("a implies -1 < b"), "(implies a (< -1 b))");
    assert_eq!(sexp("a implies (-1 < b)"), "(implies a (< -1 b))");
    assert_eq!(sexp("a in [-1, - 2]"), "(in a [-1 -2])");
    assert_eq!(sexp("- -1"), "(neg -1)");
    assert_eq!(sexp("-(1)"), "(neg 1)");
    assert_eq!(sexp("- 1.50 < a"), "(< -1.50 a)");
    // The smallest `long` is a valid literal only with its sign; the lexer's range error for
    // the bare magnitude goes away when the sign joins it.
    assert!(codes("model M { rule r: a implies -9223372036854775808 < b }").is_empty());
    assert_eq!(
        codes("model M { rule r: a implies 9223372036854775808 < b }"),
        vec!["E0002"]
    );
    assert_eq!(
        codes("model M { rule r: a implies -(9223372036854775808) < b }"),
        vec!["E0002"]
    );
}

#[test]
fn comparison_operators_cover_every_form() {
    for (src, op) in [
        ("a == b", BinOp::Eq),
        ("a != b", BinOp::Ne),
        ("a < b", BinOp::Lt),
        ("a <= b", BinOp::Le),
        ("a > b", BinOp::Gt),
        ("a >= b", BinOp::Ge),
        ("a in b", BinOp::In),
    ] {
        assert!(sexp(src).starts_with(&format!("({} ", op.text())), "{src}");
    }
}

#[test]
fn type_arguments_may_be_followed_by_equals_without_a_space() {
    let r = p("model M { a: Map<string,int>= .x }");
    // `>=` is one token in expressions only; a type still closes with `>`.
    assert!(
        r.diagnostics.iter().all(|d| !d.message.contains("`>`")),
        "{:?}",
        r.diagnostics
    );
}
