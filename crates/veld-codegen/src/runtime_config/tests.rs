use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::*;

fn url(name: &str) -> Setting {
    Setting::service_url(name, Vec::new())
}

fn model(settings: Vec<Setting>) -> RuntimeModel {
    RuntimeModel::from_settings("bookings", settings)
}

fn full() -> RuntimeModel {
    model(vec![url("payments"), Setting::rules_output(), url("iam")])
}

fn paths(m: &RuntimeModel) -> Vec<&str> {
    m.settings().iter().map(|s| s.path.as_str()).collect()
}

fn messages(r: Result<Values, Vec<Problem>>) -> Vec<String> {
    match r {
        Ok(v) => panic!("expected problems, got {v:?}"),
        Err(ps) => ps.into_iter().map(|p| p.message).collect(),
    }
}

fn strings(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

#[test]
fn the_type_name_is_the_pascal_case_target_and_runtime() {
    for (target, expected) in [
        ("bookings", "BookingsRuntime"),
        ("web", "WebRuntime"),
        ("card-service", "CardServiceRuntime"),
        ("my_api", "MyApiRuntime"),
        ("v1.api", "V1ApiRuntime"),
        ("api.v2", "ApiV2Runtime"),
        ("web2", "Web2Runtime"),
        ("orders-API", "OrdersApiRuntime"),
        ("a--b__c", "ABCRuntime"),
        ("3d-app", "T3dAppRuntime"),
        ("---", "TargetRuntime"),
    ] {
        assert_eq!(runtime_type_name(target), expected, "{target}");
    }
}

#[test]
fn settings_are_in_byte_order_of_their_path() {
    let m = model(vec![
        url("a"),
        url("a-b"),
        Setting::rules_output(),
        url("B"),
    ]);
    // `-` (0x2d) sorts before `.` (0x2e), so `a-b` comes before `a`; `B` before `a`.
    assert_eq!(
        paths(&m),
        [
            "rules.output",
            "services.B.url",
            "services.a-b.url",
            "services.a.url"
        ]
    );
}

#[test]
fn a_model_knows_its_urls_and_what_is_required() {
    let m = full();
    assert_eq!(m.type_name(), "BookingsRuntime");
    assert_eq!(m.urls().count(), 2);
    assert_eq!(m.required().count(), 2);
    assert!(m.has_required());
    let only_rules = model(vec![Setting::rules_output()]);
    assert!(!only_rules.has_required());
    assert_eq!(
        m.setting("rules.output").map(|s| s.default.as_deref()),
        Some(Some("strict"))
    );
    assert!(m.setting("rules.ouput").is_none());
}

#[test]
fn the_messages_are_the_text_of_the_spec() {
    assert_eq!(missing("services.iam.url"), "services.iam.url is missing");
    assert_eq!(
        not_a_url("services.iam.url"),
        "services.iam.url must be an http or https URL"
    );
    assert_eq!(
        not_in_list("rules.output", RULE_OUTPUT_VALUES),
        "rules.output must be \"strict\", \"audit\" or \"off\""
    );
    assert_eq!(not_in_list("x", &["a", "b"]), "x must be \"a\" or \"b\"");
    assert_eq!(not_in_list("x", &["a"]), "x must be \"a\"");
    assert_eq!(not_a_setting("rules.ouput"), "rules.ouput is not a setting");
    assert_eq!(
        missing_env("services.iam.url", "IAM_URL"),
        "services.iam.url is missing (environment variable IAM_URL is not set)"
    );
    assert_eq!(not_an_object("rules"), "rules must be an object");
}

#[test]
fn the_error_text_joins_the_problems_in_order() {
    let problems = vec![
        Problem::new("a", "a is missing".into()),
        Problem::new("b", "b is not a setting".into()),
    ];
    assert_eq!(
        error_text(&problems),
        "invalid runtime config: a is missing; b is not a setting"
    );
}

#[test]
fn urls_follow_the_literal_rule() {
    for ok in [
        "http://iam:3001",
        "https://iam.internal",
        "HTTP://IAM",
        "http://user:pw@iam:80/base",
        "http://[::1]:8080",
        "http://[::1]",
        "http://a_b.c-d/path?x=1#f",
        "http://host:65535",
    ] {
        assert!(normalize_url(ok).is_some(), "{ok}");
    }
    for bad in [
        "",
        "iam:3001",
        "ftp://iam",
        "http://",
        "http:///x",
        "https://:80",
        "http://host:",
        "http://host:65536",
        "http://host:123456",
        "http://host:80a",
        "http://ho st",
        " http://host",
        "http://host ",
        "http://host\n",
        "http://hôst",
        "http://[]",
        "http://[::1",
        "http://a:b:c",
        "http://@",
        "/relative",
        "https:/host",
    ] {
        assert!(normalize_url(bad).is_none(), "{bad:?}");
    }
}

#[test]
fn every_trailing_slash_is_dropped() {
    assert_eq!(
        normalize_url("http://iam:3001/").as_deref(),
        Some("http://iam:3001")
    );
    assert_eq!(normalize_url("http://iam//").as_deref(), Some("http://iam"));
    assert_eq!(
        normalize_url("http://iam/v1/").as_deref(),
        Some("http://iam/v1")
    );
}

#[test]
fn a_correct_value_gives_its_values_with_defaults_and_without_slashes() {
    let v = json!({ "services": { "iam": { "url": "http://iam:3001/" }, "payments": { "url": "https://pay" } } });
    let got = full().check(&v).unwrap();
    assert_eq!(
        got,
        strings(&[
            ("rules.output", "strict"),
            ("services.iam.url", "http://iam:3001"),
            ("services.payments.url", "https://pay"),
        ])
    );
    let v = json!({ "rules": { "output": "audit" }, "services": { "iam": { "url": "http://i" }, "payments": { "url": "http://p" } } });
    assert_eq!(full().check(&v).unwrap()["rules.output"], "audit");
}

#[test]
fn every_problem_is_reported_at_once_in_path_order() {
    let v = json!({
        "rules": { "ouput": "off", "output": "loud" },
        "services": { "iam": { "url": "iam:3001" }, "cards": { "url": "http://c" } },
        "extra": 1
    });
    let m = messages(full().check(&v));
    assert_eq!(
        m,
        [
            "extra is not a setting",
            "rules.ouput is not a setting",
            "rules.output must be \"strict\", \"audit\" or \"off\"",
            "services.cards is not a setting",
            "services.iam.url must be an http or https URL",
            "services.payments.url is missing",
        ]
    );
}

#[test]
fn a_required_value_that_is_null_absent_or_empty_is_missing() {
    for v in [
        json!({}),
        Value::Null,
        json!({ "services": null }),
        json!({ "services": { "iam": null, "payments": { "url": null } } }),
        json!({ "services": { "iam": { "url": "" }, "payments": { "url": "" } } }),
    ] {
        assert_eq!(
            messages(full().check(&v)),
            [
                "services.iam.url is missing",
                "services.payments.url is missing"
            ],
            "{v}"
        );
    }
}

#[test]
fn a_value_of_the_wrong_json_type_is_the_wrong_value() {
    let v = json!({ "rules": { "output": 1 }, "services": { "iam": { "url": 5 }, "payments": { "url": ["x"] } } });
    assert_eq!(
        messages(full().check(&v)),
        [
            "rules.output must be \"strict\", \"audit\" or \"off\"",
            "services.iam.url must be an http or https URL",
            "services.payments.url must be an http or https URL",
        ]
    );
}

#[test]
fn an_object_position_that_holds_something_else_is_not_an_object() {
    let m = messages(full().check(&json!({ "services": "x", "rules": 3 })));
    assert_eq!(
        m,
        [
            "rules must be an object",
            "services must be an object",
            "services.iam.url is missing",
            "services.payments.url is missing"
        ]
    );
    assert_eq!(
        messages(full().check(&json!(5))),
        [
            "runtime config must be an object",
            "services.iam.url is missing",
            "services.payments.url is missing"
        ]
    );
}

#[test]
fn a_dotted_key_is_not_a_shortcut_to_a_path() {
    let v = json!({ "services.iam.url": "http://i", "services": { "payments": { "url": "http://p" } } });
    let m = messages(full().check(&v));
    assert_eq!(
        m,
        [
            "services.iam.url is missing",
            "services.iam.url is not a setting"
        ]
    );
}

#[test]
fn the_optional_setting_keeps_its_default_and_an_empty_value_is_not_one() {
    let urls =
        json!({ "services": { "iam": { "url": "http://i" }, "payments": { "url": "http://p" } } });
    assert_eq!(full().check(&urls).unwrap()["rules.output"], "strict");
    let v = json!({ "rules": { "output": "" }, "services": urls["services"] });
    assert_eq!(
        messages(full().check(&v)),
        ["rules.output must be \"strict\", \"audit\" or \"off\""]
    );
    let v = json!({ "rules": { "output": null }, "services": urls["services"] });
    assert_eq!(full().check(&v).unwrap()["rules.output"], "strict");
}

#[test]
fn a_map_uses_paths_as_keys() {
    let m = full();
    let ok = strings(&[
        ("services.iam.url", "http://iam/"),
        ("services.payments.url", "http://pay"),
        ("rules.output", "off"),
    ]);
    assert_eq!(m.check_map(&ok).unwrap()["rules.output"], "off");
    let bad = strings(&[("services.iam.url", "nope"), ("rules.ouput", "off")]);
    assert_eq!(
        messages(m.check_map(&bad)),
        [
            "rules.ouput is not a setting",
            "services.iam.url must be an http or https URL",
            "services.payments.url is missing"
        ]
    );
}

#[test]
fn from_env_reads_the_named_variables() {
    let m = full();
    let names = strings(&[
        ("services.iam.url", "IAM_URL"),
        ("services.payments.url", "PAY_URL"),
        ("rules.output", "RULES"),
    ]);
    let env = strings(&[("IAM_URL", "http://iam/"), ("PAY_URL", "http://pay")]);
    let got = m.check_env(&names, &env).unwrap();
    assert_eq!(got["services.iam.url"], "http://iam");
    assert_eq!(
        got["rules.output"], "strict",
        "an optional variable that is not set keeps the default"
    );
    let env = strings(&[
        ("IAM_URL", "http://iam"),
        ("PAY_URL", "http://pay"),
        ("RULES", "audit"),
    ]);
    assert_eq!(m.check_env(&names, &env).unwrap()["rules.output"], "audit");
}

#[test]
fn from_env_reports_the_variable_that_is_not_set_and_checks_what_is() {
    let m = full();
    let names = strings(&[
        ("services.iam.url", "IAM_URL"),
        ("services.payments.url", "PAY_URL"),
        ("rules.output", "RULES"),
        ("rules.ouput", "TYPO"),
    ]);
    // An empty variable counts as not set.
    let env = strings(&[("IAM_URL", ""), ("RULES", "loud")]);
    assert_eq!(
        messages(m.check_env(&names, &env)),
        [
            "rules.ouput is not a setting",
            "rules.output must be \"strict\", \"audit\" or \"off\"",
            "services.iam.url is missing (environment variable IAM_URL is not set)",
            "services.payments.url is missing (environment variable PAY_URL is not set)",
        ]
    );
}

#[test]
fn from_env_without_a_name_for_a_required_path_is_missing() {
    let m = full();
    let names = strings(&[("services.iam.url", "IAM_URL")]);
    let env = strings(&[("IAM_URL", "http://iam")]);
    assert_eq!(
        messages(m.check_env(&names, &env)),
        ["services.payments.url is missing"]
    );
}

#[test]
fn a_model_with_no_setting_checks_only_for_unknown_keys() {
    let m = model(Vec::new());
    assert!(m.check(&json!({})).unwrap().is_empty());
    assert_eq!(
        messages(m.check(&json!({ "a": 1 }))),
        ["a is not a setting"]
    );
}
