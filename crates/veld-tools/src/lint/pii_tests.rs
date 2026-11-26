//! `pii-without-auth` (`L0015`) and `secret-in-output` (`L0016`), `spec/rules.md` section 19.3.

use std::collections::BTreeMap;

use super::*;
use crate::testutil::ir;

fn found(src: &str, rule: &str) -> Vec<LintIssue> {
    lint(&ir(src), &LintConfig::default())
        .into_iter()
        .filter(|i| i.rule == rule)
        .collect()
}

fn subjects(src: &str, rule: &str) -> Vec<String> {
    found(src, rule).into_iter().map(|i| i.subject).collect()
}

const GUEST: &str = "model Guest {\n  name: string @pii\n  password: string @secret\n}\n\
model Login {\n  user: string\n  password: string @secret\n}\n\
model Wrapper { inner: Guest }\n\
error Leak 400 { who: string @pii }\n";

#[test]
fn pii_in_the_answer_of_an_endpoint_without_auth_is_reported() {
    let src = format!("{GUEST}service S at \"/s\" {{\n  @auth(none)\n  get \"/\" f() -> Guest\n}}");
    assert_eq!(subjects(&src, "pii-without-auth"), ["S.f"]);
    let issue = &found(&src, "pii-without-auth")[0];
    assert_eq!(issue.code, "L0015");
    assert_eq!(issue.level, LintLevel::Warn);
    assert!(issue.message.contains("Guest.name"), "{}", issue.message);
}

#[test]
fn an_endpoint_with_no_auth_at_all_counts_like_auth_none() {
    let src = format!("{GUEST}service S at \"/s\" {{\n  get \"/\" f() -> Guest\n}}");
    assert_eq!(subjects(&src, "pii-without-auth"), ["S.f"]);
}

#[test]
fn auth_silences_it_on_the_service_and_on_the_endpoint() {
    let on_service =
        format!("{GUEST}@auth(bearer)\nservice S at \"/s\" {{\n  get \"/\" f() -> Guest\n}}");
    assert!(subjects(&on_service, "pii-without-auth").is_empty());
    let on_endpoint =
        format!("{GUEST}service S at \"/s\" {{\n  @auth(basic)\n  get \"/\" f() -> Guest\n}}");
    assert!(subjects(&on_endpoint, "pii-without-auth").is_empty());
}

#[test]
fn it_reaches_nested_models_declared_errors_and_pages() {
    let nested =
        format!("{GUEST}service S at \"/s\" {{\n  @auth(none)\n  get \"/\" f() -> Wrapper\n}}");
    assert_eq!(subjects(&nested, "pii-without-auth"), ["S.f"]);
    let error =
        format!("{GUEST}service S at \"/s\" {{\n  @auth(none)\n  get \"/\" f() -> int | Leak\n}}");
    assert_eq!(subjects(&error, "pii-without-auth"), ["S.f"]);
    let page =
        format!("{GUEST}service S at \"/s\" {{\n  @auth(none)\n  get \"/\" f() -> Page<Guest>\n}}");
    assert_eq!(subjects(&page, "pii-without-auth"), ["S.f"]);
}

#[test]
fn a_request_body_is_not_output() {
    let src = format!(
        "{GUEST}service S at \"/s\" {{\n  @auth(none)\n  post \"/\" f(body: Guest) -> int\n}}"
    );
    assert!(subjects(&src, "pii-without-auth").is_empty());
}

#[test]
fn a_secret_field_of_a_response_is_reported_once_and_with_the_usual_fix() {
    let src = format!(
        "{GUEST}@auth(bearer)\nservice S at \"/s\" {{\n  get \"/\" f() -> Guest\n  get \"/g\" g() -> Wrapper\n}}"
    );
    assert_eq!(subjects(&src, "secret-in-output"), ["Guest.password"]);
    let issue = &found(&src, "secret-in-output")[0];
    assert_eq!(issue.code, "L0016");
    assert!(
        issue.message.contains("separate model"),
        "{}",
        issue.message
    );
    assert!(issue.message.contains("S.f"), "{}", issue.message);
}

#[test]
fn a_secret_field_only_a_request_carries_is_not_reported() {
    let src = format!(
        "{GUEST}@auth(bearer)\nservice S at \"/s\" {{\n  post \"/login\" login(body: Login) -> int\n}}"
    );
    assert!(subjects(&src, "secret-in-output").is_empty());
}

