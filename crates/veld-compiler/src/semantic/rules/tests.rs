//! Tests of the invariant checker, driven through the public pipeline.

use crate::testing::{codes, ir, one};
use veld_ir::rules::{Expr, ExprKind, ExprLit, ExprType, Root};
use veld_ir::{Ir, Scalar, Type};

/// The IR of `src`, or the rendered diagnostics.
fn compiled(src: &str) -> Ir {
    ir(src)
}

fn rule_of(src: &str, model: &str, rule: &str) -> veld_ir::rules::Rule {
    let ir = compiled(src);
    let decl = ir.decls.iter().find(|d| d.name == model);
    let found = decl.and_then(|d| d.rules().iter().find(|r| r.name == rule));
    match found {
        Some(r) => r.clone(),
        None => panic!("no rule {model}.{rule} in:\n{src}"),
    }
}

fn ty(t: &ExprType) -> String {
    match t {
        ExprType::Duration => "duration".to_string(),
        ExprType::Type(Type::Scalar(s)) => s.name().to_string(),
        ExprType::Type(Type::Decl(d)) => format!("decl{}", d.index()),
        ExprType::Type(Type::List(i)) => format!("{}[]", ty(&ExprType::Type((**i).clone()))),
        ExprType::Type(other) => format!("{other:?}"),
    }
}

/// An s-expression of the typed tree: literals carry their type, widenings their target.
fn sx(e: &Expr) -> String {
    let path = |p: &veld_ir::rules::PathRef| {
        let root = match &p.root {
            Root::SelfValue => "self".to_string(),
            Root::Var(v) => format!("${v}"),
            Root::Param(n) | Root::Fact(n) => n.clone(),
            Root::Principal => "principal".to_string(),
            Root::Result => "result".to_string(),
        };
        let steps: Vec<&str> = p.steps.iter().map(|s| s.name.as_str()).collect();
        if steps.is_empty() {
            root
        } else {
            format!("{root}.{}", steps.join("."))
        }
    };
    match &e.e {
        ExprKind::Lit(l) => match l {
            ExprLit::Bool(b) => b.to_string(),
            ExprLit::Int(n) => format!("int:{n}"),
            ExprLit::Long(n) => format!("long:{n}"),
            ExprLit::Float(f) => format!("float:{f}"),
            ExprLit::Decimal(s) => format!("dec:{s}"),
            ExprLit::String(s) => format!("{s:?}"),
            ExprLit::Enum(v) => format!(".{}", v.name),
        },
        ExprKind::Path(p) => path(p),
        ExprKind::Has(p) => format!("(has {})", path(p)),
        ExprKind::Now(_) => "now".to_string(),
        ExprKind::Not(x) => format!("(not {})", sx(x)),
        ExprKind::Neg(x) => format!("(neg {})", sx(x)),
        ExprKind::Widen(x) => format!("(widen->{} {})", ty(&e.t), sx(x)),
        ExprKind::Old(x) => format!("(old {})", sx(x)),
        ExprKind::Bin(b) => format!("({:?} {} {})", b.op, sx(&b.l), sx(&b.r)).to_lowercase(),
        ExprKind::Call(c) => {
            let args: Vec<String> = c.args.iter().map(sx).collect();
            format!("({:?} {})", c.f, args.join(" ")).to_lowercase()
        }
        ExprKind::Quant(q) => {
            format!("({:?} {} {} => {})", q.q, sx(&q.list), q.var, sx(&q.body)).to_lowercase()
        }
        ExprKind::List(items) => {
            let items: Vec<String> = items.iter().map(sx).collect();
            format!("[{}]", items.join(" "))
        }
    }
}

/// `sx` of rule `r` of a model with `fields`.
fn typed(fields: &str, rule: &str) -> String {
    let src = format!("enum E {{ a, b }}\nmodel M {{\n{fields}\n rule r: {rule}\n}}");
    sx(&rule_of(&src, "M", "r").expr)
}

