//! Service levels over the production handler (`spec/platform.md` section 13), generated from
//! `tests/toolchain/contract/platform_p2/service_levels.veld`: the metric of a `200`, of a declared
//! error, of a `500` and of a `504`, every endpoint measured (one without an objective too) and a
//! route that matches nothing not, and a hook that fails ignored.

use levels_harness::{ID, send, serve};
use serde_json::json;

fn last(s: &levels_harness::Server) -> serde_json::Value {
    s.world.metrics.lock().unwrap().last().expect("a metric").to_json()
}

/// The value of section 13 for a `200`, a declared error (`404`), an error that becomes `500`
/// (`error.type` only here) and the `504` of a deadline.
async fn the_metric_of_every_kind_of_answer() {
    let s = serve().await;
    assert_eq!(send(&s, "GET", &format!("/rooms/{ID}"), None, None).await.status, 200);
    let m = last(&s);
    assert_eq!(m["name"], json!("http.server.request.duration"));
    assert_eq!(m["unit"], json!("s"));
    assert!(m["value"].as_f64().is_some_and(|v| v >= 0.0 && v < 5.0), "{m}");
    let a = &m["attributes"];
    assert_eq!(a["http.request.method"], json!("GET"));
    assert_eq!(a["http.route"], json!("/rooms/{id}"));
    assert_eq!(a["http.response.status_code"], json!(200));
    assert!(a.get("error.type").is_none(), "only a 5xx has it: {m}");
    assert_eq!(a["veld.service"], json!("rooms/Rooms"));
    assert_eq!(a["veld.endpoint"], json!("getRoom"));
    assert!(a["veld.operation"].as_str().is_some_and(|op| op.starts_with("veld:") && op.contains("Rooms.getRoom@")), "{m}");

    *s.world.fail.lock().unwrap() = Some("missing");
    assert_eq!(send(&s, "GET", &format!("/rooms/{ID}"), None, None).await.status, 404);
    let a = last(&s)["attributes"].clone();
    assert_eq!(a["http.response.status_code"], json!(404));
    assert!(a.get("error.type").is_none());

    *s.world.fail.lock().unwrap() = Some("boom");
    assert_eq!(send(&s, "GET", &format!("/rooms/{ID}"), None, None).await.status, 500);
    let a = last(&s)["attributes"].clone();
    assert_eq!((a["http.response.status_code"].clone(), a["error.type"].clone()), (json!(500), json!("500")));

    *s.world.delay_ms.lock().unwrap() = 500;
    assert_eq!(send(&s, "GET", &format!("/rooms/{ID}"), None, None).await.status, 504);
    let m = last(&s);
    assert_eq!((m["attributes"]["http.response.status_code"].clone(), m["attributes"]["error.type"].clone()), (json!(504), json!("504")));
    assert!(m["value"].as_f64().is_some_and(|v| v >= 0.19), "the 504 is what is measured: {m}");
}

/// Every request that matched an endpoint is measured, one that states no objective too (`listRooms`
/// takes the service's); a route that matches nothing is not. The metric is called once per request.
async fn every_endpoint_is_measured_and_a_route_that_matches_nothing_is_not() {
    let s = serve().await;
    assert_eq!(send(&s, "GET", "/rooms", None, None).await.status, 200);
    assert_eq!(send(&s, "GET", "/nowhere", None, None).await.status, 404);
    let seen: Vec<String> = s.world.metrics.lock().unwrap().iter().map(|m| m.veld_endpoint.clone()).collect();
    assert_eq!(seen, vec!["listRooms"]);
}

/// A hook that fails is ignored: the answer is the one the pipeline made.
async fn a_hook_that_fails_is_ignored() {
    let s = serve().await;
    *s.world.metrics_panic.lock().unwrap() = true;
    assert_eq!(send(&s, "GET", &format!("/rooms/{ID}"), None, None).await.status, 200);
    assert_eq!(s.world.metrics.lock().unwrap().len(), 1, "the hook was called");
}

#[tokio::main]
async fn main() {
    the_metric_of_every_kind_of_answer().await;
    every_endpoint_is_measured_and_a_route_that_matches_nothing_is_not().await;
    a_hook_that_fails_is_ignored().await;
    println!("slo ok");
}
