//! `veld export slo` (`spec/platform.md` section 13): the `@slo` of a workspace as OpenSLO `v1`
//! documents.
//!
//! One `SLO` per stated objective, in the order of the IR (package, service, endpoint) and, for an
//! endpoint, `p50`, `p90`, `p95`, `p99`, `errors`. Every SLO is a ratio over a 28-day rolling
//! window whose `good` and `total` queries are written for Prometheus over the metric of the
//! `metrics` hook (`http.server.request.duration`, which an OpenTelemetry exporter names
//! `http_server_request_duration_seconds`).
//!
//! The documents have a fixed shape, so the YAML is written by hand ([`yaml`]): no dependency, the
//! same bytes on every run, and every string scalar quoted.

mod yaml;

use veld_ir::{Endpoint, Ir, Service, ServiceId};
use yaml::Doc;

/// The Prometheus metric the `metrics` hook feeds, as an exporter names it.
const BUCKET: &str = "http_server_request_duration_seconds_bucket";
const COUNT: &str = "http_server_request_duration_seconds_count";

/// One objective of one endpoint, ready to be written.
struct Objective {
    /// `p99`, `errors`, ...
    key: &'static str,
    /// The text of the objective: `p99 under 300 ms`.
    display: String,
    /// What the SLO says in a sentence.
    description: String,
    /// The target ratio of good requests, as a decimal such as `0.99`.
    target: String,
    /// The PromQL of the good and of the total requests.
    good: String,
    total: String,
}

/// The documents for the services in `services`, or `None` when none of their endpoints states an
/// objective.
#[must_use]
pub fn openslo(ir: &Ir, services: &[ServiceId]) -> Option<String> {
    let mut docs: Vec<String> = Vec::new();
    for sid in services {
        let service = ir.service(*sid);
        let package = &ir.package(service.package).name;
        for ep in &service.endpoints {
            for o in objectives(ep) {
                docs.push(document(package, service, ep, &o));
            }
        }
    }
    (!docs.is_empty()).then(|| docs.join("---\n"))
}

/// The objectives of an endpoint in the order `p50`, `p90`, `p95`, `p99`, `errors`.
fn objectives(ep: &Endpoint) -> Vec<Objective> {
    let Some(slo) = &ep.slo else {
        return Vec::new();
    };
    let selector = format!("veld_operation={}", quote_label(&ep.sig));
    let total = format!("sum({COUNT}{{{selector}}})");
    let mut out = Vec::new();
    for (key, ms) in slo.latencies() {
        let Some(share) = share_of(key) else { continue };
        let seconds = seconds_text(ms);
        out.push(Objective {
            key: static_key(key),
            display: format!("{key} under {ms} ms"),
            description: format!(
                "{share_text}% of the requests of {name} answer within {ms} ms.",
                share_text = share.percent,
                name = ep.name
            ),
            target: share.target.to_owned(),
            good: format!("sum({BUCKET}{{{selector},le=\"{seconds}\"}})"),
            total: total.clone(),
        });
    }
    if let Some(errors) = &slo.errors
        && let Some(target) = target_of_errors(errors)
    {
        out.push(Objective {
            key: "errors",
            display: format!("at most {errors} percent of requests end in a 5xx"),
            description: format!(
                "At most {errors} percent of the requests of {} end in a 5xx.",
                ep.name
            ),
            target,
            good: format!("sum({COUNT}{{{selector},http_response_status_code!~\"5..\"}})"),
            total,
        });
    }
    out
}

/// The share of requests a latency objective covers.
struct Share {
    percent: &'static str,
    target: &'static str,
}

fn share_of(key: &str) -> Option<Share> {
    Some(match key {
        "p50" => Share {
            percent: "50",
            target: "0.5",
        },
        "p90" => Share {
            percent: "90",
            target: "0.9",
        },
        "p95" => Share {
            percent: "95",
            target: "0.95",
        },
        "p99" => Share {
            percent: "99",
            target: "0.99",
        },
        _ => return None,
    })
}

