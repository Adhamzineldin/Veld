//! Events in `veld diff` (`spec/events.md` section 16). Event-aware classification is a later
//! phase: every change to an event, a channel, `publishes` or `subscribes` is one entry, "possibly
//! breaking".

use super::ChangeKind::{self, *};
use super::Severity::{self, PossiblyBreaking as P};
use super::{diff, has_breaking};
use crate::testutil::ir;

fn run(old: &str, new: &str) -> Vec<(ChangeKind, Severity)> {
    let mut v: Vec<_> = diff(&ir(old), &ir(new))
        .into_iter()
        .map(|c| (c.kind, c.severity))
        .collect();
    v.sort();
    v
}

const BASE: &str = "event Paid { id: uuid }\nevent Gone { id: uuid }\n";
const PUB: &str = "service P at \"/p\" {\n  publishes C\n  get \"/\" ping() -> 204\n}\n";

fn contract(events: &str, channel: &str, services: &str) -> String {
    format!("{events}{channel}{services}")
}

fn sub(group: &str, handlers: &str) -> String {
    format!("service S {{\n  subscribes C as \"{group}\" {{\n{handlers}  }}\n}}\n")
}

const CH: &str = "channel C { carries Paid, Gone }\n";

#[test]
fn identical_event_contracts_have_no_changes() {
    let src = contract(
        BASE,
        CH,
        &format!("{PUB}{}", sub("g", "    on Paid -> a\n")),
    );
    assert!(run(&src, &src).is_empty());
}

#[test]
fn an_added_removed_or_changed_event_is_one_possibly_breaking_entry() {
    let svc = format!("{PUB}{}", sub("g", "    on Paid -> a\n"));
    let old = contract(BASE, CH, &svc);
    let added = contract(&format!("{BASE}event New {{ id: uuid }}\n"), CH, &svc);
    assert_eq!(run(&old, &added), [(EventAdded, P)]);
    assert_eq!(run(&added, &old), [(EventRemoved, P)]);
    let changed = contract(
        "event Paid { id: uuid\n note?: string }\nevent Gone { id: uuid }\n",
        CH,
        &svc,
    );
    assert_eq!(run(&old, &changed), [(EventChanged, P)]);
    assert!(has_breaking(&diff(&ir(&old), &ir(&changed))));
}

#[test]
fn a_changed_rule_of_an_event_is_one_entry_not_two() {
    let svc = PUB.to_string();
    let a = contract(
        "event Paid { n: int\n rule r: n > 0 }\n",
        "channel C { carries Paid }\n",
        &svc,
    );
    let b = contract(
        "event Paid { n: int\n rule r: n > 1 }\n",
        "channel C { carries Paid }\n",
        &svc,
    );
    assert_eq!(run(&a, &b), [(EventChanged, P)]);
}

#[test]
fn a_channel_that_changes_topic_key_or_events_is_one_entry() {
    let old = contract(BASE, CH, PUB);
    let topic = contract(BASE, &format!("@topic(\"a.b\")\n{CH}"), PUB);
    assert_eq!(run(&old, &topic), [(ChannelChanged, P)]);
    let key = contract(BASE, "channel C { carries Paid, Gone\n key id }\n", PUB);
    assert_eq!(run(&old, &key), [(ChannelChanged, P)]);
    let fewer = contract(BASE, "channel C { carries Paid }\n", PUB);
    assert_eq!(run(&old, &fewer), [(ChannelChanged, P)]);
    let reordered = contract(BASE, "channel C { carries Gone, Paid }\n", PUB);
    assert!(run(&old, &reordered).is_empty());
}

#[test]
fn an_added_or_removed_channel_is_one_entry() {
    let old = contract(BASE, CH, PUB);
    let more = contract(BASE, &format!("{CH}channel D {{ carries Paid }}\n"), PUB);
    assert_eq!(run(&old, &more), [(ChannelAdded, P)]);
    assert_eq!(run(&more, &old), [(ChannelRemoved, P)]);
}

#[test]
fn a_service_that_gains_or_loses_a_published_channel_is_one_entry() {
    let two = format!("{BASE}channel C {{ carries Paid }}\nchannel D {{ carries Gone }}\n");
    let one = "service P at \"/p\" {\n  publishes C\n  get \"/\" ping() -> 204\n}\n";
    let both = "service P at \"/p\" {\n  publishes C, D\n  get \"/\" ping() -> 204\n}\n";
    assert_eq!(
        run(&format!("{two}{one}"), &format!("{two}{both}")),
        [(PublishesAdded, P)]
    );
    assert_eq!(
        run(&format!("{two}{both}"), &format!("{two}{one}")),
        [(PublishesRemoved, P)]
    );
}

#[test]
fn a_subscription_that_appears_disappears_or_changes_is_one_entry() {
    let events = format!("{BASE}channel C {{ carries Paid, Gone }}\n");
    let with = |s: &str| format!("{events}{PUB}{s}");
    let none = "service S {}\n".to_string();
    let a = sub("g", "    on Paid -> a\n");
    assert_eq!(run(&with(&none), &with(&a)), [(SubscriptionAdded, P)]);
    assert_eq!(run(&with(&a), &with(&none)), [(SubscriptionRemoved, P)]);
    let group = sub("other", "    on Paid -> a\n");
    assert_eq!(run(&with(&a), &with(&group)), [(SubscriptionChanged, P)]);
    let handler = sub("g", "    on Paid -> renamed\n");
    assert_eq!(run(&with(&a), &with(&handler)), [(SubscriptionChanged, P)]);
    let more = sub("g", "    on Paid -> a\n    on Gone -> b\n");
    assert_eq!(run(&with(&a), &with(&more)), [(SubscriptionChanged, P)]);
}

#[test]
fn a_new_service_that_publishes_or_subscribes_reports_each_line() {
    let events = format!("{BASE}channel C {{ carries Paid, Gone }}\n");
    let old = format!(
        "{events}service Other at \"/o\" {{\n  publishes C\n  get \"/\" ping() -> 204\n}}\n"
    );
    let new = format!("{old}{}", sub("g", "    on Paid -> a\n"));
    assert_eq!(run(&old, &new), [(SubscriptionAdded, P)]);
}

#[test]
fn a_contract_that_gains_events_never_reports_models_or_endpoints_as_changed() {
    let old = "model M { x: int }\nservice S at \"/s\" {\n  get \"/\" f() -> M\n}\n";
    let new = format!(
        "{old}event E {{ x: int }}\nchannel C {{ carries E }}\nservice P {{ publishes C }}\n"
    );
    assert_eq!(
        run(old, &new),
        [(EventAdded, P), (ChannelAdded, P), (PublishesAdded, P)].map(|x| x)
    );
}
