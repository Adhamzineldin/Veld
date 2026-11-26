//! `veld export slo`: the OpenSLO documents, byte for byte (a snapshot) and by plain assertions.

use veld_ir::ServiceId;

use super::yaml::quote;
use super::{kebab, openslo, seconds_text, target_of_errors};
use crate::testutil::ir;

const SOURCE: &str = "model Booking { id: uuid, guests: int }

service Bookings at \"/bookings\" {
  @slo(p99: ms(300), errors: 0.1)
  get \"/{id}\" getBooking(id: uuid) -> Booking | NotFound

  get \"/\" listBookings() -> Booking[]

  @slo(p50: ms(80), p95: seconds(1) + ms(500), p90: ms(200))
  post \"/\" createBooking(body: Booking) -> 201 Booking
}

@slo(errors: 5)
service Pings at \"/pings\" {
  get \"/\" ping() -> 204
}
";

fn export(src: &str) -> Option<String> {
    let ir = ir(src);
    let all: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    openslo(&ir, &all)
}

#[test]
fn the_documents_are_a_snapshot() {
    let text = export(SOURCE).unwrap();
    insta::assert_snapshot!("openslo", text);
}

#[test]
fn there_is_one_document_per_stated_objective_in_ir_order() {
    let text = export(SOURCE).unwrap();
    let names: Vec<&str> = text
        .lines()
        .filter_map(|l| l.strip_prefix("  name: \""))
        .filter(|l| !l.contains("-sli"))
        .map(|l| l.trim_end_matches('"'))
        .collect();
    assert_eq!(
        names,
        [
            "app-bookings-get-booking-p99",
            "app-bookings-get-booking-errors",
            "app-bookings-create-booking-p50",
            "app-bookings-create-booking-p90",
            "app-bookings-create-booking-p95",
            "app-pings-ping-errors",
        ]
    );
    assert_eq!(text.matches("apiVersion: \"openslo/v1\"").count(), 6);
    assert_eq!(text.matches("\n---\n").count(), 5);
}

#[test]
fn an_endpoint_without_an_objective_has_no_document() {
    let text = export(SOURCE).unwrap();
    assert!(!text.contains("list-bookings"));
}

#[test]
fn a_workspace_without_slo_has_nothing_to_export() {
    assert_eq!(
        export("model M { a: int }\nservice S at \"/s\" { get \"/\" f() -> M }"),
        None
    );
}

#[test]
fn the_output_is_deterministic() {
    assert_eq!(export(SOURCE), export(SOURCE));
}

#[test]
fn every_string_scalar_is_quoted_and_only_numbers_and_booleans_are_not() {
    let text = export(SOURCE).unwrap();
    for line in text.lines().filter(|l| !l.trim().is_empty() && *l != "---") {
        let body = line.trim_start().trim_start_matches("- ");
        let Some((_, value)) = body.split_once(": ") else {
            assert!(body.ends_with(':'), "{line}");
            continue;
        };
        let bare = value == "true" || value.parse::<f64>().is_ok();
        assert!(value.starts_with('"') || bare, "{line}");
    }
}

#[test]
fn the_queries_select_the_operation_and_the_bucket() {
    let text = export(SOURCE).unwrap();
    let ir = ir(SOURCE);
    let sig = &ir.services[0].endpoints[0].sig;
    assert!(
        text.contains(&format!(
            "sum(http_server_request_duration_seconds_bucket{{veld_operation=\\\"{sig}\\\",le=\\\"0.3\\\"}})"
        )),
        "{text}"
    );
    assert!(text.contains(&format!(
        "sum(http_server_request_duration_seconds_count{{veld_operation=\\\"{sig}\\\"}})"
    )));
    assert!(text.contains("http_response_status_code!~\\\"5..\\\""));
    assert!(text.contains("le=\\\"1.5\\\""));
}

#[test]
fn targets_are_the_share_and_one_minus_the_error_percent() {
    let text = export(SOURCE).unwrap();
    for target in [
        "target: 0.99",
        "target: 0.999",
        "target: 0.5",
        "target: 0.9",
        "target: 0.95",
    ] {
        assert!(text.contains(target), "{target}");
    }
    assert!(text.contains("target: 0.95\n") && text.contains("displayName: \"at most 5 percent"));
}

#[test]
fn seconds_are_the_shortest_decimal() {
    for (ms, want) in [
        ("300", "0.3"),
        ("1500", "1.5"),
        ("2000", "2"),
        ("80", "0.08"),
        ("1", "0.001"),
        ("61000", "61"),
        ("100", "0.1"),
    ] {
        assert_eq!(seconds_text(ms), want, "{ms}");
    }
}

#[test]
fn error_percent_becomes_an_exact_ratio() {
    for (errors, want) in [
        ("0.1", "0.999"),
        ("1", "0.99"),
        ("0.5", "0.995"),
        ("5", "0.95"),
        ("10", "0.9"),
        ("99.9", "0.001"),
        ("0.01", "0.9999"),
        ("12.5", "0.875"),
    ] {
        assert_eq!(target_of_errors(errors).as_deref(), Some(want), "{errors}");
    }
    assert_eq!(target_of_errors("a"), None);
}

#[test]
fn names_are_lowercase_words() {
    assert_eq!(kebab("getBooking"), "get-booking");
    assert_eq!(kebab("Bookings"), "bookings");
    assert_eq!(kebab("my_pkg.v2"), "my-pkg-v2");
    assert_eq!(kebab("HTTPServer"), "httpserver");
}

#[test]
fn quoting_escapes_what_yaml_would_read() {
    assert_eq!(quote("a\"b\\c"), "\"a\\\"b\\\\c\"");
    assert_eq!(quote("x\ny\t"), "\"x\\ny\\t\"");
    assert_eq!(quote("\u{1}"), "\"\\u0001\"");
    assert_eq!(quote("yes: no # x"), "\"yes: no # x\"");
}
