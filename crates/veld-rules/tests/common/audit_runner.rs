//! Runs one case of `tests/wire/audit/cases.json` through the reference: the one place that says
//! how a case becomes a pipeline request. The test of the goldens and the tool that printed them
//! use it, so the bytes in the file are what the test checks.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use serde_json::Value as Json;
use veld_ir::{AuthKind, Binding, Endpoint, Ir, Service};
use veld_rules::audit::{Ended, event};
use veld_rules::{Outcome, OutputMode, Pipeline, Request};

/// What the server did with the request of a case.
pub struct Ran {
    /// The audit event, `None` for an endpoint that is not audited.
    pub event: Option<Json>,
    pub status: u16,
    pub error: Option<String>,
}

fn find<'a>(ir: &'a Ir, case: &Json) -> (&'a Service, &'a Endpoint) {
    let service = ir
        .services
        .iter()
        .find(|s| s.name == case["service"].as_str().unwrap())
        .unwrap();
    let endpoint = service
        .endpoints
        .iter()
        .find(|e| e.name == case["endpoint"].as_str().unwrap())
        .unwrap();
    (service, endpoint)
}

/// The parameters of the request, by name: a path parameter from the segment of the path, the
/// body from the body. The goldens use no other binding.
fn bind(endpoint: &Endpoint, request: &Json) -> BTreeMap<String, Json> {
    let template: Vec<&str> = endpoint.path.trim_matches('/').split('/').collect();
    let path = request["path"].as_str().unwrap();
    let actual: Vec<&str> = path.trim_matches('/').split('/').collect();
    let mut out = BTreeMap::new();
    for p in &endpoint.params {
        match p.binding {
            Binding::Path => {
                let at = template
                    .iter()
                    .position(|s| *s == format!("{{{}}}", p.name))
                    .unwrap();
                out.insert(p.name.clone(), Json::String(actual[at].to_owned()));
            }
            Binding::Body => {
                if let Some(body) = request.get("body").filter(|b| !b.is_null()) {
                    out.insert(p.name.clone(), body.clone());
                }
            }
            Binding::Query | Binding::QueryObject | Binding::Header => {
                panic!("the audit goldens bind only the path and the body")
            }
        }
    }
    out
}

pub fn run(ir: &Ir, case: &Json, clock_ms: i64) -> Ran {
    let (service, endpoint) = find(ir, case);
    let principal_decl = service.principal.as_ref().map(|p| p.decl);
    let caller = Some(&case["principal"]).filter(|p| !p.is_null());
    let empty = BTreeMap::new();
    // The answers a server gives before the pipeline: no caller, and a limiter that refuses.
    let (status, error) = if endpoint.auth.kind != AuthKind::None && caller.is_none() {
        (401, "Unauthorized")
    } else if case["limiter"]["allowed"] == Json::Bool(false) {
        (429, "TooManyRequests")
    } else {
        return through_the_pipeline(ir, service, endpoint, case, clock_ms);
    };
    let ended = Ended {
        time_ms: clock_ms,
        principal: caller.filter(|_| endpoint.auth.kind != AuthKind::None),
        status,
        error: Some(error),
        parameters: None,
        facts: &empty,
        result: None,
        hidden: &[],
    };
    Ran {
        event: endpoint
            .audited
            .then(|| event(ir, endpoint, principal_decl, &ended).unwrap()),
        status,
        error: Some(error.to_owned()),
    }
}

fn through_the_pipeline(
    ir: &Ir,
    service: &Service,
    endpoint: &Endpoint,
    case: &Json,
    clock_ms: i64,
) -> Ran {
    let facts: BTreeMap<String, Option<Json>> = case["facts"]
        .as_object()
        .map(|m| {
            m.iter()
                .map(|(k, v)| (k.clone(), Some(v.clone()).filter(|v| !v.is_null())))
                .collect()
        })
        .unwrap_or_default();
    let request = Request {
        params: bind(endpoint, &case["request"]),
        facts,
        principal: Some(case["principal"].clone()).filter(|p| !p.is_null()),
        now: clock_ms,
        result: Some(case["returns"].clone()).filter(|r| !r.is_null()),
        if_match: case["request"]["headers"]["If-Match"]
            .as_str()
            .map(str::to_owned),
        ..Request::default()
    };
    let pipeline = Pipeline::new(ir, service, &endpoint.name).unwrap();
    let reported = pipeline.run_reported(&request, OutputMode::Strict).unwrap();
    let (status, error) = match &reported.outcome {
        Outcome::Passes => (endpoint.success.status, None),
        Outcome::Error(a) => (a.status, Some(a.name.clone())),
    };
    Ran {
        event: endpoint
            .audited
            .then(|| pipeline.audit_event(&request, &reported).unwrap()),
        status,
        error,
    }
}
