//! `@rollout(audit)` in the IR (`spec/rules.md` section 17.1).

use serde_json::json;

use super::*;

fn audited(rules: &mut EndpointRules) {
    rules.requires[0].rollout = Some(Rollout::Audit);
}

#[test]
fn an_enforced_statement_has_no_rollout_key() {
    let v = serde_json::to_value(sample_rules()).unwrap();
    assert!(v["requires"][0].get("rollout").is_none(), "{v}");
    assert!(v["ensures"][0].get("rollout").is_none(), "{v}");
}

#[test]
fn an_audited_statement_writes_rollout_audit_and_reads_it_back() {
    let mut rules = sample_rules();
    audited(&mut rules);
    rules.ensures[0].rollout = Some(Rollout::Audit);
    let v = serde_json::to_value(&rules).unwrap();
    assert_eq!(v["requires"][0]["rollout"], json!("audit"));
    assert_eq!(v["ensures"][0]["rollout"], json!("audit"));
    assert!(v["requires"][1].get("rollout").is_none(), "{v}");
    let back: EndpointRules = serde_json::from_value(v).unwrap();
    assert_eq!(back, rules);
}

#[test]
fn an_audited_statement_uses_the_rollout_feature_and_an_enforced_one_does_not() {
    let plain = ir_with(
        vec![service_with(vec![endpoint_with(sample_rules())])],
        vec![],
    );
    assert!(
        !plain
            .features_used(&[ServiceId(0)])
            .contains(&Feature::RulesRollout)
    );
    let mut rules = sample_rules();
    audited(&mut rules);
    assert_eq!(rules.first_audited_origin().map(|o| o.line), Some(11));
    let ir = ir_with(vec![service_with(vec![endpoint_with(rules)])], vec![]);
    let used = ir.features_used(&[ServiceId(0)]);
    assert!(used.contains(&Feature::RulesRollout), "{used:?}");
    assert!(used.contains(&Feature::RulesEndpoint), "{used:?}");
}

#[test]
fn the_first_audited_origin_is_the_earliest_of_requires_and_ensures() {
    let mut rules = sample_rules();
    rules.ensures[0].rollout = Some(Rollout::Audit);
    assert_eq!(rules.first_audited_origin().map(|o| o.line), Some(13));
    rules.requires[1].rollout = Some(Rollout::Audit);
    assert_eq!(rules.first_audited_origin().map(|o| o.line), Some(12));
    assert!(EndpointRules::default().first_audited_origin().is_none());
}

#[test]
fn the_feature_has_a_name_and_is_opt_in() {
    assert_eq!(Feature::RulesRollout.name(), "rules_rollout");
    assert_eq!(
        Feature::from_name("rules_rollout"),
        Some(Feature::RulesRollout)
    );
    assert!(Feature::ALL.contains(&Feature::RulesRollout));
    assert!(Feature::OPT_IN.contains(&Feature::RulesRollout));
    assert!(!Feature::RULES.contains(&Feature::RulesRollout));
}
