//! Wire goldens (`tests/wire/*.json`) checked against `veld_codegen::canonical`.
//! The same files are checked from TypeScript by `tests/toolchain/ts/wire.ts`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use serde_json::Value;
use veld_codegen::canonical::canonical;

fn wire_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/wire")
}

#[test]
fn json_goldens_match_canonical_bytes() {
    let mut checked = 0;
    let mut entries: Vec<_> = std::fs::read_dir(wire_dir()).unwrap().map(|e| e.unwrap().path()).collect();
    entries.sort();
    for path in entries.iter().filter(|p| p.extension().is_some_and(|e| e == "json")) {
        let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for case in cases.iter().filter(|c| c["kind"] == "json") {
            let want = case["canonical"].as_str().unwrap();
            assert_eq!(canonical(&case["input"]), want, "{}: {}", path.display(), case["name"]);
            checked += 1;
        }
    }
    assert!(checked >= 10, "expected the JCS goldens, found {checked}");
}

#[test]
fn model_goldens_are_idempotent_under_canonical() {
    // Expected bytes of a model golden are already canonical JSON.
    let text = std::fs::read_to_string(wire_dir().join("models.json")).unwrap();
    let cases: Vec<Value> = serde_json::from_str(&text).unwrap();
    for case in cases {
        let want = case["canonical"].as_str().unwrap();
        let reparsed: Value = serde_json::from_str(want).unwrap();
        assert_eq!(canonical(&reparsed), want, "{}", case["name"]);
    }
}