const NUMS: &str = " i: int\n l: long\n f: float\n d: decimal\n s: string\n b: bool";

fn first_code(fields: &str, rule: &str) -> Vec<&'static str> {
    codes(&format!(
        "enum E {{ a, b }}\nmodel M {{\n{fields}\n rule r: {rule}\n}}"
    ))
}

mod typing {
    use super::*;

    #[test]
    fn a_literal_takes_the_type_of_the_other_operand() {
        assert_eq!(typed(NUMS, "l + 1 == l"), "(eq (add self.l long:1) self.l)");
        assert_eq!(typed(NUMS, "1 + l == l"), "(eq (add long:1 self.l) self.l)");
        assert_eq!(typed(NUMS, "d > 0"), "(gt self.d dec:0)");
        assert_eq!(typed(NUMS, "f < 2"), "(lt self.f float:2)");
        assert_eq!(typed(NUMS, "f < 2.5"), "(lt self.f float:2.5)");
        assert_eq!(typed(NUMS, "d > 1.50"), "(gt self.d dec:1.50)");
        assert_eq!(typed(NUMS, "i > -3"), "(gt self.i int:-3)");
    }

    #[test]
    fn without_context_an_int_literal_is_int_or_long_and_a_float_literal_is_decimal() {
        assert_eq!(typed(NUMS, "1 < 2"), "(lt int:1 int:2)");
        assert_eq!(typed(NUMS, "3000000000 > 2"), "(gt long:3000000000 long:2)");
        assert_eq!(typed(NUMS, "1.5 > 1"), "(gt dec:1.5 dec:1)");
        assert_eq!(typed(NUMS, "1 < 2.5"), "(lt dec:1 dec:2.5)");
    }

    #[test]
    fn a_non_literal_is_widened_explicitly() {
        assert_eq!(typed(NUMS, "i < l"), "(lt (widen->long self.i) self.l)");
        assert_eq!(typed(NUMS, "l > i"), "(gt self.l (widen->long self.i))");
        assert_eq!(typed(NUMS, "i < f"), "(lt (widen->float self.i) self.f)");
        assert_eq!(
            typed(NUMS, "i * d <= d"),
            "(le (mul (widen->decimal self.i) self.d) self.d)"
        );
        assert_eq!(typed(NUMS, "l == d"), "(eq (widen->decimal self.l) self.d)");
        assert_eq!(
            typed(NUMS, "i > 1.5"),
            "(gt (widen->decimal self.i) dec:1.5)"
        );
    }

    #[test]
    fn narrowing_conversions_are_type_errors() {
        for rule in [
            "l < f",
            "f < l",
            "d == f",
            "f + d > f",
            "s == i",
            "b < b",
            "s + s == s",
        ] {
            assert!(
                !first_code(NUMS, rule).is_empty(),
                "`{rule}` must not compile"
            );
        }
        assert_eq!(first_code(NUMS, "l < f"), vec!["E0902"]);
        assert_eq!(first_code(NUMS, "d == f"), vec!["E0902"]);
        assert_eq!(first_code(NUMS, "b < b"), vec!["E0905"]);
    }

    #[test]
    fn literals_out_of_range_are_e0908() {
        assert_eq!(first_code(NUMS, "i > 3000000000"), vec!["E0908"]);
        assert_eq!(first_code(NUMS, "f < 9007199254740993"), vec!["E0908"]);
        assert_eq!(first_code(NUMS, "d < 1.5e3"), vec!["E0908"]);
        assert!(first_code(NUMS, "f < 9007199254740992").is_empty());
        assert!(first_code(NUMS, "i > 2147483647").is_empty());
        assert!(first_code(NUMS, "l > 9223372036854775807").is_empty());
    }

