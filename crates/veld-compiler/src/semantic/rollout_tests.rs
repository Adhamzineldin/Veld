//! `@rollout(audit)` (`spec/rules.md` section 17.1): where it may stand, what it lowers to and
//! what it leaves alone.

use crate::testing::{codes, ir};
use veld_ir::{DeclKind, Feature, Ir, Rollout};

const MODELS: &str = "model Room { id: uuid, capacity: int }\n\
model Booking { id: uuid, guests: int, status: Status }\n\
enum Status { pending, paid\n flow pending -> paid\n final paid }\n\
error RoomFull 409 { capacity: int }\n";

fn model(rules: &str) -> String {
    format!("model B {{\n guests: int\n{rules}\n}}\n")
}

fn service(models: &str, block: &str) -> String {
    format!(
        "{MODELS}{models}\nservice S at \"/s\" {{\n  post \"/\" book(body: B) -> 201 B | RoomFull {{\n{block}\n  }}\n}}\n"
    )
}

fn compile(rules: &str) -> Ir {
    ir(&format!(
        "{}service S at \"/s\" {{ post \"/\" book(body: B) -> 201 B }}\n",
        model(rules)
    ))
}

fn with_block(block: &str) -> Ir {
    ir(&service(&model(""), block))
}

fn rule<'a>(ir: &'a Ir, name: &str) -> &'a veld_ir::Rule {
    let found = ir
        .decls
        .iter()
        .flat_map(|d| d.rules())
        .find(|r| r.name == name);
    match found {
        Some(r) => r,
        None => panic!("no rule {name}"),
    }
}

const AUDITED: &str = " @rollout(audit)\n rule small: guests <= 6\n rule positive: guests > 0";
const ENFORCED: &str = " rule small: guests <= 6\n rule positive: guests > 0";

// ---- the IR ----------------------------------------------------------------------------------

#[test]
fn an_audited_invariant_carries_rollout_and_an_enforced_one_does_not() {
    let ir = compile(AUDITED);
    assert_eq!(rule(&ir, "small").rollout, Some(Rollout::Audit));
    assert_eq!(rule(&ir, "positive").rollout, None);
    let json = serde_json::to_value(rule(&ir, "small")).unwrap_or_default();
    assert_eq!(json["rollout"], "audit");
    let json = serde_json::to_value(rule(&ir, "positive")).unwrap_or_default();
    assert!(json.get("rollout").is_none(), "{json}");
}

#[test]
fn a_contract_without_the_annotation_has_no_rollout_key_anywhere() {
    let ir = compile(ENFORCED);
    let text = serde_json::to_string(&ir).unwrap_or_default();
    assert!(!text.contains("rollout"), "{text}");
}

#[test]
fn the_rule_id_does_not_change_with_the_annotation() {
    let (a, b) = (compile(AUDITED), compile(ENFORCED));
    assert_eq!(rule(&a, "small").id, rule(&b, "small").id);
    assert_eq!(rule(&a, "positive").id, rule(&b, "positive").id);
}

#[test]
fn the_annotation_may_follow_a_doc_comment_and_a_deprecation() {
    let ir = compile(
        " /// Small parties.\n @deprecated(\"use positive\")\n @rollout(audit)\n rule small: guests <= 6",
    );
    let r = rule(&ir, "small");
    assert_eq!(r.rollout, Some(Rollout::Audit));
    assert_eq!(r.docs.as_deref(), Some("Small parties."));
    assert_eq!(r.deprecated.as_deref(), Some("use positive"));
}

#[test]
fn an_inherited_audited_rule_stays_audited_in_the_child() {
    let src = "model P { n: int\n @rollout(audit)\n rule low: n > 0 }\n\
        model C extends P { m: int }\n\
        service S at \"/s\" { get \"/\" f() -> C }";
    let ir = ir(src);
    let child = ir.decls.iter().find(|d| d.name == "C").map(|d| d.rules());
    let low = child.and_then(|r| r.iter().find(|r| r.name == "low"));
    assert_eq!(low.map(|r| r.rollout), Some(Some(Rollout::Audit)));
}

