//! Formatter behaviour for derived fields (`spec/rules.md` section 17.2).

use super::format;

fn f(src: &str) -> String {
    format(src)
        .unwrap_or_else(|d| format!("ERR {:?}", d.iter().map(|x| x.code).collect::<Vec<_>>()))
}

/// Formats once, checks the result is a fixed point, and returns it.
fn stable(src: &str) -> String {
    let once = f(src);
    assert_eq!(f(&once), once, "not idempotent for:\n{src}");
    once
}

#[test]
fn a_derived_field_keeps_its_expression_and_loses_nothing() {
    let out = stable(
        "model O {\n  items: L[]\n  discount: decimal   =   0\n  subtotal: decimal = sum(items,i=>i.price*i.qty)\n  total: decimal = subtotal-discount\n}",
    );
    assert_eq!(
        out,
        "model O {\n  items: L[]\n  discount: decimal = 0\n  subtotal: decimal = sum(items, i => i.price * i.qty)\n  total: decimal = subtotal - discount\n}\n"
    );
}

#[test]
fn the_expression_is_printed_with_the_fewest_parentheses() {
    let out =
        stable("model M {\n  a: int\n  b: int = (a + 1) * (a + 2)\n  c: int = ((a)) + (1 * 2)\n}");
    assert_eq!(
        out,
        "model M {\n  a: int\n  b: int = (a + 1) * (a + 2)\n  c: int = a + 1 * 2\n}\n"
    );
}

#[test]
fn a_default_stays_a_default_and_a_negative_literal_stays_one() {
    let src = "model M {\n  a: int = -1\n  b: float = -0.5\n  c: string = \"x\"\n  d: bool = true\n  e?: string = null\n  f: int[] = []\n}\n";
    assert_eq!(stable(src), src);
}

#[test]
fn a_literal_in_parentheses_stays_an_expression() {
    // Without the parentheses the field would turn into a field with a default.
    let out = stable("model M {\n  a: int = (1)\n  b: int = (-1)\n  c: bool = (true)\n}");
    assert_eq!(
        out,
        "model M {\n  a: int = (1)\n  b: int = (-1)\n  c: bool = (true)\n}\n"
    );
}

#[test]
fn a_negated_name_and_a_negated_literal_are_expressions() {
    let out = stable("model M {\n  a: int\n  b: int = -a\n  c: int = -(1)\n}");
    assert_eq!(
        out,
        "model M {\n  a: int\n  b: int = -a\n  c: int = -(1)\n}\n"
    );
}

#[test]
fn annotations_after_a_derived_field_stay_on_its_line() {
    let out = stable("model M {\n  a: int\n  b: int = a + 1 @deprecated(\"use a\")\n}");
    assert_eq!(
        out,
        "model M {\n  a: int\n  b: int = a + 1 @deprecated(\"use a\")\n}\n"
    );
}

#[test]
fn derived_fields_of_an_error_and_docs_and_comments_survive() {
    let out = stable(
        "// the error\nerror Low 409 {\n  /// what is needed\n  need: int // wanted\n  have: int\n  gap: int = need - have // the difference\n}",
    );
    assert!(out.contains("gap: int = need - have"), "{out}");
    assert!(out.contains("// the difference"), "{out}");
    assert!(out.contains("/// what is needed"), "{out}");
}

#[test]
fn rules_still_follow_the_fields_after_one_blank_line() {
    let out = stable("model M {\n  rule r: b > 0\n  a: int\n  b: int = a * 2\n}");
    assert_eq!(
        out,
        "model M {\n  a: int\n  b: int = a * 2\n\n  rule r: b > 0\n}\n"
    );
}