    #[test]
    fn the_arithmetic_table() {
        assert_eq!(first_code(NUMS, "i / 2 == 1"), Vec::<&str>::new());
        assert_eq!(first_code(NUMS, "l % 2 == 1"), Vec::<&str>::new());
        assert_eq!(first_code(NUMS, "f / 2.0 > 1"), Vec::<&str>::new());
        assert_eq!(first_code(NUMS, "d / 2 > 1"), vec!["E0905"]);
        assert_eq!(first_code(NUMS, "d % 2 > 1"), vec!["E0905"]);
        assert_eq!(first_code(NUMS, "i / d > 1"), vec!["E0905"]);
        assert_eq!(first_code(NUMS, "-s == s"), vec!["E0902"]);
    }

    #[test]
    fn time_arithmetic() {
        let t = " a: datetime\n z: datetime\n d1: date\n d2: date\n t1: time\n n: int";
        assert!(first_code(t, "z - a <= days(30)").is_empty());
        assert!(first_code(t, "a + hours(2) == z").is_empty());
        assert!(first_code(t, "hours(2) + a == z").is_empty());
        assert!(first_code(t, "d2 - d1 == days(7)").is_empty());
        assert!(first_code(t, "days(n) - hours(1) > ms(5)").is_empty());
        assert!(first_code(t, "-days(n) < ms(0)").is_empty());
        assert!(first_code(t, "t1 < t1").is_empty());
        assert_eq!(first_code(t, "a + z > a"), vec!["E0902"]);
        assert_eq!(first_code(t, "d1 + days(1) == d2"), vec!["E0902"]);
        assert_eq!(first_code(t, "a == d1"), vec!["E0902"]);
        assert_eq!(first_code(t, "days(1) * 2 > days(1)"), vec!["E0902"]);
        assert_eq!(first_code(t, "n == days(1)"), vec!["E0902"]);
        assert_eq!(
            typed(t, "z - a <= days(30)"),
            "(le (sub self.z self.a) (days int:30))"
        );
    }

    #[test]
    fn enum_literals_take_their_enum_from_the_other_side() {
        let f = " e: E\n n: int";
        assert_eq!(typed(f, "e == .a"), "(eq self.e .a)");
        assert_eq!(typed(f, ".b != e"), "(ne .b self.e)");
        assert_eq!(typed(f, "e in [.a, .b]"), "(in self.e [.a .b])");
        assert_eq!(first_code(f, ".a == .b"), vec!["E0902", "E0902"]);
        assert_eq!(first_code(f, "n == .a"), vec!["E0902"]);
        assert_eq!(first_code(f, "e == .c"), vec!["E0901"]);
        assert_eq!(first_code(f, "e < .b"), vec!["E0905"]);
    }

    #[test]
    fn equality_and_membership() {
        let f = " tags: string[]\n limits: Map<string, int>\n n: int\n l: long\n s: string\n m: M2[]\n u: uuid";
        let src = |rule: &str| {
            codes(&format!(
                "model M2 {{ x: int }}\nmodel M {{\n{f}\n rule r: {rule}\n}}"
            ))
        };
        assert!(src("s in tags").is_empty());
        assert!(src("\"k\" in limits").is_empty());
        assert!(src("n in [1, 2, 3]").is_empty());
        assert!(src("1 in [n, 2]").is_empty());
        assert!(src("l in [n, 2]").is_empty());
        assert!(src("u == u").is_empty());
        assert_eq!(src("n in tags"), vec!["E0902"]);
        assert_eq!(src("n in limits"), vec!["E0902"]);
        assert_eq!(src("l in [n]"), Vec::<&str>::new());
        assert_eq!(src("n in [l]"), vec!["E0902"]);
        assert_eq!(src("s in s"), vec!["E0902"]);
        assert_eq!(src("tags == tags"), vec!["E0905"]);
        assert_eq!(src("u == \"x\""), vec!["E0902"]);
    }

