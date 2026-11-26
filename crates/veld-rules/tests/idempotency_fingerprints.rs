//! The shared goldens of the idempotency fingerprint (`spec/rules.md` section 11.10,
//! `tests/wire/idempotency/`): every case is bound the way the spec says, run through the reference
//! pipeline, and its fingerprint and the canonical JSON that was hashed are compared with the file.
//! The pairs that must be equal, and the pairs that must differ, are checked as well.
//!
//! The reference takes the bound parameters in wire form. It has no binder for the text of a path,
//! a query string or a header, so [`bind`] is the one the spec describes: it parses the text into
//! the typed value of the parameter and writes that value in its wire form.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::wildcard_enum_match_arm,
    clippy::field_reassign_with_default
)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{Map, Number, Value as Json};
use veld_codegen::canonical::canonical;
use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Binding, DeclKind, Endpoint, Ir, PackageKind, Param, Scalar, Type};
use veld_rules::{IdempotencyKey, OutputMode, Pipeline, Request, StoreState, fingerprint};

pub fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/wire/idempotency")
}

pub fn contract() -> Ir {
    let text = std::fs::read_to_string(dir().join("fingerprints.veld")).unwrap();
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "bookings".into(),
            kind: PackageKind::Service,
            files: vec![SourceFile {
                path: "t.veld".into(),
                text,
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

pub fn cases() -> Vec<Json> {
    let text = std::fs::read_to_string(dir().join("fingerprints.json")).unwrap();
    match serde_json::from_str::<Json>(&text).unwrap() {
        Json::Array(a) => a,
        other => panic!("the file is not an array: {other}"),
    }
}

pub fn endpoint<'a>(ir: &'a Ir, name: &str) -> (&'a veld_ir::Service, &'a Endpoint) {
    let service = ir
        .services
        .iter()
        .find(|s| s.name == "Fingerprints")
        .unwrap();
    let ep = service.endpoints.iter().find(|e| e.name == name).unwrap();
    (service, ep)
}

// --- the binder -----------------------------------------------------------------------------------

/// One scalar from text: the typed value in its wire form. An `int` and a `float` are numbers
/// (`007` is 7, `7.0` is 7), a `long` is the string of its value (`007` is `"7"`), a `bool` is a
/// boolean; every other scalar, and an enum, is the text as it was sent.
fn scalar_from_text(ir: &Ir, ty: &Type, text: &str) -> Json {
    match ty {
        Type::Scalar(Scalar::Int) => Json::from(text.parse::<i32>().unwrap()),
        Type::Scalar(Scalar::Long) => Json::String(text.parse::<i64>().unwrap().to_string()),
        Type::Scalar(Scalar::Float) => {
            Json::Number(Number::from_f64(text.parse::<f64>().unwrap()).unwrap())
        }
        Type::Scalar(Scalar::Bool) => Json::Bool(text.parse::<bool>().unwrap()),
        Type::Scalar(_) => Json::String(text.to_owned()),
        Type::Decl(id) => match &ir.decl(*id).kind {
            DeclKind::Enum(_) => Json::String(text.to_owned()),
            _ => panic!("a model is not text"),
        },
        Type::List(_) | Type::Map(_) | Type::Page(_) => panic!("not a scalar"),
    }
}

/// The value of a path, query or header parameter from the texts that were sent for it.
fn from_texts(ir: &Ir, ty: &Type, texts: &[String]) -> Json {
    match ty {
        Type::List(item) => Json::Array(
            texts
                .iter()
                .map(|t| scalar_from_text(ir, item, t))
                .collect(),
        ),
        _ => scalar_from_text(ir, ty, &texts[0]),
    }
}

fn texts_of(query: &Json, name: &str) -> Option<Vec<String>> {
    match query.get(name)? {
        Json::String(s) => Some(vec![s.clone()]),
        Json::Array(a) => Some(a.iter().map(|v| v.as_str().unwrap().to_owned()).collect()),
        other => panic!("a query value is a string or a list of strings: {other}"),
    }
}

fn path_texts(template: &str, path: &str) -> BTreeMap<String, String> {
    let (t, p): (Vec<_>, Vec<_>) = (template.split('/').collect(), path.split('/').collect());
    assert_eq!(t.len(), p.len(), "{path} does not fit {template}");
    t.iter()
        .zip(&p)
        .filter_map(|(t, p)| {
            let name = t.strip_prefix('{')?.strip_suffix('}')?;
            Some((name.to_owned(), (*p).to_owned()))
        })
        .collect()
}

fn query_object(ir: &Ir, p: &Param, query: &Json) -> Option<Json> {
    let Type::Decl(id) = &p.ty else {
        panic!("`query` is a model")
    };
    let mut out = Map::new();
    for f in ir.decl(*id).fields() {
        if let Some(texts) = texts_of(query, &f.wire) {
            out.insert(f.wire.clone(), from_texts(ir, &f.ty, &texts));
        }
    }
    Some(Json::Object(out))
}

fn header_text(headers: &Json, name: &str) -> Option<String> {
    headers.as_object()?.iter().find_map(|(k, v)| {
        k.eq_ignore_ascii_case(name)
            .then(|| v.as_str().unwrap().to_owned())
    })
}

fn body_of(request: &Json) -> Option<Json> {
    match (request.get("body"), request.get("body_text")) {
        (Some(b), None) => Some(b.clone()),
        (None, Some(t)) => Some(serde_json::from_str(t.as_str().unwrap()).unwrap()),
        (None, None) => None,
        (Some(_), Some(_)) => panic!("a case has a body or a body_text, not both"),
    }
}

/// The bound parameters of a request, by parameter name, in wire form. A parameter that was not
/// sent is not there.
pub fn bind(ir: &Ir, ep: &Endpoint, request: &Json) -> BTreeMap<String, Json> {
    let in_path = path_texts(&ep.path, request["path"].as_str().unwrap());
    let query = request
        .get("query")
        .cloned()
        .unwrap_or_else(|| Json::Object(Map::new()));
    let headers = request
        .get("headers")
        .cloned()
        .unwrap_or_else(|| Json::Object(Map::new()));
    let mut out = BTreeMap::new();
    for p in &ep.params {
        let found = match p.binding {
            Binding::Path => in_path
                .get(&p.name)
                .map(|t| from_texts(ir, &p.ty, std::slice::from_ref(t))),
            Binding::Query => texts_of(&query, &p.name).map(|t| from_texts(ir, &p.ty, &t)),
            Binding::QueryObject => query_object(ir, p, &query),
            Binding::Header => {
                let name = p.header.as_deref().unwrap_or(&p.name);
                header_text(&headers, name).map(|t| from_texts(ir, &p.ty, &[t]))
            }
            Binding::Body => body_of(request),
        };
        if let Some(v) = found {
            out.insert(p.name.clone(), v);
        }
    }
    out
}

/// What the reference makes of a request: the fingerprint of the pipeline.
pub fn reference_fingerprint(ir: &Ir, name: &str, request: &Json) -> String {
    let (service, ep) = endpoint(ir, name);
    let mut req = Request::default();
    req.params = bind(ir, ep, request);
    req.key = Some(IdempotencyKey {
        value: "k".into(),
        store: StoreState::Empty,
    });
    let pipeline = Pipeline::new(ir, service, name).unwrap();
    let reported = pipeline.run_reported(&req, OutputMode::Strict).unwrap();
    reported
        .fingerprint
        .expect("a request that passes step 5 has a fingerprint")
}

// --- the tests -------------------------------------------------------------------------------------

fn accepted(cases: &[Json]) -> impl Iterator<Item = &Json> {
    cases.iter().filter(|c| c.get("rejected").is_none())
}

fn text<'a>(case: &'a Json, key: &str) -> &'a str {
    case[key]
        .as_str()
        .unwrap_or_else(|| panic!("{}: no `{key}`", case["name"]))
}

