//! The golden file `tests/wire/runtime-config/cases.json` against the reference check.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;
use veld_codegen::runtime_config::{
    Problem, RULE_OUTPUT_VALUES, RuntimeModel, Setting, SettingKind, Values, error_text,
    normalize_url, runtime_type_name,
};

fn golden() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/wire/runtime-config/cases.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

fn texts(v: &Value) -> BTreeMap<String, String> {
    v.as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
        .collect()
}

/// The model of the golden, built with the constructors the generators' core uses, so that the
/// golden also pins the settings, their order, kinds, defaults and requiredness.
fn model(spec: &Value) -> RuntimeModel {
    let mut settings = Vec::new();
    for s in spec["settings"].as_array().unwrap() {
        let path = s["path"].as_str().unwrap();
        let built = match s["kind"].as_str().unwrap() {
            "url" => {
                let name = path.strip_prefix("services.").unwrap();
                Setting::service_url(name.strip_suffix(".url").unwrap(), Vec::new())
            }
            "list" => {
                let values: Vec<&str> = s["values"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect();
                assert_eq!(values, RULE_OUTPUT_VALUES);
                Setting::rules_output()
            }
            other => panic!("unknown kind {other}"),
        };
        assert_eq!(built.path, path);
        assert_eq!(built.required, s["required"].as_bool().unwrap(), "{path}");
        assert_eq!(built.default.as_deref(), s["default"].as_str(), "{path}");
        assert_eq!(
            matches!(built.kind, SettingKind::Url),
            s["kind"] == "url",
            "{path}"
        );
        settings.push(built);
    }
    let m = RuntimeModel::from_settings(spec["target"].as_str().unwrap(), settings);
    let listed: Vec<&str> = spec["settings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["path"].as_str().unwrap())
        .collect();
    let ordered: Vec<&str> = m.settings().iter().map(|s| s.path.as_str()).collect();
    assert_eq!(
        listed, ordered,
        "the golden lists the settings in path order"
    );
    assert_eq!(m.type_name(), spec["type"].as_str().unwrap());
    m
}

fn run(m: &RuntimeModel, case: &Value) -> Result<Values, Vec<Problem>> {
    match case["from"].as_str().unwrap() {
        "value" => m.check(&case["input"]),
        "map" => m.check_map(&texts(&case["input"])),
        "env" => m.check_env(&texts(&case["names"]), &texts(&case["env"])),
        other => panic!("unknown source {other}"),
    }
}

#[test]
fn every_case_of_the_golden_file_gives_the_expected_values_or_problems() {
    let g = golden();
    assert_eq!(g["version"], 1);
    let models: BTreeMap<String, RuntimeModel> = g["models"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(id, spec)| (id.clone(), model(spec)))
        .collect();
    let cases = g["cases"].as_array().unwrap();
    assert!(cases.len() >= 40);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let m = &models[case["model"].as_str().unwrap()];
        let got = run(m, case);
        let expect = &case["expect"];
        match got {
            Ok(values) => {
                assert_eq!(texts(&expect["values"]), values, "{name}: expected values");
            }
            Err(problems) => {
                let want: Vec<(String, String)> = expect["problems"]
                    .as_array()
                    .unwrap_or_else(|| panic!("{name}: expected values, got {problems:?}"))
                    .iter()
                    .map(|p| {
                        (
                            p["path"].as_str().unwrap().to_string(),
                            p["message"].as_str().unwrap().to_string(),
                        )
                    })
                    .collect();
                let have: Vec<(String, String)> = problems
                    .iter()
                    .map(|p| (p.path.clone(), p.message.clone()))
                    .collect();
                assert_eq!(want, have, "{name}: problems");
                assert_eq!(
                    expect["error"].as_str().unwrap(),
                    error_text(&problems),
                    "{name}: error text"
                );
            }
        }
    }
}

#[test]
fn the_golden_type_names_and_urls_agree_with_the_reference() {
    let g = golden();
    for t in g["type_names"].as_array().unwrap() {
        let target = t["target"].as_str().unwrap();
        assert_eq!(
            runtime_type_name(target),
            t["type"].as_str().unwrap(),
            "{target}"
        );
    }
    for u in g["urls"].as_array().unwrap() {
        let input = u["input"].as_str().unwrap();
        assert_eq!(
            normalize_url(input).as_deref(),
            u["normalized"].as_str(),
            "{input:?}"
        );
    }
}
