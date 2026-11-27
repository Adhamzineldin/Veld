//! The changed rules that `veld diff --prove` hands to a solver.

use super::{ChangeKind, RuleSite, changed_rules, diff};
use crate::testutil::ir;

const SVC: &str = "service S at \"/s\" { post \"/\" make(body: M) -> 204 }\n";

fn model(rule: &str) -> String {
    format!("model M {{\n a: int\n b: int\n {rule}\n}}\n{SVC}")
}

#[test]
fn a_rule_with_a_new_expression_is_a_changed_rule() {
    let (old, new) = (ir(&model("rule r: a > 0")), ir(&model("rule r: a > 5")));
    let changed = changed_rules(&old, &new);
    assert_eq!(changed.len(), 1);
    let c = &changed[0];
    assert_eq!(c.kind, ChangeKind::InvariantChanged);
    assert!(c.subject.ends_with("M"), "{}", c.subject);
    assert!(c.request && !c.response);
    assert_eq!(c.name(), "rule `r`");
    assert!(matches!(c.site, RuleSite::Invariant { .. }));
    // `diff` reports it as possibly breaking, with the words the marker names.
    let msg = diff(&old, &new)
        .into_iter()
        .find(|d| d.kind == ChangeKind::InvariantChanged)
        .unwrap()
        .message;
    assert!(
        msg.starts_with(&format!("possibly breaking: {}", c.marker())),
        "{msg}"
    );
}

#[test]
fn a_message_added_removed_or_renamed_rule_is_not_a_changed_rule() {
    let old = ir(&model("rule r: a > 0"));
    assert!(changed_rules(&old, &ir(&model("rule r: a > 0 message \"x\""))).is_empty());
    assert!(changed_rules(&old, &ir(&model("rule s: a > 0"))).is_empty());
    assert!(changed_rules(&old, &ir(&model(""))).is_empty());
    // An audited rule never rejects and is judged as absent.
    assert!(changed_rules(&old, &ir(&model("@rollout(audit) rule r: a > 9"))).is_empty());
}

const ENDPOINT: &str = "model B { n: int }\n\
service S at \"/s\" {\n\
  post \"/\" make(body: B) -> 204 {\n\
    require capped: body.n < LIMIT\n\
    require body.n > 0\n\
  }\n\
}\n";

#[test]
fn a_changed_require_is_found_by_name_and_by_position() {
    let old = ir(&ENDPOINT.replace("LIMIT", "10"));
    let new = ir(&ENDPOINT.replace("LIMIT", "5").replace("> 0", "> 1"));
    let changed = changed_rules(&old, &new);
    assert_eq!(changed.len(), 2, "{changed:?}");
    assert!(changed.iter().all(|c| c.kind == ChangeKind::RequireChanged));
    let names: Vec<String> = changed.iter().map(|c| c.name()).collect();
    assert!(names.contains(&"require `capped`".to_string()), "{names:?}");
    for c in &changed {
        let RuleSite::Require { old, new } = c.site else {
            panic!()
        };
        assert_eq!((old.3, new.3), (old.3, old.3), "same line in both versions");
    }
}