    #[test]
    fn a_duration_is_not_a_member_of_anything() {
        let t = " a: datetime
 z: datetime
 xs: int[]";
        assert_eq!(first_code(t, "z - a in [days(1)]"), vec!["E0902"]);
        assert_eq!(first_code(t, "z - a in xs"), vec!["E0902"]);
    }

    #[test]
    fn an_expression_that_fails_always_leaves_a_diagnostic() {
        // Whatever else is wrong, no rule may drop out of the IR without an error.
        for rule in [
            "z - a in [days(1)]",
            "len([]) == 0",
            ".a == .b",
            "has(n)",
            "all(xs)",
            "all(n, x => true)",
            "min(n, 1) > 0",
            "x in n",
        ] {
            let t = " a: datetime
 z: datetime
 xs: int[]
 n: int";
            let out = codes(&format!(
                "model M {{
{t}
 rule r: {rule}
}}"
            ));
            assert!(!out.is_empty(), "`{rule}` compiled without a diagnostic");
        }
    }

    #[test]
    fn an_empty_list_needs_a_list_on_the_other_side() {
        assert!(first_code(NUMS, "i in []").is_empty());
        assert_eq!(first_code(NUMS, "len([]) == 0"), vec!["E0902"]);
    }

    #[test]
    fn a_rule_must_be_a_bool() {
        assert_eq!(first_code(NUMS, "i + 1"), vec!["E0902"]);
        assert_eq!(first_code(NUMS, "s"), vec!["E0902"]);
        assert!(first_code(NUMS, "b").is_empty());
    }

    #[test]
    fn float_equality_warns() {
        assert_eq!(first_code(NUMS, "f == f"), vec!["W0901"]);
        assert_eq!(first_code(NUMS, "f != 1.5"), vec!["W0901"]);
        assert_eq!(first_code(NUMS, "i == f"), vec!["W0901"]);
        assert!(first_code(NUMS, "f < f").is_empty());
        assert!(first_code(NUMS, "d == d").is_empty());
    }
}

mod names {
    use super::*;

    #[test]
    fn unknown_names_get_a_did_you_mean() {
        let out = one("model M { total: int\n rule r: totl > 0 }");
        let d = &out.diagnostics[0];
        assert_eq!(d.code, "E0901");
        assert_eq!(d.help.as_deref(), Some("did you mean `total`?"));
    }

    #[test]
    fn a_const_is_inlined_as_a_literal_in_its_declared_type() {
        let src = "const K: int = 3\nconst L: long = 5000000000\nconst S: string = \"vip\"\nconst D: decimal = 2.5\nconst F: float = 0.5\nconst B: bool = true\nmodel M { n: int\n big: long\n s: string\n d: decimal\n f: float\n b: bool\n rule r: n < K && big < L && s == S && d > D && f < F && b == B }";
        let r = rule_of(src, "M", "r");
        assert_eq!(
            sx(&r.expr),
            "(and (and (and (and (and (lt self.n int:3) (lt self.big long:5000000000)) (eq self.s \"vip\")) (gt self.d dec:2.5)) (lt self.f float:0.5)) (eq self.b true))"
        );
        assert_eq!(r.reads, vec!["n", "big", "s", "d", "f", "b"]);
    }

    #[test]
    fn a_typed_const_is_widened_not_retyped() {
        let src = "const K: int = 3\nmodel M { big: long\n rule r: big > K }";
        assert_eq!(
            sx(&rule_of(src, "M", "r").expr),
            "(gt self.big (widen->long int:3))"
        );
    }

    #[test]
    fn consts_of_other_types_are_e0905() {
        let src = "const D: date = \"2026-01-01\"\nmodel M { d: date\n rule r: d > D }";
        assert_eq!(codes(src), vec!["E0905"]);
    }

