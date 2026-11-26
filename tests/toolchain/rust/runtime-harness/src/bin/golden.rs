//! The goldens of the typed runtime config (`tests/wire/runtime-config/cases.json`) through the
//! generated `runtime_config` modules of four targets, one for each model of the file:
//!
//! - `server`: the crate `bookings` (rules, and the services `iam` and `payments`),
//! - `client`: the crate `web` (a client of `bookings` and `iam`),
//! - `ordering`: the crate `card-service` (the services `a-b` and `a`),
//! - `rules_only`: the crate `orders` (rules, no service).
//!
//! `from: "value"` goes through `from_value`, `from: "map"` through `from_map` and `from: "env"`
//! through `from_env_with`. The values, the problems in order and the text of the error must be
//! those of the file. A nested value that a plain `serde` deserialization accepts must also give the
//! same result through the derived `Deserialize` and `check` (`serde` reports the wrong types and
//! the unknown keys with its own text, so a value it refuses is only checked through `from_value`).
//!
//! Usage: `golden <cases.json>`. Prints `runtime goldens ok: <cases> cases, <urls> urls`.

use std::collections::BTreeMap;

use serde_json::Value;

/// What a check gives: the values, or the problems (path, message) and the text of the error.
#[derive(Debug, PartialEq)]
enum Outcome {
    Values(BTreeMap<String, String>),
    Problems(Vec<(String, String)>, String),
}

fn expected(case: &Value) -> Outcome {
    let expect = &case["expect"];
    if let Some(values) = expect.get("values") {
        let map = values.as_object().expect("values").iter().map(|(k, v)| (k.clone(), v.as_str().expect("text").to_string()));
        return Outcome::Values(map.collect());
    }
    let problems = expect["problems"]
        .as_array()
        .expect("problems")
        .iter()
        .map(|p| (p["path"].as_str().expect("path").to_string(), p["message"].as_str().expect("message").to_string()))
        .collect();
    Outcome::Problems(problems, expect["error"].as_str().expect("error").to_string())
}

/// The names of `case` as pairs, and the environment as a map.
fn env_of(case: &Value) -> (Vec<(String, String)>, BTreeMap<String, String>) {
    let names = case["names"].as_object().expect("names").iter().map(|(p, n)| (p.clone(), n.as_str().expect("name").to_string()));
    let env = case["env"].as_object().expect("env").iter().filter_map(|(n, v)| v.as_str().map(|v| (n.clone(), v.to_string())));
    (names.collect(), env.collect())
}

/// Runs one case through the generated type `$rt` of the crate `$krate`.
macro_rules! run_model {
    ($krate:ident, $rt:ident, $case:expr) => {{
        use $krate::runtime_config::{$rt, ConfigError};
        fn outcome(r: Result<$rt, ConfigError>) -> Outcome {
            match r {
                Ok(rt) => Outcome::Values(rt.values()),
                Err(e) => Outcome::Problems(
                    e.problems().iter().map(|p| (p.path.clone(), p.message.clone())).collect(),
                    e.to_string(),
                ),
            }
        }
        let case: &Value = $case;
        let input = &case["input"];
        match case["from"].as_str().expect("from") {
            "value" => {
                // A value that `serde` accepts gives the same result through `check`.
                if let Ok(typed) = serde_json::from_value::<$rt>(input.clone()) {
                    assert_eq!(outcome(typed.check()), expected(case), "{} (serde and check)", case["name"]);
                }
                outcome($rt::from_value(input))
            }
            "map" => {
                let map = input.as_object().expect("map").iter().map(|(k, v)| (k.clone(), v.as_str().expect("text").to_string()));
                outcome($rt::from_map(&map.collect()))
            }
            "env" => {
                let (names, env) = env_of(case);
                let names: Vec<(&str, &str)> = names.iter().map(|(p, n)| (p.as_str(), n.as_str())).collect();
                outcome($rt::from_env_with(&names, &|n| env.get(n).cloned()))
            }
            other => panic!("unknown source {other}"),
        }
    }};
}

fn run_case(case: &Value) -> Outcome {
    match case["model"].as_str().expect("model") {
        "server" => run_model!(bookings, BookingsRuntime, case),
        "client" => run_model!(web, WebRuntime, case),
        "ordering" => run_model!(card_service, CardServiceRuntime, case),
        "rules_only" => run_model!(orders, OrdersRuntime, case),
        other => panic!("unknown model {other}"),
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("the path of cases.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let file: Value = serde_json::from_str(&text).expect("json");
    let cases = file["cases"].as_array().expect("cases");
    let mut failures = Vec::new();
    for case in cases {
        let got = run_case(case);
        let want = expected(case);
        if got != want {
            failures.push(format!("{}: got {got:?}, wanted {want:?}", case["name"]));
        }
    }
    assert!(failures.is_empty(), "{} cases differ:\n{}", failures.len(), failures.join("\n"));
    let urls = file["urls"].as_array().expect("urls");
    for u in urls {
        let got = bookings::veld::config::normalize_url(u["input"].as_str().expect("input"));
        let want = u["normalized"].as_str().map(str::to_string);
        assert_eq!(got, want, "url {}", u["input"]);
    }
    println!("runtime goldens ok: {} cases, {} urls", cases.len(), urls.len());
}
