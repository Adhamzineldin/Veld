//! Formatter behaviour for `when flag("name")` and `given flag` (`spec/platform.md` section 11):
//! the suffix stays at the end of its statement, formatting is idempotent, the words stay names.

use super::format;

fn f(src: &str) -> String {
    format(src)
        .unwrap_or_else(|d| format!("ERR {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>()))
}

fn stable(src: &str) -> String {
    let once = f(src);
    assert_eq!(
        f(&once),
        once,
        "not idempotent for:\n{src}\n--- first pass:\n{once}"
    );
    once
}

#[test]
fn an_invariant_keeps_its_flag_after_its_message() {
    let out = stable(
        "model M {\n n: int\n rule a: n > 0   when   flag( \"x\" )\n rule b: n < 9 message \"small\" when flag(\"y.z-1\")\n}",
    );
    assert!(out.contains("rule a: n > 0 when flag(\"x\")"), "{out}");
    assert!(
        out.contains("message \"small\" when flag(\"y.z-1\")"),
        "{out}"
    );
}

#[test]
fn a_require_and_an_ensure_keep_their_flag() {
    let src = "service S at \"/s\" {\n get \"/\" f(@query n: int) -> int {\n  require n > 0 else Bad { n: n } when flag(\"a\")\n  ensure result > 0 when flag(\"b\")\n }\n}";
    let out = stable(src);
    assert!(
        out.contains("require n > 0 else Bad { n: n } when flag(\"a\")"),
        "{out}"
    );
    assert!(out.contains("ensure result > 0 when flag(\"b\")"), "{out}");
}

#[test]
fn a_policy_clause_keeps_its_flag() {
    let src = "policy P(u: User) {\n permit when \"a\" in u.roles when flag(\"beta\")\n forbid when u.bad message \"no\" when flag(\"lock\")\n}";
    let out = stable(src);
    assert!(
        out.contains("permit when \"a\" in u.roles when flag(\"beta\")"),
        "{out}"
    );
    assert!(out.contains("message \"no\" when flag(\"lock\")"), "{out}");
}

#[test]
fn a_flow_keeps_after_then_flag() {
    let out = stable(
        "enum S { a, b, c\n flow a -> b when flag(\"x\")\n flow  a -> c after hours(1)   when flag(\"y\")\n}",
    );
    assert!(out.contains("flow a -> b when flag(\"x\")"), "{out}");
    assert!(
        out.contains("flow a -> c after hours(1) when flag(\"y\")"),
        "{out}"
    );
}

#[test]
fn given_flag_lines_keep_their_order_among_the_givens() {
    let src = "example \"e\" {\n given   flag \"a\"\n given room = { open: true }\n given flag \"b\"   off\n  go(id: room.id) -> ok\n}";
    let out = stable(src);
    let a = out.find("given flag \"a\"").unwrap();
    let room = out.find("given room").unwrap();
    let b = out.find("given flag \"b\" off").unwrap();
    assert!(a < room && room < b, "{out}");
}

#[test]
fn a_comment_after_a_flagged_statement_stays() {
    let out = stable("model M {\n n: int\n rule a: n > 0 when flag(\"x\") // only for beta\n}");
    assert!(out.contains("when flag(\"x\") // only for beta"), "{out}");
}

#[test]
fn flag_words_stay_names() {
    let src = "model M { flag: string, when: int, off: bool }\nservice S at \"/s\" { get \"/\" flag(@query off: int) -> M }";
    stable(src);
}
