//! `auth-required` (`L0014`, `spec/rules.md` section 11.6).

use std::collections::BTreeMap;

use super::*;
use crate::testutil::ir;

fn subjects(src: &str) -> Vec<String> {
    lint(&ir(src), &LintConfig::default())
        .into_iter()
        .filter(|i| i.rule == "auth-required")
        .map(|i| i.subject)
        .collect()
}

const MODEL: &str = "model A { x: int }\n";

#[test]
fn an_endpoint_with_no_auth_anywhere_is_reported() {
    let src =
        format!("{MODEL}service S at \"/s\" {{\n  get \"/\" f() -> A\n  get \"/g\" g() -> A\n}}");
    assert_eq!(subjects(&src), ["S.f", "S.g"]);
    let issue = lint(&ir(&src), &LintConfig::default())
        .into_iter()
        .find(|i| i.rule == "auth-required")
        .unwrap();
    assert_eq!(issue.code, "L0014");
    assert_eq!(issue.level, LintLevel::Warn);
    assert_eq!(issue.origin.file, "app.veld");
    assert!(issue.message.contains("@auth(none)"), "{}", issue.message);
}

#[test]
fn auth_none_on_the_endpoint_silences_it() {
    let src = format!(
        "{MODEL}service S at \"/s\" {{\n  @auth(none)\n  get \"/\" f() -> A\n  get \"/g\" g() -> A\n}}"
    );
    assert_eq!(subjects(&src), ["S.g"]);
}

#[test]
fn auth_on_the_service_silences_every_endpoint_of_it() {
    for auth in ["@auth(bearer)", "@auth(basic)", "@auth(none)"] {
        let src = format!(
            "{MODEL}{auth}\nservice S at \"/s\" {{\n  get \"/\" f() -> A\n  get \"/g\" g() -> A\n}}"
        );
        assert!(subjects(&src).is_empty(), "{auth}");
    }
}

#[test]
fn an_endpoint_can_make_up_for_a_service_without_auth() {
    let src = format!(
        "{MODEL}service S at \"/s\" {{\n  @auth(bearer)\n  get \"/\" f() -> A\n  get \"/g\" g() -> A\n}}"
    );
    assert_eq!(subjects(&src), ["S.g"]);
}

#[test]
fn an_endpoint_that_overrides_the_auth_of_its_service_is_not_reported() {
    let src = format!(
        "{MODEL}@auth(bearer)\nservice S at \"/s\" {{\n  @auth(none)\n  get \"/\" f() -> A\n}}"
    );
    assert!(subjects(&src).is_empty());
}

#[test]
fn the_level_is_configurable() {
    let src = format!("{MODEL}service S at \"/s\" {{\n  get \"/\" f() -> A\n}}");
    let ir = ir(&src);
    let off: BTreeMap<String, LintLevel> = [("auth-required".to_string(), LintLevel::Off)].into();
    let cfg = LintConfig::from_levels(off.iter()).unwrap();
    assert!(lint(&ir, &cfg).iter().all(|i| i.rule != "auth-required"));
    let error: BTreeMap<String, LintLevel> =
        [("auth-required".to_string(), LintLevel::Error)].into();
    let cfg = LintConfig::from_levels(error.iter()).unwrap();
    let found: Vec<_> = lint(&ir, &cfg)
        .into_iter()
        .filter(|i| i.rule == "auth-required")
        .collect();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].level, LintLevel::Error);
    assert!(has_errors(&found));
}

#[test]
fn the_principal_and_policy_parameters_are_used_models() {
    let src = "model User { id: uuid }\nprincipal User\nmodel Room { id: uuid }\npolicy P(r: Room) { permit when true }\n@auth(none)\nservice S at \"/s\" {\n  get \"/\" f() -> int\n}";
    let found = lint(&ir(src), &LintConfig::default());
    assert!(found.iter().all(|i| i.rule != "unused-model"), "{found:?}");
}

#[test]
fn the_rule_is_named_by_the_code_table() {
    let names = rule_names();
    assert!(names.contains(&("auth-required", "L0014")));
}
