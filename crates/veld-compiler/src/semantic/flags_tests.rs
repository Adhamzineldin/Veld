//! Flags (`spec/platform.md` section 11): the `flag` key in the IR, the rules digest, the
//! feature, and `E0938`.

use crate::testing::{codes, ir};
use veld_ir::{DeclKind, Feature};

const STAY: &str = "model Stay {\n  guests: int\n  rule positive: guests > 0\n  rule small: guests <= 6 when flag(\"smallGroups\")\n}\n";

fn service(block: &str) -> String {
    format!(
        "{STAY}enum Phase {{\n  a, b\n  flow a -> b when flag(\"phases\")\n  final b\n}}\n\
         model Room {{ id: uuid, open: bool, phase: Phase }}\n\
         policy P(n: int) {{\n  permit when n > 0\n  permit when n > 5 when flag(\"wide\")\n  forbid when n > 9 when flag(\"lock\")\n}}\n\
         service S at \"/s\" {{\n  post \"/{{id}}\" go(id: uuid, body: Stay) -> Room {{\n    fact room: Room = id\n    allow P(1)\n    require room.open when flag(\"openOnly\")\n    ensure result.open when flag(\"openOut\")\n{block}\n  }}\n}}\n"
    )
}

#[test]
fn every_flagged_statement_carries_its_flag_in_the_ir() {
    let ir = ir(&service(""));
    let stay = ir.decls.iter().find(|d| d.name == "Stay").unwrap();
    let flags: Vec<(&str, Option<&str>)> = stay
        .rules()
        .iter()
        .map(|r| (r.name.as_str(), r.flag.as_deref()))
        .collect();
    assert_eq!(flags, [("positive", None), ("small", Some("smallGroups"))]);
    let flows: Vec<Option<&str>> = ir
        .decls
        .iter()
        .filter_map(|d| match &d.kind {
            DeclKind::Enum(e) => Some(e.flows.iter().map(|f| f.flag.as_deref()).collect()),
            _ => None,
        })
        .next()
        .unwrap();
    assert_eq!(flows, [Some("phases")]);
    let ep = &ir.services[0].endpoints[0];
    assert_eq!(ep.rules.requires[0].flag.as_deref(), Some("openOnly"));
    assert_eq!(ep.rules.ensures[0].flag.as_deref(), Some("openOut"));
    let clauses: Vec<Option<&str>> = ir.policies[0]
        .clauses
        .iter()
        .map(|c| c.flag.as_deref())
        .collect();
    assert_eq!(clauses, [None, Some("wide"), Some("lock")]);
}

#[test]
fn the_flag_is_not_part_of_a_rule_id() {
    let flagged = ir(&service(""));
    let plain = ir(&service("").replace(" when flag(\"smallGroups\")", ""));
    let id = |ir: &veld_ir::Ir| {
        ir.decls
            .iter()
            .find(|d| d.name == "Stay")
            .and_then(|d| d.rules().iter().find(|r| r.name == "small"))
            .map(|r| r.id.clone())
            .unwrap()
    };
    assert_eq!(id(&flagged), id(&plain));
}

#[test]
fn a_flagged_invariant_leaves_the_rules_digest_and_a_contract_without_flags_keeps_it() {
    let src = "model Stay { guests: int\n rule positive: guests > 0 }\nservice S at \"/s\" { post \"/\" go(body: Stay) -> 204 }";
    let with_extra = "model Stay { guests: int\n rule positive: guests > 0\n rule small: guests <= 6 when flag(\"x\") }\nservice S at \"/s\" { post \"/\" go(body: Stay) -> 204 }";
    let digest = |s: &str| ir(s).services[0].rules_digest.clone();
    assert!(digest(src).is_some());
    assert_eq!(digest(src), digest(with_extra));
    // A model whose only invariant is flagged has no digest at all.
    let only = "model Stay { guests: int\n rule small: guests <= 6 when flag(\"x\") }\nservice S at \"/s\" { post \"/\" go(body: Stay) -> 204 }";
    assert_eq!(digest(only), None);
}

#[test]
fn the_feature_is_used_exactly_when_a_flag_is() {
    let flagged = ir(&service(""));
    let used = flagged.features_used(&[flagged.services[0].id]);
    assert!(used.contains(&Feature::RulesFlags), "{used:?}");
    let plain = ir(STAY.replace(" when flag(\"smallGroups\")", "").as_str());
    assert!(!plain.features_used(&[]).contains(&Feature::RulesFlags));
    let json = serde_json::to_string(&plain).unwrap();
    assert!(!json.contains("\"flag\""), "{json}");
}

#[test]
fn a_given_flag_must_name_a_flag_that_some_statement_uses() {
    let ok = format!(
        "{}\nexample \"e\" {{\n  given flag \"openOnly\"\n  given room = {{ open: true }}\n  go(id: room.id, body: {{ guests: 2 }}) -> ok\n}}\n",
        service("")
    );
    assert!(codes(&ok).is_empty(), "{:?}", codes(&ok));
    let bad = ok.replace("given flag \"openOnly\"", "given flag \"opneOnly\"");
    assert_eq!(codes(&bad), ["E0938"]);
    let off = ok.replace("given flag \"openOnly\"", "given flag \"nothing\" off");
    assert_eq!(codes(&off), ["E0938"]);
}

#[test]
fn an_example_lists_the_flags_that_are_on_sorted() {
    let src = format!(
        "{}\nexample \"e\" {{\n  given flag \"openOut\"\n  given flag \"openOnly\"\n  given flag \"phases\"\n  given flag \"phases\" off\n  given room = {{ open: true }}\n  go(id: room.id, body: {{ guests: 2 }}) -> ok\n}}\n",
        service("")
    );
    let ir = ir(&src);
    assert_eq!(ir.examples[0].flags, ["openOnly", "openOut"]);
}

#[test]
fn flags_are_listed_with_what_they_gate() {
    let usage = ir(&service("")).flag_usage();
    let names: Vec<&str> = usage.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [
            "lock",
            "openOnly",
            "openOut",
            "phases",
            "smallGroups",
            "wide"
        ]
    );
    assert_eq!(usage["openOnly"].requires, 1);
    assert_eq!(usage["openOut"].ensures, 1);
    assert_eq!(usage["phases"].flows, 1);
    assert_eq!(usage["smallGroups"].invariants, 1);
    assert_eq!(usage["wide"].permits, 1);
    assert_eq!(usage["lock"].forbids, 1);
}