#[test]
fn error_and_event_invariants_can_be_audited() {
    let src = "error Bad 400 { n: int\n @rollout(audit)\n rule big: n > 10 }\n\
        event Paid { amount: int\n @rollout(audit)\n rule positive: amount > 0 }\n\
        model M { a: int }\n\
        service S at \"/s\" { get \"/\" f() -> M | Bad }";
    let ir = ir(src);
    assert_eq!(rule(&ir, "big").rollout, Some(Rollout::Audit));
    assert_eq!(rule(&ir, "positive").rollout, Some(Rollout::Audit));
}

#[test]
fn require_and_ensure_carry_the_annotation() {
    let ir = with_block(
        "    @rollout(audit)\n    require body.guests > 0 else RoomFull { capacity: 1 }\n    require body.guests < 9\n    @rollout(audit)\n    ensure named: result.guests > 0\n    ensure result.guests < 9",
    );
    let r = &ir.services[0].endpoints[0].rules;
    let kinds: Vec<_> = r.requires.iter().map(|x| x.rollout).collect();
    assert_eq!(kinds, [Some(Rollout::Audit), None]);
    let kinds: Vec<_> = r.ensures.iter().map(|x| x.rollout).collect();
    assert_eq!(kinds, [Some(Rollout::Audit), None]);
    let json = serde_json::to_value(r).unwrap_or_default();
    assert_eq!(json["requires"][0]["rollout"], "audit");
    assert!(json["requires"][1].get("rollout").is_none(), "{json}");
}

#[test]
fn a_statement_id_does_not_change_with_the_annotation() {
    let plain = with_block("    require body.guests > 0\n    ensure result.guests > 0");
    let audited = with_block(
        "    @rollout(audit)\n    require body.guests > 0\n    @rollout(audit)\n    ensure result.guests > 0",
    );
    let (p, a) = (
        &plain.services[0].endpoints[0].rules,
        &audited.services[0].endpoints[0].rules,
    );
    assert_eq!(p.requires[0].id, a.requires[0].id);
    assert_eq!(p.ensures[0].id, a.ensures[0].id);
}

// ---- the feature ------------------------------------------------------------------------------

fn features(ir: &Ir) -> Vec<Feature> {
    ir.features_used(&[veld_ir::ServiceId(0)])
        .into_iter()
        .collect()
}

#[test]
fn a_service_that_reaches_an_audited_rule_uses_rules_rollout() {
    assert!(features(&compile(AUDITED)).contains(&Feature::RulesRollout));
    assert!(!features(&compile(ENFORCED)).contains(&Feature::RulesRollout));
    let ir = with_block("    @rollout(audit)\n    require body.guests > 0");
    assert!(features(&ir).contains(&Feature::RulesRollout));
    let ir = with_block("    @rollout(audit)\n    ensure result.guests > 0");
    assert!(features(&ir).contains(&Feature::RulesRollout));
    let ir = with_block("    require body.guests > 0\n    ensure result.guests > 0");
    assert!(!features(&ir).contains(&Feature::RulesRollout));
}

#[test]
fn an_audited_rule_of_a_declaration_nobody_reaches_is_not_used() {
    let src = "model Lonely { n: int\n @rollout(audit)\n rule r: n > 0 }\n\
        model M { a: int }\nservice S at \"/s\" { get \"/\" f() -> M }";
    assert!(!features(&ir(src)).contains(&Feature::RulesRollout));
}

// ---- the digest -------------------------------------------------------------------------------

fn digest(ir: &Ir) -> Option<String> {
    ir.services[0].rules_digest.clone()
}

