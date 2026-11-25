//! The expression depth limit and the JSON backstop: every rule that compiles must read back
//! from the IR JSON with `serde_json`'s default settings (`spec/rules.md` section 3.1).

use super::depth::{IR_PREFIX_DEPTH, json_depth};
use crate::testing::one;
use veld_ir::Ir;

const FIELDS: &str = "a: bool\n  n: int\n  xs: int[]\n";

fn source(rule: &str, fields: &str) -> String {
    format!("model M {{\n  {fields}\n  rule r: {rule}\n}}")
}

/// Compiles `rule` on the fields of [`FIELDS`]; the IR when it compiles, else the error codes.
fn build(rule: &str, fields: &str) -> Result<Ir, Vec<&'static str>> {
    let out = one(&source(rule, fields));
    match out.ir {
        Some(ir) => Ok(ir),
        None => Err(out.diagnostics.iter().map(|d| d.code).collect()),
    }
}

/// The whole IR document survives a round trip with the default recursion limit.
fn assert_reads_back(ir: &Ir, what: &str) {
    let text = serde_json::to_string(ir).expect("the IR serialises");
    let back: Ir = match serde_json::from_str(&text) {
        Ok(back) => back,
        Err(e) => panic!("{what}: the IR does not read back: {e}"),
    };
    assert_eq!(&back, ir, "{what}");
}

/// A rule built by `make(n)`: `n = max` compiles and reads back, `n = max + 1` is `E0005`.
fn assert_limit(shape: &str, max: usize, make: impl Fn(usize) -> String) {
    match build(&make(max), FIELDS) {
        Ok(ir) => assert_reads_back(&ir, &format!("{shape} at {max}")),
        Err(c) => panic!("{shape} at {max} should compile: {c:?}"),
    }
    let c = build(&make(max + 1), FIELDS).expect_err("one level more must not compile");
    assert!(c.contains(&"E0005"), "{shape} at {}: {c:?}", max + 1);
}

#[test]
fn a_unary_chain_stops_at_32_levels() {
    assert_limit("!", 32, |n| format!("{}a", "!".repeat(n)));
    assert_limit("not", 32, |n| format!("{}a", "not ".repeat(n)));
}

#[test]
fn a_left_associative_chain_stops_at_32_levels() {
    // 31 `+` and the comparison above them are 32 operators: 32 terms.
    assert_limit("+", 32, |n| format!("{} > 0", vec!["n"; n].join(" + ")));
    // 32 `&&` or `||` operators join 33 terms.
    assert_limit("&&", 33, |n| vec!["a"; n].join(" && "));
    assert_limit("||", 33, |n| vec!["a"; n].join(" || "));
}

#[test]
fn a_right_associative_implies_chain_stops_at_32_levels() {
    assert_limit("implies", 33, |n| vec!["a"; n].join(" implies "));
}

#[test]
fn nested_comprehensions_stop_at_32_levels() {
    // Each comprehension is a call level and a lambda body level in the parser.
    assert_limit("all", 16, |n| {
        let open: String = (0..n).map(|i| format!("all(xs, v{i} => ")).collect();
        format!("{open}a{}", ")".repeat(n))
    });
}

#[test]
fn nested_list_literals_stop_at_32_levels() {
    // The rule, `len(` and its argument take three levels; a bracket and its item two.
    assert_limit("[[..]]", 15, |n| {
        format!("len({}a{}) > 0", "[".repeat(n), "]".repeat(n))
    });
}

/// The IR of a rule that ends in `len(deep) > 0` after `chain` terms joined by `&&`.
fn deep_type_rule(brackets: usize, chain: usize) -> Result<Ir, Vec<&'static str>> {
    let rule = format!("len(deep) > 0{}", " && a".repeat(chain));
    build(
        &rule,
        &format!("a: bool\n  deep: int{}", "[]".repeat(brackets)),
    )
}

#[test]
fn a_deep_type_inside_an_expression_is_caught_by_the_backstop() {
    // The expression has 22 levels, well inside the parser's limit, and the type of its
    // innermost path is a list nested 52 or 53 deep: 3 containers per level (60) plus the
    // leaf (`len(deep) > 0`, 9 + brackets) is 121 at 52 brackets and 122 at 53.
    match deep_type_rule(52, 20) {
        Ok(ir) => assert_reads_back(&ir, "52 brackets"),
        Err(c) => panic!("52 brackets should compile: {c:?}"),
    }
    let c = deep_type_rule(53, 20).expect_err("53 brackets exceed the JSON depth");
    assert_eq!(c, vec!["E0005"]);
    let c = deep_type_rule(64, 20).expect_err("the type limit alone does not catch it");
    assert_eq!(c, vec!["E0005"]);
    // The same type in a short rule is fine.
    assert!(deep_type_rule(64, 0).is_ok());
}

#[test]
fn the_budget_matches_the_real_documents() {
    // At the budget the IR is 126 containers deep (5 above the rule, 121 in it) and a plugin
    // request around it 127: `serde_json` reads the deepest document it accepts.
    let ir = deep_type_rule(52, 20).expect("compiles");
    let whole = serde_json::to_value(&ir).expect("serialises");
    assert_eq!(value_depth(&whole), 126);
    let request = serde_json::json!({ "type": "generate", "ir": whole });
    assert_eq!(value_depth(&request), 127);
    let text = serde_json::to_string(&request).expect("serialises");
    serde_json::from_str::<serde_json::Value>(&text).expect("the plugin request reads back");
    assert_eq!(super::depth::IR_PREFIX_DEPTH, 5);
}

/// Containers nested in a JSON value, counted as `serde_json` counts them.
fn value_depth(v: &serde_json::Value) -> usize {
    match v {
        serde_json::Value::Array(items) => 1 + items.iter().map(value_depth).max().unwrap_or(0),
        serde_json::Value::Object(map) => 1 + map.values().map(value_depth).max().unwrap_or(0),
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => 0,
    }
}

#[test]
fn the_depth_function_matches_the_real_json() {
    let fields = "a: bool\n  n: int\n  d: decimal\n  xs: int[]\n  deep: int[][]\n  s: string\n";
    for rule in [
        "a",
        "!a",
        "n + 1 > 2",
        "d == 1.50",
        "len(deep) > 0",
        "all(xs, x => x > n)",
        "len([[a]]) > 0",
        "matches(s, \"x\") && a",
        "n in [1, 2] implies -1 < n",
        "sum(xs, x => x) == 0",
    ] {
        let ir = build(rule, fields).unwrap_or_else(|c| panic!("`{rule}`: {c:?}"));
        for decl in &ir.decls {
            for r in decl.rules() {
                let value = serde_json::to_value(&r.expr).expect("serialises");
                assert_eq!(json_depth(&r.expr), value_depth(&value), "`{rule}`");
            }
        }
        let whole = serde_json::to_value(&ir).expect("serialises");
        let decl_rule_depth = IR_PREFIX_DEPTH
            + ir.decls
                .iter()
                .flat_map(|d| d.rules())
                .map(|r| json_depth(&r.expr))
                .max()
                .unwrap_or(0);
        assert!(value_depth(&whole) >= decl_rule_depth, "`{rule}`");
    }
}

