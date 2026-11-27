//! The checker and the evaluator beyond the shared vectors: order, ids, messages, wire form,
//! partial mode, unavailable roots and malformed input.

use serde_json::{Value as Json, json};
use veld_ir::{Builtin, Decl, Ir, Literal, Root};

use super::kit::*;
use super::models;
use crate::{Env, EvalError, Failure, InputError, Options, Value, Violation, check_value, eval};

fn decl<'a>(ir: &'a Ir, name: &str) -> &'a Decl {
    ir.decls.iter().find(|d| d.name == name).unwrap()
}

fn check(
    ir: &Ir,
    name: &str,
    value: &Json,
    options: Options,
) -> Result<Vec<Violation>, InputError> {
    check_value(ir, decl(ir, name), value, options)
}

fn names(vs: &[Violation]) -> Vec<(&str, &str)> {
    vs.iter()
        .map(|v| (v.rule_name.as_str(), v.path.as_str()))
        .collect()
}

const REQUEST: Options = Options {
    request: true,
    partial: false,
};
const PARTIAL: Options = Options {
    request: false,
    partial: true,
};

#[test]
fn inherited_rules_run_before_the_childs_own() {
    let ir = models::nested();
    let v = json!({ "low": 5, "high": 1, "step": 9 });
    let out = check(&ir, "Range", &v, Options::default()).unwrap();
    assert_eq!(names(&out), [("ordered", "$"), ("stepFits", "$")]);
    let range = decl(&ir, "Range");
    let ids: Vec<&str> = range.rules().iter().map(|r| r.name.as_str()).collect();
    assert_eq!(ids, ["ordered", "stepFits"]);
}

#[test]
fn own_rules_come_before_the_rules_of_nested_values_in_field_order() {
    let ir = models::nested();
    let v = json!({
        "rows": [{ "low": 2, "high": 1, "step": 0 }],
        "named": { "k": { "low": 2, "high": 1 } },
        "main": { "low": 2, "high": 1, "step": 0 },
        "page": { "items": [{ "low": 2, "high": 1 }], "total": "1", "page": 1, "size": 0 }
    });
    let out = check(&ir, "Grid", &v, Options::default()).unwrap();
    assert_eq!(
        names(&out),
        [
            ("pageFull", "$"),
            ("ordered", "$.rows[0]"),
            ("stepFits", "$.rows[0]"),
            ("ordered", "$.named.k"),
            ("ordered", "$.main"),
            ("stepFits", "$.main"),
            ("ordered", "$.page.items[0]"),
        ]
    );
}

#[test]
fn every_nested_position_is_checked() {
    let ir = models::nested();
    let bad = json!({ "low": 2, "high": 1 });
    let v = json!({
        "rows": [{ "low": 1, "high": 2, "step": 1 }, { "low": 3, "high": 4, "step": 1 }],
        "named": { "a": { "low": 0, "high": 0 }, "b.c": bad },
        "page": { "items": [{ "low": 0, "high": 0 }, bad], "total": "2", "page": 1, "size": 2 }
    });
    let out = check(&ir, "Grid", &v, Options::default()).unwrap();
    assert_eq!(
        names(&out),
        [("ordered", "$.named.b.c"), ("ordered", "$.page.items[1]")]
    );
}

#[test]
fn a_union_arm_is_picked_by_its_discriminator() {
    let ir = models::nested();
    let v = json!({ "payment": { "kind": "bank", "iban": "x" }, "backup": { "kind": "card", "last4": "1" } });
    let out = check(&ir, "Checkout", &v, Options::default()).unwrap();
    assert_eq!(names(&out), [("four", "$.backup")]);
    let unknown = json!({ "payment": { "kind": "cash" } });
    assert!(matches!(
        check(&ir, "Checkout", &unknown, Options::default()),
        Err(InputError::Unsupported(_))
    ));
    let missing = json!({ "payment": { "iban": "x" } });
    assert!(matches!(
        check(&ir, "Checkout", &missing, Options::default()),
        Err(InputError::MissingField { .. })
    ));
}

