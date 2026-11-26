//! `@timeout` and `@slo` (`spec/platform.md` sections 12 and 13): where they may stand, what they
//! lower to, and what a contract without them keeps.

use crate::testing::{codes, ir as build};
use veld_ir::{Feature, Ir};

const ROOM: &str = "model Room { id: uuid, name: string }\n";

fn find<'a>(ir: &'a Ir, name: &str) -> &'a veld_ir::Endpoint {
    ir.services[0]
        .endpoints
        .iter()
        .find(|e| e.name == name)
        .unwrap()
}

fn error_names(ir: &Ir, ep: &veld_ir::Endpoint) -> Vec<String> {
    ep.errors
        .iter()
        .map(|e| format!("{} {}", e.status, ir.decls[e.decl.index()].name))
        .collect()
}

// ---- @timeout ---------------------------------------------------------------------------------

#[test]
fn a_timeout_on_an_endpoint_is_milliseconds_in_the_ir_and_declares_the_error() {
    let src = format!(
        "{ROOM}service Rooms at \"/r\" {{\n  @timeout(seconds(2))\n  get \"/{{id}}\" one(id: uuid) -> Room\n  get \"/all\" all() -> Room[]\n}}"
    );
    let ir = build(&src);
    assert_eq!(find(&ir, "one").timeout_ms.as_deref(), Some("2000"));
    assert!(find(&ir, "all").timeout_ms.is_none());
    assert!(error_names(&ir, find(&ir, "one")).contains(&"504 DeadlineExceeded".to_string()));
    assert!(!error_names(&ir, find(&ir, "all")).contains(&"504 DeadlineExceeded".to_string()));
    let used = ir.features_used(&[ir.services[0].id]);
    assert!(used.contains(&Feature::Timeouts), "{used:?}");
}

#[test]
fn the_prelude_error_is_reached_only_by_a_contract_that_uses_a_timeout() {
    let plain = build(&format!(
        "{ROOM}service Rooms at \"/r\" {{ get \"/\" all() -> Room[] }}"
    ));
    assert!(!plain.decls.iter().any(|d| d.name == "DeadlineExceeded"));
    let json = serde_json::to_string(&plain).unwrap();
    assert!(
        !json.contains("timeout_ms") && !json.contains("\"slo\""),
        "{json}"
    );
    assert!(
        !plain
            .features_used(&[plain.services[0].id])
            .contains(&Feature::Timeouts)
    );
    let timed = build(&format!(
        "{ROOM}service Rooms at \"/r\" {{ @timeout(ms(500))\n get \"/\" all() -> Room[] }}"
    ));
    assert!(timed.decls.iter().any(|d| d.name == "DeadlineExceeded"));
}

#[test]
fn a_service_timeout_is_the_default_and_an_endpoint_replaces_it() {
    let src = format!(
        "{ROOM}@timeout(seconds(5))\nservice Rooms at \"/r\" {{\n  get \"/a\" a() -> Room[]\n  @timeout(ms(250))\n  get \"/b\" b() -> Room[]\n}}"
    );
    let ir = build(&src);
    assert_eq!(find(&ir, "a").timeout_ms.as_deref(), Some("5000"));
    assert_eq!(find(&ir, "b").timeout_ms.as_deref(), Some("250"));
}

#[test]
fn a_service_timeout_skips_a_streaming_endpoint_but_an_own_one_is_an_error() {
    let svc = format!(
        "{ROOM}@timeout(seconds(5))\nservice Rooms at \"/r\" {{\n  get \"/a\" a() -> Room[]\n  get \"/s\" s() -> stream Room\n}}"
    );
    let ir = build(&svc);
    assert!(find(&ir, "s").timeout_ms.is_none());
    assert_eq!(find(&ir, "a").timeout_ms.as_deref(), Some("5000"));
    let own = format!(
        "{ROOM}service Rooms at \"/r\" {{\n  @timeout(seconds(5))\n  get \"/s\" s() -> stream Room\n}}"
    );
    assert_eq!(codes(&own), ["E0602"]);
}