#[test]
fn a_secret_field_of_an_event_is_reported() {
    let src = "event Signed { token: string @secret }\nchannel Signs { carries Signed }\n";
    assert_eq!(subjects(src, "secret-in-output"), ["Signed.token"]);
}

#[test]
fn an_inherited_secret_is_reported_on_the_parent_that_wrote_it() {
    let src = "model Base { key: string @secret }\nmodel Kid extends Base { n: int }\n\
               @auth(bearer)\nservice S at \"/s\" {\n  get \"/\" f() -> Kid\n}";
    assert_eq!(subjects(src, "secret-in-output"), ["Base.key"]);
}

#[test]
fn the_levels_are_configurable() {
    let src = format!("{GUEST}service S at \"/s\" {{\n  get \"/\" f() -> Guest\n}}");
    let ir = ir(&src);
    for rule in ["pii-without-auth", "secret-in-output"] {
        let off: BTreeMap<String, LintLevel> = [(rule.to_string(), LintLevel::Off)].into();
        let cfg = LintConfig::from_levels(off.iter()).unwrap();
        assert!(lint(&ir, &cfg).iter().all(|i| i.rule != rule), "{rule}");
        let error: BTreeMap<String, LintLevel> = [(rule.to_string(), LintLevel::Error)].into();
        let cfg = LintConfig::from_levels(error.iter()).unwrap();
        let hits: Vec<_> = lint(&ir, &cfg)
            .into_iter()
            .filter(|i| i.rule == rule)
            .collect();
        assert!(!hits.is_empty() && hits.iter().all(|i| i.level == LintLevel::Error));
    }
}

#[test]
fn a_contract_without_the_annotations_is_not_reported() {
    let src = "model A { x: int }\n@auth(none)\nservice S at \"/s\" {\n  get \"/\" f() -> A\n}";
    assert!(subjects(src, "pii-without-auth").is_empty());
    assert!(subjects(src, "secret-in-output").is_empty());
}

#[test]
fn the_rules_are_named_by_the_code_table() {
    let names = rule_names();
    assert!(names.contains(&("pii-without-auth", "L0015")));
    assert!(names.contains(&("secret-in-output", "L0016")));
}

// ---- pii-without-retention (`L0017`, `spec/platform.md` section 23) -------------------------

const GUEST_RETAINED: &str = "@retain(days(30), since: seen, then: anonymize)\nmodel Guest {\n  seen?: datetime\n  name: string @pii\n}\n";

#[test]
fn a_model_with_a_pii_field_and_no_retain_is_reported() {
    let src = "model Guest {\n  name: string @pii\n  phone?: string @pii\n}\n";
    assert_eq!(subjects(src, "pii-without-retention"), ["Guest"]);
    let issue = &found(src, "pii-without-retention")[0];
    assert_eq!(issue.code, "L0017");
    assert_eq!(issue.level, LintLevel::Warn);
    assert!(issue.message.contains("`name`") && issue.message.contains("`phone`"));
}

#[test]
fn retain_silences_it() {
    assert!(subjects(GUEST_RETAINED, "pii-without-retention").is_empty());
}

#[test]
fn a_child_of_a_model_with_pii_needs_its_own_retain() {
    let src = format!("{GUEST_RETAINED}model Vip extends Guest {{\n  tier: int\n}}\n");
    assert_eq!(subjects(&src, "pii-without-retention"), ["Vip"]);
}

#[test]
fn a_secret_is_not_personal_data_and_errors_and_events_are_not_stored_models() {
    let src = "model Key {\n  token: string @secret\n}\nerror Leak 400 {\n  who: string @pii\n}\nevent Registered {\n  email: string @pii\n}\nchannel Registrations { carries Registered }\n";
    assert!(subjects(src, "pii-without-retention").is_empty());
}

#[test]
fn the_retention_level_is_configurable() {
    let src = "model Guest {\n  name: string @pii\n}\n";
    let ir = ir(src);
    let off: BTreeMap<String, LintLevel> =
        [("pii-without-retention".to_string(), LintLevel::Off)].into();
    let cfg = LintConfig::from_levels(off.iter()).unwrap();
    assert!(
        lint(&ir, &cfg)
            .iter()
            .all(|i| i.rule != "pii-without-retention")
    );
    assert!(rule_names().contains(&("pii-without-retention", "L0017")));
}