#[test]
fn an_absent_optional_value_is_not_checked() {
    let ir = models::nested();
    let v = json!({ "rows": [{ "low": 1, "high": 2, "step": 1 }], "named": {}, "page": { "items": [], "total": "0", "page": 1, "size": 1 } });
    assert!(
        check(&ir, "Grid", &v, Options::default())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn readonly_rules_are_skipped_only_in_request_position() {
    let ir = models::nested();
    let draft = decl(&ir, "Draft");
    let owner = draft
        .rules()
        .iter()
        .find(|r| r.name == "ownerNamed")
        .unwrap();
    assert!(owner.reads_readonly);
    let v = json!({ "title": "x" });
    assert!(check(&ir, "Draft", &v, REQUEST).unwrap().is_empty());
    // As a response the readonly fields are required, so the same value is malformed.
    assert!(matches!(
        check(&ir, "Draft", &v, Options::default()),
        Err(InputError::MissingField { .. })
    ));
    let response =
        json!({ "id": "a1b2c3d4-0000-0000-0000-000000000001", "title": "x", "createdBy": "" });
    let out = check(&ir, "Draft", &response, Options::default()).unwrap();
    assert_eq!(names(&out), [("ownerNamed", "$")]);
}

#[test]
fn defaults_apply_before_rules_and_inside_nested_values() {
    let mut k = Kit::new();
    let line = k.model(
        "Line",
        vec![
            field("qty", int()),
            dflt(field("price", dec()), Literal::Int(0)),
            dflt(field("big", long()), Literal::Int(7)),
        ],
        vec![
            rule("cheap", lt(p("price", dec()), dc("1"))),
            rule("big", eq(p("big", long()), lng(7))),
        ],
    );
    k.model(
        "Cart",
        vec![
            field("lines", list_of(line.clone())),
            dflt(field("tags", list_of(string())), Literal::EmptyList),
        ],
        vec![rule("some", gt(len(p("tags", list_of(string()))), i(0)))],
    );
    let ir = k.ir();
    let out = check(
        &ir,
        "Cart",
        &json!({ "lines": [{ "qty": 1 }, { "qty": 2, "price": "5" }] }),
        Options::default(),
    )
    .unwrap();
    assert_eq!(names(&out), [("some", "$"), ("cheap", "$.lines[1]")]);
}

#[test]
fn the_message_and_the_wire_form_of_a_violation() {
    let mut k = Kit::new();
    let f = vec![
        wired(field("startsAt", datetime()), "starts_at"),
        wired(field("endsAt", datetime()), "ends_at"),
    ];
    let order = lt(
        pw("startsAt", "starts_at", datetime()),
        pw("endsAt", "ends_at", datetime()),
    );
    k.model("Slot", f, vec![rule("order", order)]);
    let ir = k.ir();
    let v = json!({ "starts_at": "2026-01-02T00:00:00Z", "ends_at": "2026-01-01T00:00:00Z" });
    let out = check(&ir, "Slot", &v, Options::default()).unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].fields, ["starts_at", "ends_at"]);
    assert_eq!(out[0].message, "violates rule order");
    assert_eq!(
        serde_json::to_value(&out[0]).unwrap(),
        json!({
            "rule": "veld:t/Slot.order@0000000000000000",
            "path": "$",
            "fields": ["starts_at", "ends_at"],
            "message": "violates rule order"
        })
    );
}