#[test]
fn a_wrong_timeout_is_e0602() {
    let on = |anno: &str| {
        codes(&format!(
            "{ROOM}service Rooms at \"/r\" {{\n  {anno}\n  get \"/\" all() -> Room[]\n}}"
        ))
    };
    for bad in [
        "@timeout",
        "@timeout()",
        "@timeout(2)",
        "@timeout(seconds(0))",
        "@timeout(hours(2))",
        "@timeout(seconds(1), seconds(2))",
        "@timeout(d: seconds(1))",
        "@timeout(\"2s\")",
        "@timeout(fortnights(1))",
    ] {
        assert_eq!(on(bad), ["E0602"], "{bad}");
    }
    for ok in [
        "@timeout(hours(1))",
        "@timeout(ms(1))",
        "@timeout(minutes(59))",
    ] {
        assert!(on(ok).is_empty(), "{ok}");
    }
    assert_eq!(codes("@timeout(seconds(1))\nmodel M { a: int }"), ["E0602"]);
}

// ---- @slo --------------------------------------------------------------------------------------

#[test]
fn an_slo_is_in_the_ir_with_its_milliseconds_and_percent() {
    let src = format!(
        "{ROOM}service Rooms at \"/r\" {{\n  @slo(p99: ms(300), errors: 0.1, p50: ms(80))\n  get \"/\" all() -> Room[]\n}}"
    );
    let ir = build(&src);
    let slo = find(&ir, "all").slo.clone().unwrap();
    assert_eq!(slo.p99_ms.as_deref(), Some("300"));
    assert_eq!(slo.p50_ms.as_deref(), Some("80"));
    assert_eq!(slo.errors.as_deref(), Some("0.1"));
    assert!(slo.p95_ms.is_none());
    let used = ir.features_used(&[ir.services[0].id]);
    assert!(used.contains(&Feature::Slo), "{used:?}");
    assert!(!used.contains(&Feature::Timeouts), "{used:?}");
    // An slo declares no error and does not change the endpoint's errors.
    assert!(!error_names(&ir, find(&ir, "all")).contains(&"504 DeadlineExceeded".to_string()));
}

#[test]
fn a_service_slo_is_the_default_of_its_endpoints() {
    let src = format!(
        "{ROOM}@slo(p95: seconds(1))\nservice Rooms at \"/r\" {{\n  get \"/a\" a() -> Room[]\n  @slo(errors: 1)\n  get \"/b\" b() -> Room[]\n}}"
    );
    let ir = build(&src);
    assert_eq!(
        find(&ir, "a")
            .slo
            .as_ref()
            .and_then(|s| s.p95_ms.as_deref()),
        Some("1000")
    );
    let b = find(&ir, "b").slo.clone().unwrap();
    assert!(b.p95_ms.is_none());
    assert_eq!(b.errors.as_deref(), Some("1"));
}

#[test]
fn a_wrong_slo_is_e0602() {
    let on = |anno: &str| {
        codes(&format!(
            "{ROOM}service Rooms at \"/r\" {{\n  {anno}\n  get \"/\" all() -> Room[]\n}}"
        ))
    };
    for bad in [
        "@slo",
        "@slo()",
        "@slo(ms(300))",
        "@slo(p98: ms(300))",
        "@slo(p99: 300)",
        "@slo(p99: ms(0))",
        "@slo(p99: ms(1), p99: ms(2))",
        "@slo(errors: 0)",
        "@slo(errors: 100)",
        "@slo(errors: \"1\")",
        "@slo(p95: ms(500), p99: ms(300))",
    ] {
        assert_eq!(on(bad), ["E0602"], "{bad}");
    }
    assert!(
        on("@slo(p50: ms(10), p90: ms(20), p95: ms(30), p99: ms(40), errors: 99.9)").is_empty()
    );
    let stream = format!(
        "{ROOM}service Rooms at \"/r\" {{\n  @slo(errors: 1)\n  get \"/s\" s() -> stream Room\n}}"
    );
    assert_eq!(codes(&stream), ["E0602"]);
}

#[test]
fn neither_annotation_changes_an_operation_signature_or_a_digest() {
    let plain = build(&format!(
        "{ROOM}service Rooms at \"/r\" {{ get \"/\" all() -> Room[] }}"
    ));
    let both = build(&format!(
        "{ROOM}service Rooms at \"/r\" {{ @timeout(seconds(1))\n @slo(errors: 1)\n get \"/\" all() -> Room[] }}"
    ));
    assert_eq!(
        plain.services[0].endpoints[0].sig,
        both.services[0].endpoints[0].sig
    );
    assert_eq!(
        plain.services[0].rules_digest,
        both.services[0].rules_digest
    );
}

#[test]
fn the_words_stay_ordinary_names() {
    let src = "model M { timeout: int, slo: int }\nservice S at \"/s\" {\n get \"/a\" timeout(@query slo: int) -> M\n get \"/b\" slo() -> M\n}";
    assert!(codes(src).is_empty());
}
