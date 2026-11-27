//! Flags in the IR (`spec/platform.md` section 11): the `flag` key of a rule, a `require`, an
//! `ensure`, a policy clause and a flow, and the `flags` of an example, each omitted when absent.

use super::*;

#[test]
fn nothing_is_written_for_a_statement_without_a_flag() {
    let v = serde_json::to_value(flow("pending", "paid")).unwrap();
    assert!(v.get("flag").is_none());
    let clause = &sample_policies()[0].clauses[0];
    let plain = PolicyClause {
        flag: None,
        ..clause.clone()
    };
    let v = serde_json::to_value(&plain).unwrap();
    assert!(v.get("flag").is_none());
}

#[test]
fn a_flagged_statement_writes_its_flag_and_reads_it_back() {
    let flagged = Flow {
        flag: Some("shortStays".into()),
        ..flow("pending", "paid")
    };
    let v = serde_json::to_value(&flagged).unwrap();
    assert_eq!(v["flag"], "shortStays");
    let back: Flow = serde_json::from_value(v).unwrap();
    assert_eq!(back, flagged);
    let rules = sample_rules();
    let v = serde_json::to_value(&rules).unwrap();
    assert_eq!(v["requires"][0]["flag"], "betaDesk");
    assert_eq!(v["ensures"][0]["flag"], "betaDesk");
    let back: EndpointRules = serde_json::from_value(v).unwrap();
    assert_eq!(back, rules);
}

#[test]
fn an_example_writes_the_flags_that_are_on() {
    let ex = sample_examples();
    let v = serde_json::to_value(&ex[0]).unwrap();
    assert_eq!(v["flags"], json!(["betaDesk"]));
    let plain = Example {
        flags: vec![],
        ..ex[0].clone()
    };
    assert!(serde_json::to_value(&plain).unwrap().get("flags").is_none());
}

#[test]
fn flags_use_the_opt_in_feature() {
    assert!(Feature::OPT_IN.contains(&Feature::RulesFlags));
    assert_eq!(Feature::from_name("rules_flags"), Some(Feature::RulesFlags));
}

#[test]
fn the_first_flag_origin_is_a_require_then_an_ensure() {
    let ir = ir_with(vec![], vec![]);
    let rules = sample_rules();
    assert!(rules.first_flag_origin(&ir).is_some());
    let none = EndpointRules::default();
    assert!(none.first_flag_origin(&ir).is_none());
}