    #[test]
    fn a_decimal_const_must_look_like_a_number_to_be_inlined() {
        let src = |v: &str| {
            format!(
                "const D: decimal = {v}
model M {{ d: decimal
 rule r: d > D }}"
            )
        };
        assert!(codes(&src("\"12.50\"")).is_empty());
        assert!(codes(&src("3")).is_empty());
        assert_eq!(codes(&src("\"abc\"")), vec!["E0905"]);
        assert_eq!(codes(&src("\"1e5\"")), vec!["E0905"]);
        assert_eq!(codes(&src("\"\"")), vec!["E0905"]);
    }

    #[test]
    fn a_const_has_no_fields() {
        let src = "const K: int = 3\nmodel M { n: int\n rule r: K.x > n }";
        assert_eq!(codes(src), vec!["E0902"]);
    }

    #[test]
    fn a_field_wins_over_a_const_and_a_lambda_wins_over_both() {
        let src = "const n: int = 3\nmodel M { n: int\n xs: int[]\n rule r: n > 0\n rule q: all(xs, k => k > 0) }";
        let r = rule_of(src, "M", "r");
        assert_eq!(sx(&r.expr), "(gt self.n int:0)");
        assert_eq!(r.reads, vec!["n"]);
    }

    #[test]
    fn now_result_and_old_are_not_available_but_fields_may_carry_their_names() {
        let f = " at2: datetime\n n: int";
        assert_eq!(first_code(f, "at2 > now"), vec!["E0904"]);
        assert_eq!(first_code(f, "n == result.n"), vec!["E0904"]);
        assert_eq!(first_code(f, "old(n) == n"), vec!["E0904"]);
        let named = " now: datetime\n result: int\n old: int";
        assert!(first_code(named, "now > now && result > old").is_empty());
    }

    #[test]
    fn lambda_variables_must_not_shadow() {
        let f = " xs: int[]\n n: int";
        assert_eq!(first_code(f, "all(xs, n => n > 0)"), vec!["E0901"]);
        assert_eq!(
            first_code(f, "all(xs, x => any(xs, x => x > 0))"),
            vec!["E0901"]
        );
        assert!(first_code(f, "all(xs, x => x > 0) && any(xs, x => x < 0)").is_empty());
    }

    #[test]
    fn paths_read_fields_of_models_pages_and_inherited_fields() {
        let src = "model P { q: int }\nmodel C extends P { w: int }\nmodel M { c: C\n pg: Page<P>\n rule r: c.q > 0 && c.w > 0 && len(pg.items) <= pg.size && pg.total > 0 }";
        let r = rule_of(src, "M", "r");
        assert_eq!(r.reads, vec!["c", "pg"]);
        assert!(
            codes("model P { q: int }\nmodel M { p: P\n rule r: p.zz > 0 }").contains(&"E0901")
        );
        assert!(
            codes("model P { q: int }\nmodel M { pg: Page<P>\n rule r: pg.nope > 0 }")
                .contains(&"E0901")
        );
    }
}

mod calls {
    use super::*;

    const F: &str =
        " s: string\n n: int\n l: long\n xs: int[]\n m: Map<string, int>\n by: bytes\n opt?: int";

    #[test]
    fn the_builtins_type_as_the_spec_says() {
        assert_eq!(
            typed(F, "len(s) > len(xs)"),
            "(gt (len self.s) (len self.xs))"
        );
        assert_eq!(
            typed(F, "len(m) + len(by) == 0"),
            "(eq (add (len self.m) (len self.by)) int:0)"
        );
        assert_eq!(
            typed(
                F,
                "startsWith(s, \"a\") && endsWith(s, s) && contains(s, \"x\")"
            ),
            "(and (and (startswith self.s \"a\") (endswith self.s self.s)) (contains self.s \"x\"))"
        );
        assert_eq!(
            typed(F, "matches(s, \"^a+$\")"),
            "(matches self.s \"^a+$\")"
        );
        assert_eq!(
            typed(F, "sum(xs, x => x) == n"),
            "(eq (sum self.xs x => $x) self.n)"
        );
        assert_eq!(
            typed(F, "count(xs, x => x > 0) < 2"),
            "(lt (count self.xs x => (gt $x int:0)) int:2)"
        );
        assert_eq!(
            typed(F, "all(xs, x => x > n)"),
            "(all self.xs x => (gt $x self.n))"
        );
        assert_eq!(
            typed(F, "any(xs, x => x > 0)"),
            "(any self.xs x => (gt $x int:0))"
        );
        assert!(first_code(F, "days(n) > hours(l)").is_empty());
        assert!(first_code(F, "minutes(n) < seconds(2) + ms(3)").is_empty());
    }

