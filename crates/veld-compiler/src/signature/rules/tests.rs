//! Rule ids (`spec/rules.md` section 6).

use super::*;
use crate::testing::ir;
use veld_ir::Ir;

fn id_of(src: &str, model: &str, rule: &str) -> String {
    let ir = ir(src);
    ir.decls
        .iter()
        .find(|d| d.name == model)
        .and_then(|d| d.rules().iter().find(|r| r.name == rule))
        .map(|r| r.id.clone())
        .unwrap_or_else(|| panic!("no rule {model}.{rule}"))
}

fn sig_of(ir: &Ir, model: &str) -> String {
    ir.decls
        .iter()
        .find(|d| d.name == model)
        .map(|d| d.sig.clone())
        .unwrap_or_default()
}

#[test]
fn the_id_has_the_documented_form() {
    let id = id_of("model M { a: int\n rule r: a > 0 }", "M", "r");
    assert!(id.starts_with("veld:app/M.r@"), "{id}");
    let hex = &id["veld:app/M.r@".len()..];
    assert_eq!(hex.len(), 16);
    assert!(hex.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
}

#[test]
fn the_hashed_bytes_are_the_canonical_expr_shape() {
    let ir = ir("model M { a: int @wire(\"A\")\n rule r: a > 0 }");
    let m = ir.decls.iter().position(|d| d.name == "M").unwrap();
    let rule = &ir.decls[m].rules()[0];
    assert_eq!(
        rule_bytes(&ir.decls, m, "r", &rule.expr),
        r#"{"d":"app:app.M","e":{"e":{"bin":{"l":{"e":{"path":{"root":"self","steps":[{"w":"A"}]}},"t":"int"},"op":"gt","r":{"e":{"lit":{"int":"0"}},"t":"int"}}},"t":"bool"},"n":"r"}"#
    );
}

#[test]
fn renaming_a_field_while_keeping_its_wire_name_keeps_the_id() {
    let a = id_of("model M { a: int @wire(\"x\")\n rule r: a > 0 }", "M", "r");
    let b = id_of(
        "model M { renamed: int @wire(\"x\")\n rule r: renamed > 0 }",
        "M",
        "r",
    );
    assert_eq!(a, b);
    let c = id_of("model M { a: int @wire(\"y\")\n rule r: a > 0 }", "M", "r");
    assert_ne!(a, c, "the wire name is part of the rule");
}

#[test]
fn renaming_a_lambda_variable_keeps_the_id() {
    let src = |v: &str, w: &str| {
        format!(
            "model R {{ q: int[] }}\nmodel M {{ xs: R[]\n rule r: all(xs, {v} => all({v}.q, {w} => {w} > 0)) }}"
        )
    };
    let a = id_of(&src("row", "cell"), "M", "r");
    assert_eq!(a, id_of(&src("x", "y"), "M", "r"));
    assert_eq!(a, id_of(&src("p", "q2"), "M", "r"));
    let swapped =
        "model R { q: int[] }\nmodel M { xs: R[]\n rule r: all(xs, a => all(a.q, b => b < 0)) }";
    assert_ne!(a, id_of(swapped, "M", "r"));
}

#[test]
fn the_message_docs_and_annotations_are_not_part_of_the_id() {
    let plain = id_of("model M { a: int\n rule r: a > 0 }", "M", "r");
    let dressed =
        "model M { a: int\n /// Docs.\n @deprecated(\"later\")\n rule r: a > 0 message \"text\" }";
    assert_eq!(plain, id_of(dressed, "M", "r"));
    let other = "model M { a: int\n rule r: a > 0 message \"other\" }";
    assert_eq!(plain, id_of(other, "M", "r"));
}

#[test]
fn changing_the_expression_the_name_or_the_declaration_changes_the_id() {
    let base = id_of("model M { a: int\n rule r: a > 0 }", "M", "r");
    for other in [
        "model M { a: int\n rule r: a >= 0 }",
        "model M { a: int\n rule r: a > 1 }",
        "model M { a: int\n rule r: 0 < a }",
        "model M { a: int\n rule r: a > 0 && true }",
        "model M { a: long\n rule r: a > 0 }",
    ] {
        assert_ne!(base, id_of(other, "M", "r"), "{other}");
    }
    assert_ne!(base, id_of("model M { a: int\n rule s: a > 0 }", "M", "s"));
    assert_ne!(base, id_of("model N { a: int\n rule r: a > 0 }", "N", "r"));
}

#[test]
fn numbers_are_written_as_text() {
    let dec = |lit: &str| {
        id_of(
            &format!("model M {{ d: decimal\n rule r: d > {lit} }}"),
            "M",
            "r",
        )
    };
    assert_ne!(dec("1.5"), dec("1.50"), "a decimal is its source text");
    assert_eq!(dec("1.5"), dec("1.5"));
    let flt = |lit: &str| {
        id_of(
            &format!("model M {{ f: float\n rule r: f < {lit} }}"),
            "M",
            "r",
        )
    };
    assert_eq!(flt("0.5"), flt("0.50"), "a float is its IEEE 754 bits");
    assert_ne!(flt("0.5"), flt("0.25"));
    let ir = ir("model M { f: float\n l: long\n rule r: f < 0.5 && l > 5000000000 }");
    let m = ir.decls.iter().position(|d| d.name == "M").unwrap();
    let bytes = rule_bytes(&ir.decls, m, "r", &ir.decls[m].rules()[0].expr);
    assert!(bytes.contains(r#"{"float":"3fe0000000000000"}"#), "{bytes}");
    assert!(bytes.contains(r#"{"long":"5000000000"}"#), "{bytes}");
}

#[test]
fn enum_literals_are_written_by_declaration_name_and_wire_value() {
    let a = id_of(
        "enum S { paid = \"PAID\", open }\nmodel M { s: S\n rule r: s == .paid }",
        "M",
        "r",
    );
    let renamed = id_of(
        "enum S { settled = \"PAID\", open }\nmodel M { s: S\n rule r: s == .settled }",
        "M",
        "r",
    );
    assert_eq!(a, renamed, "the variant is identified by its wire value");
    let other = id_of(
        "enum S { paid = \"DONE\", open }\nmodel M { s: S\n rule r: s == .paid }",
        "M",
        "r",
    );
    assert_ne!(a, other);
}

#[test]
fn an_inherited_rule_keeps_the_id_of_the_declaration_that_wrote_it() {
    let src = "model A { a: int\n rule r: a > 0 }\nmodel B extends A { b: int }";
    assert_eq!(id_of(src, "A", "r"), id_of(src, "B", "r"));
}

#[test]
fn rules_never_change_the_declaration_signature() {
    let without = ir(
        "model M { a: int\n b?: string }\nmodel E extends M { c: int }\nerror X 409 { m: string }",
    );
    let with = ir(
        "model M { a: int\n b?: string\n rule r: a > 0\n rule s: !has(b) || len(b) > 0 }\nmodel E extends M { c: int\n rule t: c > a }\nerror X 409 { m: string\n rule u: len(m) > 0 }",
    );
    for name in ["M", "E", "X"] {
        assert_eq!(sig_of(&without, name), sig_of(&with, name), "{name}");
    }
    assert!(with.decls.iter().any(|d| !d.rules().is_empty()));
}
