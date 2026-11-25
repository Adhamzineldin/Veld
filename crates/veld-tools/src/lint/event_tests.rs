//! Lint rules and events (`spec/events.md` sections 5 and 15).

use super::*;
use crate::testutil::ir;

fn found(src: &str, rule: &str) -> Vec<String> {
    lint(&ir(src), &LintConfig::default())
        .into_iter()
        .filter(|i| i.rule == rule)
        .map(|i| i.subject)
        .collect()
}

const EVENT: &str = "event Paid { id: uuid }\n";

#[test]
fn an_event_that_no_channel_carries_is_unused() {
    let src = format!(
        "{EVENT}event Lonely {{ id: uuid }}\nchannel C {{ carries Paid }}\nservice S {{ publishes C }}\n"
    );
    assert_eq!(found(&src, "unused-model"), ["Lonely"]);
}

#[test]
fn a_carried_event_is_used_and_so_are_the_models_in_its_fields() {
    let src = "model Money { cents: long }\nevent Paid { amount: Money }\nchannel C { carries Paid }\nservice S { publishes C }\n";
    assert!(found(src, "unused-model").is_empty());
}

#[test]
fn a_channel_nobody_publishes_or_subscribes_to_is_unused() {
    let src = format!(
        "{EVENT}channel Used {{ carries Paid }}\nchannel Idle {{ carries Paid }}\nservice S {{ publishes Used }}\n"
    );
    assert_eq!(found(&src, "unused-model"), ["Idle"]);
}

#[test]
fn a_subscription_uses_its_channel() {
    let src = format!(
        "{EVENT}channel C {{ carries Paid }}\nservice P {{ publishes C }}\nservice S {{ subscribes C as \"g\" {{ on Paid -> h }} }}\n"
    );
    assert!(found(&src, "unused-model").is_empty());
}

#[test]
fn publishes_and_subscribes_are_content_for_empty_service() {
    let src = format!(
        "{EVENT}channel C {{ carries Paid }}\nservice P {{ publishes C }}\nservice Q {{ subscribes C as \"g\" {{ on Paid -> h }} }}\nservice Empty {{}}\n"
    );
    assert_eq!(found(&src, "empty-service"), ["Empty"]);
}

#[test]
fn a_service_without_endpoints_is_not_reported_by_auth_required() {
    let src = format!(
        "{EVENT}channel C {{ carries Paid }}\nservice P {{ publishes C }}\nservice Q {{ subscribes C as \"g\" {{ on Paid -> h }} }}\n"
    );
    assert!(found(&src, "auth-required").is_empty());
}