fn static_key(key: &str) -> &'static str {
    match key {
        "p50" => "p50",
        "p90" => "p90",
        "p95" => "p95",
        _ => "p99",
    }
}

/// A PromQL label value: double-quoted, with `\` and `"` escaped.
fn quote_label(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Milliseconds as seconds, in the shortest decimal: `300` is `0.3`, `1500` is `1.5`, `2000` is `2`.
#[must_use]
pub fn seconds_text(ms: &str) -> String {
    let Ok(n) = ms.parse::<u64>() else {
        return ms.to_owned();
    };
    let (whole, frac) = (n / 1000, n % 1000);
    if frac == 0 {
        return whole.to_string();
    }
    let frac = format!("{frac:03}");
    format!("{whole}.{}", frac.trim_end_matches('0'))
}

/// The target ratio of good requests for at most `errors` percent of failures: `0.1` is `0.999`.
/// Exact decimal arithmetic on the text, so no float rounding reaches the document.
#[must_use]
pub fn target_of_errors(errors: &str) -> Option<String> {
    let (int, frac) = errors.split_once('.').unwrap_or((errors, ""));
    if int.is_empty() || !int.bytes().chain(frac.bytes()).all(|b| b.is_ascii_digit()) {
        return None;
    }
    let scale = u32::try_from(frac.len()).ok()?;
    let digits: u128 = format!("{int}{frac}").parse().ok()?;
    let hundred = 100u128.checked_mul(10u128.checked_pow(scale)?)?;
    let good = hundred.checked_sub(digits)?;
    // `good` is the percent of good requests, scaled by 10^scale; the ratio is that over 100.
    let text = format!("{good:0>width$}", width = usize::try_from(scale).ok()? + 3);
    let split = text.len() - usize::try_from(scale).ok()? - 2;
    let (whole, frac) = text.split_at(split);
    let frac = frac.trim_end_matches('0');
    let whole = whole.trim_start_matches('0');
    let whole = if whole.is_empty() { "0" } else { whole };
    Some(if frac.is_empty() {
        whole.to_owned()
    } else {
        format!("{whole}.{frac}")
    })
}

/// Lowercase words joined by `-`: `getBooking` is `get-booking`, `Bookings_API` is `bookings-api`.
#[must_use]
pub fn kebab(name: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && prev_lower {
                out.push('-');
            }
            out.push(c.to_ascii_lowercase());
            prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        } else {
            if !out.ends_with('-') && !out.is_empty() {
                out.push('-');
            }
            prev_lower = false;
        }
    }
    out.trim_end_matches('-').to_owned()
}

fn document(package: &str, service: &Service, ep: &Endpoint, o: &Objective) -> String {
    let service_name = format!("{}-{}", kebab(package), kebab(&service.name));
    let name = format!("{service_name}-{}-{}", kebab(&ep.name), o.key);
    let mut d = Doc::new();
    d.string("apiVersion", "openslo/v1");
    d.string("kind", "SLO");
    d.open("metadata");
    d.string("name", &name);
    d.string(
        "displayName",
        &format!("{package}/{}.{} {}", service.name, ep.name, o.key),
    );
    d.close();
    d.open("spec");
    d.string("description", &o.description);
    d.string("service", &service_name);
    d.open("indicator");
    d.open("metadata");
    d.string("name", &format!("{name}-sli"));
    d.close();
    d.open("spec");
    d.open("ratioMetric");
    d.raw("counted", "true");
    for (key, query) in [("good", &o.good), ("total", &o.total)] {
        d.open(key);
        d.open("metricSource");
        d.string("type", "Prometheus");
        d.open("spec");
        d.string("query", query);
        d.close();
        d.close();
        d.close();
    }
    d.close();
    d.close();
    d.close();
    d.list_open("timeWindow");
    d.item_string("duration", "28d");
    d.item_raw("isRolling", "true");
    d.list_close();
    d.string("budgetingMethod", "Occurrences");
    d.list_open("objectives");
    d.item_string("displayName", &o.display);
    d.item_raw("target", &o.target);
    d.list_close();
    d.close();
    d.finish()
}

#[cfg(test)]
mod tests;