    #[test]
    fn bad_calls_are_e0906() {
        for rule in [
            "size(xs) > 0",
            "min(n, 1) > 0",
            "len() > 0",
            "len(s, s) > 0",
            "len(n) > 0",
            "startsWith(s) ",
            "contains(n, s)",
            "matches(s, s)",
            "matches(s, 5)",
            "len(x => x) > 0",
            "all(xs) ",
            "all(xs, 5)",
            "all(s, x => true)",
            "days(s) > days(1)",
            "has(n + 1)",
            "has(n)",
        ] {
            assert_eq!(first_code(F, rule), vec!["E0906"], "`{rule}`");
        }
    }

    #[test]
    fn matches_checks_the_regex_subset() {
        assert_eq!(first_code(F, "matches(s, \"(?=a)\")"), vec!["E0603"]);
        assert!(first_code(F, "matches(s, \"^[a-z]{2,3}$\")").is_empty());
    }

    #[test]
    fn lambda_bodies_have_a_type() {
        assert_eq!(first_code(F, "all(xs, x => x)"), vec!["E0902"]);
        assert_eq!(first_code(F, "sum(xs, x => x > 0) > 0"), vec!["E0902"]);
        assert_eq!(first_code(F, "sum(xs, x => x) > 0"), Vec::<&str>::new());
    }

    #[test]
    fn nested_comprehensions_warn_unless_both_lists_are_bounded() {
        let src = |a: &str, b: &str| {
            codes(&format!(
                "model R {{ cs: int[] {b} }}\nmodel M {{ rows: R[] {a}\n rule r: all(rows, r => all(r.cs, c => c > 0)) }}"
            ))
        };
        assert_eq!(src("", ""), vec!["W0902"]);
        assert_eq!(src("@length(max: 5)", ""), vec!["W0902"]);
        assert_eq!(src("", "@length(max: 5)"), vec!["W0902"]);
        assert!(src("@length(max: 5)", "@length(max: 5)").is_empty());
    }
}

mod guards {
    use super::*;

    const F: &str = " a?: int\n b?: int\n nick?: string\n addr?: A2\n items: A2[]";

    fn guarded(rule: &str) -> Vec<&'static str> {
        codes(&format!(
            "model A2 {{ city: string\n zip?: string }}\nmodel M {{\n{F}\n rule r: {rule}\n}}"
        ))
    }

    #[test]
    fn every_accepted_form() {
        for rule in [
            "has(a) && a > 0",
            "has(a) implies a > 0",
            "!has(a) || a > 0",
            "not has(a) || a > 0",
            "has(a) && has(b) && a < b",
            "has(a) && has(b) implies a < b",
            "!has(a) || !has(b) || a < b",
            "has(addr) && has(addr.zip) && len(addr.zip) > 0",
            "has(addr.zip) && len(addr.zip) > 0 && len(addr.city) > 0",
            "has(a) && (a > 1 || a < 0)",
            "all(items, i => has(i.zip) implies len(i.zip) > 0)",
            "all(items, i => !has(i.zip) || len(i.zip) > 0)",
            "has(nick) && len(nick) > 0 || !has(nick)",
            "(has(a)) && a > 0",
            "!(has(a) && has(b)) || a < b",
        ] {
            assert!(
                guarded(rule).is_empty(),
                "`{rule}` must compile: {:?}",
                guarded(rule)
            );
        }
    }

    #[test]
    fn every_unguarded_read() {
        for rule in [
            "a > 0",
            "has(a) || a > 0",
            "a > 0 && has(a)",
            "has(b) && a > 0",
            "!has(a) && a > 0",
            "a > 0 implies has(a)",
            "has(a) implies a > 0 && b > 0",
            "has(addr) && len(addr.zip) > 0",
            "all(items, i => len(i.zip) > 0)",
            "has(a) || has(b) && a > 0",
            "len(nick) > 0",
        ] {
            assert!(
                guarded(rule).contains(&"E0903"),
                "`{rule}` must be E0903, got {:?}",
                guarded(rule)
            );
        }
    }

    #[test]
    fn the_reads_of_a_guarded_rule_include_the_has_argument() {
        let src = "model A2 { zip?: string }\nmodel M { n?: int\n m: int\n addr?: A2\n rule r: has(addr.zip) && m > 0 && has(n) implies n > m }";
        let r = rule_of(src, "M", "r");
        assert_eq!(r.reads, vec!["addr", "m", "n"]);
    }

    #[test]
    fn a_field_with_a_default_is_always_present() {
        assert!(codes("model M { n: int = 3\n rule r: n > 0 }").is_empty());
    }
}

