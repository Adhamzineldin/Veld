//! Parser tests for `when flag("name")` and `given flag` (`spec/platform.md` section 11).

use super::{Parsed, parse};
use crate::diagnostics::FileId;
use crate::syntax::ast::{DeclKind, ExampleBody, Stmt};

fn clean(src: &str) -> Parsed {
    let r = parse(src, FileId(0));
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    r
}

fn codes(src: &str) -> Vec<&'static str> {
    parse(src, FileId(0))
        .diagnostics
        .iter()
        .map(|d| d.code)
        .collect()
}

#[test]
fn an_invariant_has_its_flag_after_the_message() {
    let r = clean(
        "model M { n: int\n rule a: n > 0 message \"m\" when flag(\"beta\")\n rule b: n > 1 }",
    );
    let DeclKind::Model(m) = &r.file.decls[0].kind else {
        panic!("not a model")
    };
    assert_eq!(
        m.rules[0].flag.as_ref().map(|f| f.name.as_str()),
        Some("beta")
    );
    assert!(m.rules[0].message.is_some());
    assert!(m.rules[1].flag.is_none());
}

#[test]
fn require_ensure_and_clauses_take_the_suffix() {
    let src = "policy P(u: User) {\n permit when u.a when flag(\"p\")\n forbid when u.b message \"x\" when flag(\"q\")\n}\n\
               service S at \"/s\" { get \"/\" f(@query n: int) -> int {\n  require n > 0 else Bad when flag(\"r\")\n  ensure result > 0 when flag(\"e\")\n }\n}";
    let r = clean(src);
    let flags: Vec<Option<&str>> = r.file.policies[0]
        .clauses
        .iter()
        .map(|c| c.flag.as_ref().map(|f| f.name.as_str()))
        .collect();
    assert_eq!(flags, [Some("p"), Some("q")]);
    let DeclKind::Service(s) = &r.file.decls[0].kind else {
        panic!("not a service")
    };
    let block = s.endpoints[0].block.as_ref().unwrap();
    let got: Vec<Option<&str>> = block
        .stmts
        .iter()
        .map(|st| match st {
            Stmt::Require(x) => x.flag.as_ref().map(|f| f.name.as_str()),
            Stmt::Ensure(x) => x.flag.as_ref().map(|f| f.name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(got, [Some("r"), Some("e")]);
}

#[test]
fn a_flow_with_one_edge_takes_the_suffix_and_several_edges_do_not() {
    let r = clean("enum S { a, b\n flow a -> b when flag(\"x\")\n}");
    let DeclKind::Enum(e) = &r.file.decls[0].kind else {
        panic!("not an enum")
    };
    assert_eq!(e.flows[0].flag.as_ref().map(|f| f.name.as_str()), Some("x"));
    assert_eq!(
        codes("enum S { a, b, c\n flow a -> b -> c when flag(\"x\")\n}"),
        ["E0937"]
    );
}

#[test]
fn a_bad_name_is_e0937() {
    for name in ["", "1a", "a b", "a/b", &"a".repeat(65)] {
        let src = format!("model M {{ n: int\n rule a: n > 0 when flag(\"{name}\") }}");
        assert_eq!(codes(&src), ["E0937"], "{name:?}");
    }
    for name in ["a", "Beta.desk-2_x", &"a".repeat(64)] {
        let src = format!("model M {{ n: int\n rule a: n > 0 when flag(\"{name}\") }}");
        assert!(codes(&src).is_empty(), "{name:?}");
    }
}

#[test]
fn the_suffix_on_anything_else_is_e0937() {
    let fact = "service S at \"/s\" { get \"/\" f(@query n: int) -> int {\n fact a: A = n when flag(\"x\")\n }\n}";
    assert_eq!(codes(fact), ["E0937"]);
    let transition = "service S at \"/s\" { get \"/\" f(@query n: int) -> int {\n fact a: A = n\n transition a.s: x -> y when flag(\"x\")\n }\n}";
    assert_eq!(codes(transition), ["E0937"]);
    let allow = "service S at \"/s\" { get \"/\" f(@query n: int) -> int {\n allow P(n) when flag(\"x\")\n }\n}";
    assert_eq!(codes(allow), ["E0937"]);
    assert_eq!(
        codes("enum S { a, b\n flow a -> b\n final b when flag(\"x\")\n}"),
        ["E0937"]
    );
}

#[test]
fn given_flag_has_an_on_and_an_off_form_and_a_fact_called_flag_stays_a_fact() {
    let src = "example \"e\" {\n given flag \"a\"\n given flag \"b\" off\n given flag = absent\n go() -> ok\n}";
    let r = clean(src);
    let ExampleBody::Call(_) = &r.file.examples[0].body else {
        panic!("not a call")
    };
    let given: Vec<(&str, Option<bool>)> = r.file.examples[0]
        .givens
        .iter()
        .map(|g| (g.name.name.as_str(), g.flag))
        .collect();
    assert_eq!(
        given,
        [("a", Some(true)), ("b", Some(false)), ("flag", None)]
    );
}

#[test]
fn flag_words_stay_ordinary_names() {
    let src = "model M { flag: string, when: int, off: bool }\n\
               enum E { flag, off\n flow flag -> off\n}\n\
               service S at \"/s\" {\n get \"/a\" flag(@query off: int) -> M\n get \"/b\" off() -> M\n}";
    assert!(parse(src, FileId(0)).diagnostics.is_empty());
}