#[test]
fn a_declared_message_is_used_and_an_error_replaces_it() {
    let mut ir = models::numbers();
    let idx = ir.decls.iter().position(|d| d.name == "AddInt").unwrap();
    if let veld_ir::DeclKind::Model(m) = &mut ir.decls[idx].kind {
        m.rules[0].message = Some("too big".to_owned());
    }
    let out = check(
        &ir,
        "AddInt",
        &json!({ "a": 60, "b": 40 }),
        Options::default(),
    )
    .unwrap();
    assert_eq!(out[0].message, "too big");
    assert_eq!(out[0].error, None);
    let out = check(
        &ir,
        "AddInt",
        &json!({ "a": i32::MAX, "b": 1 }),
        Options::default(),
    )
    .unwrap();
    assert_eq!(out[0].message, "rule small could not be evaluated");
    assert_eq!(out[0].error, Some(EvalError::Overflow));
    let wire = serde_json::to_value(&out[0]).unwrap();
    assert_eq!(wire["error"], "overflow");
    assert_eq!(wire["rule"], "veld:t/AddInt.small@0000000000000000");
}

#[test]
fn the_rules_of_an_error_declaration_are_checked() {
    let mut k = Kit::new();
    k.error(
        "OutOfStock",
        vec![field("wanted", int()), field("have", int())],
        vec![rule("short", gt(p("wanted", int()), p("have", int())))],
    );
    let ir = k.ir();
    let out = check(
        &ir,
        "OutOfStock",
        &json!({ "wanted": 1, "have": 5 }),
        Options::default(),
    )
    .unwrap();
    assert_eq!(names(&out), [("short", "$")]);
    assert!(
        check(
            &ir,
            "OutOfStock",
            &json!({ "wanted": 9, "have": 5 }),
            Options::default()
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn short_circuit_hides_the_error_of_the_skipped_operand() {
    let ir = models::numbers();
    let guarded = json!({ "a": 10, "b": 0 });
    let out = check(&ir, "Guarded", &guarded, Options::default()).unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].error, None);
    // Without the guard the same division is an error.
    let mut k = Kit::new();
    let f = vec![field("a", int()), field("b", int())];
    k.model(
        "Eager",
        f,
        vec![rule(
            "r",
            or(gt(div(p("a", int()), p("b", int())), i(1)), b(true)),
        )],
    );
    let ir = k.ir();
    let out = check(&ir, "Eager", &guarded, Options::default()).unwrap();
    assert_eq!(out[0].error, Some(EvalError::DivisionByZero));
    let mut k = Kit::new();
    let f = vec![field("a", int()), field("b", int())];
    let boom = gt(div(p("a", int()), p("b", int())), i(1));
    k.model(
        "Implied",
        f,
        vec![
            rule("r", implies(b(false), boom.clone())),
            rule("s", or(b(true), boom.clone())),
            rule("t", and(b(false), boom)),
        ],
    );
    let ir = k.ir();
    let out = check(&ir, "Implied", &guarded, Options::default()).unwrap();
    assert_eq!(names(&out), [("t", "$")]);
}

#[test]
fn quantifiers_stop_at_the_first_decisive_element() {
    let mut k = Kit::new();
    let nums = || p("n", list_of(int()));
    let boom = || gt(div(i(1), v("x", &[], int())), i(0));
    k.model(
        "Q",
        vec![field("n", list_of(int()))],
        vec![
            rule(
                "all",
                quant(
                    veld_ir::Quantifier::All,
                    nums(),
                    "x",
                    boom(),
                    et(&boolean()),
                ),
            ),
            rule(
                "any",
                quant(
                    veld_ir::Quantifier::Any,
                    nums(),
                    "x",
                    boom(),
                    et(&boolean()),
                ),
            ),
        ],
    );
    let ir = k.ir();
    let verdicts = |out: &[Violation]| -> Vec<(String, Option<EvalError>)> {
        out.iter().map(|v| (v.rule_name.clone(), v.error)).collect()
    };
    // `all` stops at -1 (1 / -1 > 0 is false) before it reaches 0; `any` does not stop
    // there and divides by zero.
    let out = check(&ir, "Q", &json!({ "n": [-1, 0] }), Options::default()).unwrap();
    let want = [
        ("all".to_owned(), None),
        ("any".to_owned(), Some(EvalError::DivisionByZero)),
    ];
    assert_eq!(verdicts(&out), want);
    // `any` stops at 1 (1 / 1 > 0 is true) before it reaches 0; `all` goes on and fails.
    let out = check(&ir, "Q", &json!({ "n": [1, 0] }), Options::default()).unwrap();
    assert_eq!(
        verdicts(&out),
        [("all".to_owned(), Some(EvalError::DivisionByZero))]
    );
}

#[test]
fn the_partial_check_skips_rules_that_read_absent_required_fields() {
    let ir = models::nested();
    let out = check(&ir, "Form", &json!({}), PARTIAL).unwrap();
    assert!(out.is_empty());
    let out = check(
        &ir,
        "Form",
        &json!({ "guests": 0, "note": "long!" }),
        PARTIAL,
    )
    .unwrap();
    assert_eq!(names(&out), [("some", "$"), ("noteShort", "$")]);
    // Only the complete check treats a missing required field as malformed input.
    assert!(matches!(
        check(&ir, "Form", &json!({ "guests": 0 }), Options::default()),
        Err(InputError::MissingField { .. })
    ));
}

#[test]
fn the_partial_check_reaches_nested_values_and_applies_defaults() {
    let ir = models::nested();
    let v = json!({ "payment": { "kind": "card" } });
    assert!(check(&ir, "Checkout", &v, PARTIAL).unwrap().is_empty());
    let v = json!({ "payment": { "kind": "card", "last4": "1" } });
    assert_eq!(
        names(&check(&ir, "Checkout", &v, PARTIAL).unwrap()),
        [("four", "$.payment")]
    );
    let out = check(&ir, "Draft", &json!({ "title": "" }), PARTIAL).unwrap();
    assert_eq!(names(&out), [("titled", "$")]);
}

#[test]
fn rules_that_read_only_guarded_optionals_always_run_in_partial_mode() {
    let ir = models::optional();
    let out = check(&ir, "Profile", &json!({ "nick": "toolong" }), PARTIAL).unwrap();
    assert_eq!(names(&out), [("nickShort", "$"), ("both", "$")]);
}

fn expr_env_value() -> Json {
    json!({ "a": 3 })
}

#[test]
fn roots_that_do_not_exist_yet_are_clear_input_errors() {
    let ir = models::numbers();
    let this = expr_env_value();
    let env = Env::for_value(&ir, &this);
    let not_available =
        |e: Result<Value, Failure>| matches!(e, Err(Failure::Input(InputError::NotAvailable(_))));
    assert!(not_available(eval(&env, &now())));
    assert!(not_available(eval(
        &env,
        &root(Root::Param("body".to_owned()), "x", int())
    )));
    assert!(not_available(eval(
        &env,
        &root(Root::Fact("room".to_owned()), "x", int())
    )));
    assert!(not_available(eval(
        &env,
        &root(Root::Principal, "id", int())
    )));
    assert!(not_available(eval(&env, &root(Root::Result, "id", int()))));
    let old = mk(et(&int()), veld_ir::ExprKind::Old(Box::new(p("a", int()))));
    assert!(not_available(eval(&env, &old)));
    let bare = Env::new(&ir);
    assert!(not_available(eval(&bare, &p("a", int()))));
}

#[test]
fn the_environment_can_already_carry_the_phase_two_roots() {
    let ir = models::numbers();
    let this = expr_env_value();
    let mut env = Env::for_value(&ir, &this);
    env.now = Some(1_000);
    env.params.insert("body".to_owned(), json!({ "n": 4 }));
    env.principal = Some(json!({ "n": 1 }));
    assert_eq!(eval(&env, &now()).unwrap(), Value::Datetime(1_000));
    let n = root(Root::Param("body".to_owned()), "n", int());
    assert_eq!(
        eval(&env, &add(n, root(Root::Principal, "n", int()))).unwrap(),
        Value::Int(5)
    );
}

#[test]
fn eval_reports_the_three_evaluation_errors() {
    let ir = models::numbers();
    let this = json!({ "a": i32::MAX, "b": 0 });
    let env = Env::for_value(&ir, &this);
    let a = || p("a", int());
    assert_eq!(
        eval(&env, &add(a(), i(1))),
        Err(Failure::Eval(EvalError::Overflow))
    );
    assert_eq!(
        eval(&env, &div(a(), p("b", int()))),
        Err(Failure::Eval(EvalError::DivisionByZero))
    );
    assert_eq!(
        eval(&env, &mul(fl(1e308), fl(10.0))),
        Err(Failure::Eval(EvalError::NonFinite))
    );
    assert_eq!(
        eval(&env, &dur(Builtin::Days, lng(106_751_991_168))),
        Err(Failure::Eval(EvalError::Overflow))
    );
    assert_eq!(eval(&env, &len(s("\u{1F600}"))).unwrap(), Value::Int(1));
}

#[test]
fn decimal_division_and_unsupported_patterns_are_input_errors() {
    let ir = models::decimal();
    let this = json!({ "a": "1", "b": "2" });
    let env = Env::for_value(&ir, &this);
    let q = div(p("a", dec()), p("b", dec()));
    assert!(matches!(
        eval(&env, &q),
        Err(Failure::Input(InputError::Unsupported(_)))
    ));
    let lookahead = text_fn(Builtin::Matches, s("x"), s("(?=x)"));
    assert!(matches!(
        eval(&env, &lookahead),
        Err(Failure::Input(InputError::Unsupported(_)))
    ));
}

#[test]
fn malformed_values_are_input_errors_never_panics() {
    let ir = models::decimal();
    let opts = Options::default();
    let bad = |v: Json| check(&ir, "DecEq", &v, opts);
    assert!(matches!(
        bad(json!({ "a": 1, "b": "1" })),
        Err(InputError::Mismatch { .. })
    ));
    assert!(matches!(
        bad(json!({ "a": "1.", "b": "1" })),
        Err(InputError::InvalidScalar { .. })
    ));
    assert!(matches!(
        bad(json!({ "a": "1e3", "b": "1" })),
        Err(InputError::InvalidScalar { .. })
    ));
    assert!(matches!(
        bad(json!({ "a": "1" })),
        Err(InputError::MissingField { .. })
    ));
    assert!(matches!(
        bad(json!(["a"])),
        Err(InputError::Mismatch { .. })
    ));
    assert!(matches!(bad(json!(null)), Err(InputError::Mismatch { .. })));
    assert!(matches!(
        bad(json!({ "a": null, "b": "1" })),
        Err(InputError::Mismatch { .. })
    ));
}

#[test]
fn malformed_scalars_of_every_kind_are_input_errors() {
    let ir = models::numbers();
    let opts = Options::default();
    assert!(check(&ir, "AddInt", &json!({ "a": 1.5, "b": 1 }), opts).is_err());
    assert!(check(&ir, "AddInt", &json!({ "a": "1", "b": 1 }), opts).is_err());
    assert!(check(&ir, "AddInt", &json!({ "a": 2147483648i64, "b": 1 }), opts).is_err());
    assert!(check(&ir, "LongMath", &json!({ "a": 5, "b": "1" }), opts).is_err());
    assert!(check(&ir, "LongMath", &json!({ "a": "+5", "b": "1" }), opts).is_err());
    assert!(
        check(
            &ir,
            "LongMath",
            &json!({ "a": "9223372036854775808", "b": "1" }),
            opts
        )
        .is_err()
    );
    assert!(check(&ir, "LongMath", &json!({ "a": "", "b": "1" }), opts).is_err());
    let ir = models::time_ir();
    assert!(
        check(
            &ir,
            "Stay",
            &json!({ "start": "yesterday", "end": "2026-01-01T00:00:00Z" }),
            opts
        )
        .is_err()
    );
    assert!(
        check(
            &ir,
            "Dates",
            &json!({ "first": "2026-02-30", "last": "2026-03-01" }),
            opts
        )
        .is_err()
    );
    assert!(
        check(
            &ir,
            "Clock",
            &json!({ "opens": "9am", "closes": "10:00:00" }),
            opts
        )
        .is_err()
    );
    let ir = models::optional();
    let t =
        json!({ "id": "x", "owner": "a1b2c3d4-0000-0000-0000-000000000001", "status": "pending" });
    assert!(check(&ir, "Ticket", &t, opts).is_err());
    let t = json!({ "id": "a1b2c3d4-0000-0000-0000-000000000001", "owner": "a1b2c3d4-0000-0000-0000-000000000001", "status": "nope" });
    assert!(check(&ir, "Ticket", &t, opts).is_err());
    let ir = models::strings();
    assert!(
        check(
            &ir,
            "Sizes",
            &json!({ "data": "AQI", "tags": [], "limits": {} }),
            opts
        )
        .is_err()
    );
    assert!(
        check(
            &ir,
            "Sizes",
            &json!({ "data": "AQID", "tags": {}, "limits": {} }),
            opts
        )
        .is_err()
    );
    assert!(
        check(
            &ir,
            "Sizes",
            &json!({ "data": "AQID", "tags": [], "limits": [] }),
            opts
        )
        .is_err()
    );
}

#[test]
fn a_broken_ir_is_an_error_not_a_panic() {
    let ir = models::numbers();
    let this = json!({ "a": 1 });
    let env = Env::for_value(&ir, &this);
    let unbound = v("nope", &[], int());
    assert!(matches!(
        eval(&env, &unbound),
        Err(Failure::Input(InputError::Ir(_)))
    ));
    let wrong = add(b(true), b(false));
    assert!(matches!(
        eval(&env, &wrong),
        Err(Failure::Input(InputError::Ir(_)))
    ));
    let arity = call(Builtin::Len, vec![], et(&int()));
    assert!(matches!(
        eval(&env, &arity),
        Err(Failure::Input(InputError::Ir(_)))
    ));
    let missing_field = p("zzz", int());
    assert!(matches!(
        eval(&env, &missing_field),
        Err(Failure::Input(InputError::MissingField { .. }))
    ));
    let mut broken = models::numbers();
    broken.decls.clear();
    assert!(matches!(
        crate::vectors::run_case(
            &broken,
            &crate::vectors::load_cases(
                r#"{"cases":[{"name":"c","type":"AddInt","value":{},"expect":[]}]}"#
            )
            .unwrap()[0]
        ),
        Err(crate::vectors::Mismatch::UnknownType { .. })
    ));
}

#[test]
fn run_case_reports_a_readable_diff() {
    let ir = models::numbers();
    let text = r#"{"cases":[
        {"name":"wrong","type":"AddInt","value":{"a":60,"b":40},"expect":[{"rule":"other","path":"$","fields":["a"]}]},
        {"name":"input","type":"AddInt","value":{"a":"x","b":1},"expect":[]}
    ]}"#;
    let cases = crate::vectors::load_cases(text).unwrap();
    let diff = crate::vectors::run_case(&ir, &cases[0])
        .unwrap_err()
        .to_string();
    assert!(diff.contains("expected but not reported"));
    assert!(diff.contains("other at $ reading [a]"));
    assert!(diff.contains("small at $ reading [a, b]"));
    let input = crate::vectors::run_case(&ir, &cases[1]).unwrap_err();
    assert!(matches!(input, crate::vectors::Mismatch::Input { .. }));
    assert!(crate::vectors::load_cases("{}").is_err());
}

#[test]
fn the_partial_check_skips_only_when_evaluation_actually_reads_the_absent_field() {
    let ir = models::numbers();
    // `safe` is `b != 0 && a / b > 1`: `a` is never read once `b != 0` is false.
    let out = check(&ir, "Guarded", &json!({ "b": 0 }), PARTIAL).unwrap();
    assert_eq!(names(&out), [("safe", "$")]);
    // With `b` present the division reads `a`, which is absent, so `safe` is skipped.
    let out = check(&ir, "Guarded", &json!({ "b": 2 }), PARTIAL).unwrap();
    assert!(out.is_empty());
}