mod declarations {
    use super::*;

    #[test]
    fn rules_are_inherited_first_with_the_declarers_id() {
        let src = "model A { x: int\n rule ra: x > 0 }\nmodel B extends A { y: int\n rule rb: y > x }\nmodel C extends B { rule rc: x < y }";
        let ir = compiled(src);
        let rules = |m: &str| -> Vec<(String, Option<String>, String)> {
            let d = ir.decls.iter().find(|d| d.name == m).unwrap();
            d.rules()
                .iter()
                .map(|r| {
                    (
                        r.name.clone(),
                        r.inherited_from.map(|f| ir.decl(f).name.clone()),
                        r.id.clone(),
                    )
                })
                .collect()
        };
        let (a, b, c) = (rules("A"), rules("B"), rules("C"));
        assert_eq!(
            b.iter()
                .map(|r| (r.0.as_str(), r.1.as_deref()))
                .collect::<Vec<_>>(),
            vec![("ra", Some("A")), ("rb", None)]
        );
        assert_eq!(
            c.iter()
                .map(|r| (r.0.as_str(), r.1.as_deref()))
                .collect::<Vec<_>>(),
            vec![("ra", Some("A")), ("rb", Some("B")), ("rc", None)]
        );
        assert_eq!(a[0].2, b[0].2);
        assert_eq!(a[0].2, c[0].2);
        assert_eq!(b[1].2, c[1].2);
        assert!(a[0].2.starts_with("veld:app/A.ra@"));
        assert!(c[1].2.starts_with("veld:app/B.rb@"));
        assert!(c[2].2.starts_with("veld:app/C.rc@"));
    }

    #[test]
    fn duplicate_names_count_inherited_rules() {
        assert_eq!(
            codes("model M { a: int\n rule r: a > 0\n rule r: a < 9 }"),
            vec!["E0907"]
        );
        assert_eq!(
            codes("model A { a: int\n rule r: a > 0 }\nmodel B extends A { rule r: a < 9 }"),
            vec!["E0907"]
        );
        assert_eq!(
            codes(
                "model A { a: int\n rule r: a > 0 }\nmodel B extends A {}\nmodel C extends B { rule r: a < 9 }"
            ),
            vec!["E0907"]
        );
        // A rule and a field may share a name; siblings may reuse a rule name.
        assert!(
            codes("model A { r: int\n rule r: r > 0 }\nmodel B { r: int\n rule r: r > 0 }")
                .is_empty()
        );
    }

