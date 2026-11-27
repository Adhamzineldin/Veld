//! The fuzzer and `@rollout(audit)` (`spec/rules.md` section 17.1): it works out expected
//! outcomes with the reference evaluator, so an audited rule must never make it expect a
//! rejection, and it must not spend its repairs or its boundary requests on an audited rule.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};

use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_rules::{Outcome, Pipeline};
use veld_testing::generate;

fn compile_text(text: &str) -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "t".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
                text: text.to_owned(),
            }],
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    match out.ir {
        Some(ir) => ir,
        None => panic!("does not compile:\n{}", out.render()),
    }
}

/// Every rule of the contract is audited, so no request can be rejected.
const ALL_AUDITED: &str = r#"
model Booking {
  guests: int @min(0) @max(40)
  @rollout(audit)
  rule smallParty: guests <= 6
  @rollout(audit)
  rule positive: guests > 3
}
service S at "/s" {
  post "/" book(body: Booking) -> 201 Booking {
    @rollout(audit)
    require body.guests > 5
    @rollout(audit)
    ensure result.guests == body.guests
  }
}
"#;

/// The same rules, enforced.
const ALL_ENFORCED: &str = r#"
model Booking {
  guests: int @min(0) @max(40)
  rule smallParty: guests <= 6
  rule positive: guests > 3
}
service S at "/s" {
  post "/" book(body: Booking) -> 201 Booking {
    require body.guests > 5
    ensure result.guests == body.guests
  }
}
"#;

fn labels(ir: &Ir, seed: u64, count: usize) -> BTreeSet<String> {
    let service = &ir.services[0];
    let endpoint = &service.endpoints[0];
    let pipeline = Pipeline::new(ir, service, &endpoint.name).unwrap();
    let generated = generate(ir, service, endpoint, seed, count).unwrap();
    assert!(!generated.requests.is_empty());
    generated
        .requests
        .iter()
        .map(|req| match pipeline.run(req).unwrap() {
            Outcome::Passes => "ok".to_owned(),
            Outcome::Error(a) => a.name,
        })
        .collect()
}

#[test]
fn an_audited_rule_never_makes_the_fuzzer_expect_a_rejection() {
    let ir = compile_text(ALL_AUDITED);
    for seed in [1, 7, 42] {
        let seen = labels(&ir, seed, 300);
        assert_eq!(seen, BTreeSet::from(["ok".to_owned()]), "seed {seed}");
    }
}

#[test]
fn the_same_rules_enforced_do_make_it_expect_rejections() {
    let ir = compile_text(ALL_ENFORCED);
    let seen = labels(&ir, 1, 300);
    assert!(
        seen.contains("ok") && seen.contains("ValidationError"),
        "{seen:?}"
    );
}

#[test]
fn an_audited_rule_has_no_boundary_requests_and_the_generation_is_still_repeatable() {
    let audited = compile_text(ALL_AUDITED);
    let enforced = compile_text(ALL_ENFORCED);
    let boundaries = |ir: &Ir| {
        let s = &ir.services[0];
        generate(ir, s, &s.endpoints[0], 3, 200).unwrap().boundaries
    };
    assert_eq!(boundaries(&audited), 0);
    assert!(boundaries(&enforced) > 0);
    let s = &audited.services[0];
    let a = generate(&audited, s, &s.endpoints[0], 9, 100).unwrap();
    let b = generate(&audited, s, &s.endpoints[0], 9, 100).unwrap();
    assert_eq!(a.requests, b.requests);
}

/// An enforced rule next to an audited one: the requests that fail are the enforced rule's, the
/// audited one is never the reason, and the repair still finds requests that pass.
#[test]
fn an_enforced_rule_next_to_an_audited_one_still_rejects_and_passes() {
    let src = r#"
model Booking {
  guests: int @min(0) @max(40)
  @rollout(audit)
  rule smallParty: guests <= 6
  rule atLeastTwo: guests >= 2
}
service S at "/s" {
  post "/" book(body: Booking) -> 201 Booking {
    require body.guests > 0
  }
}
"#;
    let ir = compile_text(src);
    let service = &ir.services[0];
    let pipeline = Pipeline::new(&ir, service, "book").unwrap();
    let generated = generate(&ir, service, &service.endpoints[0], 5, 300).unwrap();
    let (mut passes, mut rejects) = (0, 0);
    for req in &generated.requests {
        match pipeline.run(req).unwrap() {
            Outcome::Passes => {
                passes += 1;
                // Passing requests include ones that break the audited `smallParty`.
            }
            Outcome::Error(a) => {
                rejects += 1;
                assert!(
                    a.violations.iter().all(|v| v.rule_name != "smallParty"),
                    "{:?}",
                    a.violations
                );
            }
        }
    }
    assert!(
        passes > 0 && rejects > 0,
        "{passes} passes, {rejects} rejects"
    );
}