#[test]
fn the_digest_covers_only_enforced_invariants() {
    let audited = compile(AUDITED);
    let only_positive = compile(" rule positive: guests > 0");
    assert_eq!(digest(&audited), digest(&only_positive));
    assert_ne!(digest(&audited), digest(&compile(ENFORCED)));
    assert!(digest(&audited).is_some());
}

#[test]
fn adding_the_annotation_changes_the_digest_and_removing_it_changes_it_back() {
    let before = digest(&compile(ENFORCED));
    let during = digest(&compile(AUDITED));
    let after = digest(&compile(&AUDITED.replace(" @rollout(audit)\n", "")));
    assert_ne!(before, during);
    assert_eq!(before, after);
}

#[test]
fn a_service_whose_invariants_are_all_audited_has_no_digest() {
    let ir = compile(" @rollout(audit)\n rule small: guests <= 6");
    assert_eq!(digest(&ir), None);
}

#[test]
fn a_contract_with_no_audited_rule_keeps_the_digest_of_every_rule_id() {
    let ir = compile(ENFORCED);
    let mut ids: Vec<String> = ir
        .decls
        .iter()
        .flat_map(|d| d.rules().iter().map(|r| r.id.clone()))
        .collect();
    ids.sort();
    let json = format!(
        "[{}]",
        ids.iter()
            .map(|i| format!("\"{i}\""))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(
        digest(&ir).as_deref(),
        Some(crate::signature::hex16(json.as_bytes()).as_str())
    );
}

#[test]
fn endpoint_rules_never_change_the_digest() {
    let plain = with_block("    require body.guests > 0");
    let audited = with_block("    @rollout(audit)\n    require body.guests > 0");
    assert_eq!(digest(&plain), digest(&audited));
}

fn event_digest(src: &str) -> Option<String> {
    let ir = ir(src);
    let event = ir.decls.iter().find(|d| d.name == "Paid");
    match event.map(|d| &d.kind) {
        Some(DeclKind::Event(e)) => e.rules_digest.clone(),
        Some(_) | None => None,
    }
}

const EVENT_HEAD: &str = "channel Pays { carries Paid\n key id }\n\
    service S at \"/s\" { publishes Pays\n get \"/\" f() -> 204 }\n";

#[test]
fn the_digest_of_an_event_leaves_out_audited_invariants_too() {
    let event =
        |rules: &str| format!("event Paid {{ id: uuid\n amount: int\n{rules}\n}}\n{EVENT_HEAD}");
    let audited = event(" @rollout(audit)\n rule positive: amount > 0\n rule big: amount < 1000");
    let only_big = event(" rule big: amount < 1000");
    let both = event(" rule positive: amount > 0\n rule big: amount < 1000");
    assert_eq!(event_digest(&audited), event_digest(&only_big));
    assert_ne!(event_digest(&audited), event_digest(&both));
    let all_audited = event(" @rollout(audit)\n rule positive: amount > 0");
    assert_eq!(event_digest(&all_audited), None);
}

// ---- E0602 ------------------------------------------------------------------------------------

fn e0602(src: &str) -> bool {
    codes(src).contains(&"E0602")
}

#[test]
fn rollout_on_a_field_a_declaration_or_an_endpoint_is_e0602() {
    assert!(e0602("model M { @rollout(audit)\n n: int }"));
    assert!(e0602("@rollout(audit)\nmodel M { n: int }"));
    assert!(e0602("@rollout(audit)\nenum E { a }"));
    assert!(e0602(
        "model M { n: int }\nservice S at \"/s\" {\n @rollout(audit)\n get \"/\" f() -> M\n}"
    ));
    assert!(e0602(
        "model M { n: int }\n@rollout(audit)\nservice S at \"/s\" { get \"/\" f() -> M }"
    ));
    assert!(e0602(
        "model M { n: int }\nservice S at \"/s\" { get \"/\" f(@rollout(audit) @query n: int) -> M }"
    ));
}

#[test]
fn rollout_on_a_fact_an_allow_or_a_transition_is_e0602() {
    let src = |stmt: &str| {
        format!(
            "model User {{ id: uuid }}\nprincipal User\npolicy P(u: User) {{ permit when u.id == u.id }}\n{MODELS}\
             @auth(bearer)\nservice S at \"/s\" {{\n  post \"/{{id}}\" pay(id: uuid) -> Booking {{\n    fact booking: Booking = id\n{stmt}\n  }}\n}}\n"
        )
    };
    for stmt in [
        "    @rollout(audit)\n    fact other: Room = id",
        "    @rollout(audit)\n    allow P(user)",
        "    @rollout(audit)\n    transition booking.status: pending -> paid",
    ] {
        assert!(e0602(&src(stmt)), "{stmt}");
    }
    assert!(!e0602(&src("    allow P(user)")));
}

#[test]
fn rollout_on_a_policy_a_principal_or_an_example_is_e0602() {
    let head = "model User { id: uuid }\n";
    assert!(e0602(&format!(
        "{head}@rollout(audit)\npolicy P(u: User) {{ permit when u.id == u.id }}"
    )));
    assert!(e0602(&format!("{head}@rollout(audit)\nprincipal User")));
    assert!(e0602(&format!(
        "{head}service S at \"/s\" {{ get \"/\" f() -> User }}\n@rollout(audit)\nexample \"x\" {{ f() -> ok }}"
    )));
}

#[test]
fn another_argument_a_missing_argument_or_a_repeat_is_e0602() {
    for bad in [
        "@rollout(shadow)",
        "@rollout(enforce)",
        "@rollout(\"audit\")",
        "@rollout(audit, audit)",
        "@rollout(mode: audit)",
        "@rollout()",
        "@rollout",
        "@rollout(audit) @rollout(audit)",
    ] {
        let src = format!("model M {{ n: int\n {bad}\n rule r: n > 0 }}");
        assert_eq!(codes(&src), vec!["E0602"], "{bad}");
        let block = format!("    {bad}\n    require body.guests > 0");
        let src = service(&model(""), &block);
        assert_eq!(codes(&src), vec!["E0602"], "{bad}");
    }
}

#[test]
fn no_other_annotation_is_legal_on_a_require_or_an_ensure() {
    for ann in ["@deprecated(\"x\")", "@wire(\"x\")", "@min(1)"] {
        for stmt in ["require body.guests > 0", "ensure result.guests > 0"] {
            let src = service(&model(""), &format!("    {ann}\n    {stmt}"));
            assert_eq!(codes(&src), vec!["E0602"], "{ann} {stmt}");
        }
    }
    let src = service(&model(""), "    @nope\n    require body.guests > 0");
    assert_eq!(codes(&src), vec!["E0601"]);
}

#[test]
fn rollout_is_not_suggested_for_an_unknown_annotation_on_a_field() {
    let out = crate::testing::one("model M { n: int @nope }");
    let help = out
        .diagnostics
        .iter()
        .filter_map(|d| d.help.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        help.contains("@min") && !help.contains("@rollout"),
        "{help}"
    );
    let out = crate::testing::one("model M { n: int\n @nope\n rule r: n > 0 }");
    let help = out
        .diagnostics
        .iter()
        .filter_map(|d| d.help.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(help.contains("@rollout"), "{help}");
}

#[test]
fn an_audited_invariant_of_an_event_alone_makes_no_service_use_rules_rollout() {
    // No endpoint reaches an event: the service uses `events`, and a generator that supports
    // events has to look at the rules of its events itself.
    let src = format!(
        "event Paid {{ id: uuid\n amount: int\n @rollout(audit)\n rule positive: amount > 0\n}}\n{EVENT_HEAD}"
    );
    let ir = ir(&src);
    let used = features(&ir);
    assert!(used.contains(&Feature::Events), "{used:?}");
    assert!(!used.contains(&Feature::RulesRollout), "{used:?}");
}