    #[test]
    fn a_rule_that_reads_no_field_warns() {
        assert_eq!(codes("model M { n: int\n rule r: 1 < 2 }"), vec!["W0903"]);
        assert_eq!(
            codes("const K: int = 1\nmodel M { n: int\n rule r: K > 0 }"),
            vec!["W0903"]
        );
        assert!(codes("model M { n: int\n rule r: n > 0 }").is_empty());
        // Reported once, where it is written, not again for each child.
        assert_eq!(
            codes("model A { n: int\n rule r: true }\nmodel B extends A {}"),
            vec!["W0903"]
        );
    }

    #[test]
    fn reads_follow_first_use_in_the_text_and_mark_readonly() {
        let src =
            "model M { b: int\n a: int @readonly\n c: int\n rule r: c > a && a > b && c > 0 }";
        let r = rule_of(src, "M", "r");
        assert_eq!(r.reads, vec!["c", "a", "b"]);
        assert!(r.reads_readonly);
        let r = rule_of(
            "model M { a: int @readonly\n b: int\n rule r: b > 0 }",
            "M",
            "r",
        );
        assert!(!r.reads_readonly);
    }

    #[test]
    fn reads_use_source_names_not_wire_names() {
        let r = rule_of(
            "model M { first: int @wire(\"f\")\n rule r: first > 0 }",
            "M",
            "r",
        );
        assert_eq!(r.reads, vec!["first"]);
        let ExprKind::Bin(b) = &r.expr.e else {
            panic!("not a comparison")
        };
        let ExprKind::Path(p) = &b.l.e else {
            panic!("not a path")
        };
        assert_eq!(
            (p.steps[0].name.as_str(), p.steps[0].wire.as_str()),
            ("first", "f")
        );
    }

    #[test]
    fn messages_docs_and_deprecation_reach_the_ir() {
        let src = "model M { n: int\n /// Positive.\n @deprecated(\"use q\")\n rule r: n > 0 message \"must be positive\"\n rule q: n > 1 }";
        let r = rule_of(src, "M", "r");
        assert_eq!(r.message.as_deref(), Some("must be positive"));
        assert_eq!(r.docs.as_deref(), Some("Positive."));
        assert_eq!(r.deprecated.as_deref(), Some("use q"));
        let q = rule_of(src, "M", "q");
        assert_eq!((q.message, q.docs, q.deprecated), (None, None, None));
    }

    #[test]
    fn errors_carry_rules_and_the_ir_json_omits_an_empty_list() {
        let ir = compiled("error Bad 422 { n: int\n rule r: n > 0 }\nmodel Plain { n: int }");
        let bad = ir.decls.iter().find(|d| d.name == "Bad").unwrap();
        assert_eq!(bad.rules().len(), 1);
        let json = serde_json::to_value(&ir).unwrap();
        let find = |name: &str| {
            json["decls"]
                .as_array()
                .and_then(|a| a.iter().find(|d| d["name"] == name))
                .cloned()
                .unwrap()
        };
        assert!(find("Bad").get("rules").is_some());
        assert!(find("Plain").get("rules").is_none());
    }

    #[test]
    fn unknown_annotations_and_wrong_targets_on_a_rule() {
        assert_eq!(
            codes("model M { n: int\n @nope\n rule r: n > 0 }"),
            vec!["E0601"]
        );
        assert_eq!(
            codes("model M { n: int\n @min(1)\n rule r: n > 0 }"),
            vec!["E0602"]
        );
        assert!(codes("model M { n: int\n @deprecated(\"x\")\n rule r: n > 0 }").is_empty());
    }

    #[test]
    fn a_broken_field_does_not_cascade_into_the_rules_that_read_it() {
        assert_eq!(codes("model M { n: Nope\n rule r: n > 0 }"), vec!["E0205"]);
    }

    #[test]
    fn every_scalar_type_is_a_valid_field_in_a_rule() {
        let _ = Scalar::Int;
        assert!(
            codes("model M { u: uuid\n by: bytes\n j: json\n rule r: u == u && len(by) > 0 }")
                .is_empty()
        );
    }
}
