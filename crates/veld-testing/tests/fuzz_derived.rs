//! The fuzzer and derived fields (`spec/rules.md` section 17.2): nobody supplies a derived
//! value, so no generated request carries one (not in a body, not in a fact, not in a nested
//! value), an `example` block printed for a request never names one, and the expected outcome is
//! what the reference pipeline computes from the fields a caller does send.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value as Json;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};
use veld_rules::{Outcome, Pipeline, fill_example, run_example};
use veld_testing::{generate, render_example};

const SOURCE: &str = r#"
model Line {
  price: int @min(0) @max(100)
  qty: int @min(0) @max(10)
  total: int = price * qty
}

model Cart {
  items: Line[] @length(max: 3)
  coupon: int = 0
  subtotal: int = sum(items, i => i.total)
  payable: int = subtotal - coupon
  rule payableFits: payable >= 0
}

model Wallet {
  id: uuid
  balance: int @min(0) @max(1000)
  held: int @min(0) @max(1000)
  available: int = balance - held
}

service Shop at "/shop" {
  post "/pay/{id}" pay(id: uuid, body: Cart) -> 200 Cart {
    fact wallet: Wallet = id
    require body.payable <= wallet.available
  }
}
"#;

const DERIVED: [&str; 4] = ["total", "subtotal", "payable", "available"];

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

fn names_a_derived_field(v: &Json) -> bool {
    match v {
        Json::Object(map) => map
            .iter()
            .any(|(k, v)| DERIVED.contains(&k.as_str()) || names_a_derived_field(v)),
        Json::Array(items) => items.iter().any(names_a_derived_field),
        Json::Null | Json::Bool(_) | Json::Number(_) | Json::String(_) => false,
    }
}

#[test]
fn no_generated_request_carries_a_derived_value() {
    let ir = compile_text(SOURCE);
    let service = &ir.services[0];
    for seed in [1, 7, 42] {
        let generated = generate(&ir, service, &service.endpoints[0], seed, 300).unwrap();
        assert!(!generated.requests.is_empty());
        for req in &generated.requests {
            let found = req.params.values().any(names_a_derived_field)
                || req.facts.values().flatten().any(names_a_derived_field);
            assert!(!found, "seed {seed}: {req:?}");
        }
    }
}

#[test]
fn the_expected_outcomes_come_from_the_computed_values() {
    let ir = compile_text(SOURCE);
    let service = &ir.services[0];
    let pipeline = Pipeline::new(&ir, service, "pay").unwrap();
    let generated = generate(&ir, service, &service.endpoints[0], 3, 400).unwrap();
    let seen: BTreeSet<String> = generated
        .requests
        .iter()
        .map(|req| match pipeline.run(req).unwrap() {
            Outcome::Passes => "ok".to_owned(),
            Outcome::Error(a) => a.name,
        })
        .collect();
    // The require reads two derived values and the invariant reads one: both ends are reached.
    assert!(seen.contains("ok"), "{seen:?}");
    assert!(
        seen.contains("ValidationError"),
        "{seen:?}: no request failed a rule that reads a derived field"
    );
}

#[test]
fn generation_is_repeatable_with_derived_fields() {
    let ir = compile_text(SOURCE);
    let service = &ir.services[0];
    let a = generate(&ir, service, &service.endpoints[0], 9, 100).unwrap();
    let b = generate(&ir, service, &service.endpoints[0], 9, 100).unwrap();
    assert_eq!(a.requests, b.requests);
}

#[test]
fn a_printed_example_block_compiles_fills_back_and_passes() {
    let ir = compile_text(SOURCE);
    let service = &ir.services[0];
    let endpoint = &service.endpoints[0];
    let pipeline = Pipeline::new(&ir, service, "pay").unwrap();
    let mut blocks = Vec::new();
    let mut requests = Vec::new();
    for (i, req) in generate(&ir, service, endpoint, 21, 40)
        .unwrap()
        .requests
        .into_iter()
        .enumerate()
    {
        let outcome = pipeline.run(&req).unwrap();
        let title = format!("fuzz {i}");
        let block = render_example(&ir, service, endpoint, &req, &outcome, &title).unwrap();
        assert!(
            DERIVED.iter().all(|d| !block.contains(&format!("{d}:"))),
            "{block}"
        );
        blocks.push(block);
        requests.push((title, req));
    }
    let with = compile_text(&format!("{SOURCE}\n{}\n", blocks.join("\n\n")));
    for (title, req) in requests {
        let ex = with.examples.iter().find(|e| e.name == title).unwrap();
        let filled = fill_example(&with, ex).unwrap_or_else(|e| panic!("{title}: {e}"));
        assert_eq!(filled.request, req, "{title}");
        let report = run_example(&with, ex).unwrap();
        assert!(report.passed(), "{title}: {report}");
    }
}