#[test]
fn every_case_has_the_fingerprint_of_the_reference_and_the_canonical_json_that_was_hashed() {
    let ir = contract();
    for case in accepted(&cases()) {
        let name = text(case, "name");
        let got = reference_fingerprint(&ir, text(case, "endpoint"), &case["request"]);
        assert_eq!(got, text(case, "fingerprint"), "{name}: fingerprint");
        // `hashed` is the canonical text of the bound parameters: it is that text, and the
        // fingerprint is the first 16 hex characters of its SHA-256 (what `fingerprint` computes).
        let parsed: Json = serde_json::from_str(text(case, "hashed")).unwrap();
        assert_eq!(
            canonical(&parsed),
            text(case, "hashed"),
            "{name}: not canonical"
        );
        let Json::Object(object) = parsed else {
            panic!("{name}: hashed is not an object")
        };
        let params: BTreeMap<String, Json> = object.into_iter().collect();
        assert_eq!(fingerprint(&params), got, "{name}: hashed");
    }
}

#[test]
fn the_pairs_that_must_be_equal_are_equal_and_the_pairs_that_must_differ_differ() {
    let all = cases();
    let by_name: BTreeMap<&str, &Json> = all.iter().map(|c| (text(c, "name"), c)).collect();
    assert_eq!(by_name.len(), all.len(), "case names are unique");
    let mut seen = (0, 0);
    for case in accepted(&all) {
        for (key, equal) in [("same_as", true), ("differs_from", false)] {
            let Some(other) = case.get(key).and_then(Json::as_str) else {
                continue;
            };
            let o = by_name
                .get(other)
                .unwrap_or_else(|| panic!("{other}: no such case"));
            assert_eq!(
                case["endpoint"], o["endpoint"],
                "{}: {key} {other} is another endpoint",
                case["name"]
            );
            assert_eq!(
                text(case, "fingerprint") == text(o, "fingerprint"),
                equal,
                "{} {key} {other}",
                case["name"]
            );
            assert_eq!(
                text(case, "hashed") == text(o, "hashed"),
                equal,
                "{} {key} {other}: hashed",
                case["name"]
            );
            if equal { seen.0 += 1 } else { seen.1 += 1 }
        }
    }
    assert!(seen.0 >= 30 && seen.1 >= 30, "{seen:?}");
}

#[test]
fn every_endpoint_of_the_contract_has_a_case_and_a_rejected_case_says_why_it_has_no_value() {
    let ir = contract();
    let all = cases();
    let used: BTreeSet<&str> = all.iter().map(|c| text(c, "endpoint")).collect();
    for ep in ir.services.iter().flat_map(|s| &s.endpoints) {
        assert!(used.contains(ep.name.as_str()), "no case for {}", ep.name);
    }
    for case in &all {
        let rejected = case.get("rejected").is_some();
        assert_eq!(
            rejected,
            case.get("fingerprint").is_none(),
            "{}",
            case["name"]
        );
        assert_eq!(rejected, case.get("hashed").is_none(), "{}", case["name"]);
        assert_eq!(case["request"]["method"], "PUT", "{}", case["name"]);
    }
}

#[test]
fn an_empty_request_hashes_the_empty_object() {
    let all = cases();
    let nothing = all
        .iter()
        .find(|c| c["name"] == "query_nothing_sent")
        .unwrap();
    assert_eq!(text(nothing, "hashed"), "{}");
    assert_eq!(text(nothing, "fingerprint"), fingerprint(&BTreeMap::new()));
}
