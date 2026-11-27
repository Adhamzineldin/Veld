//! `@limit(n, per: w, by: k)` (`spec/platform.md` section 5): `E1120`, what it lowers to, and
//! what a contract without it keeps.

use crate::testing::{codes, ir as build, one};
use veld_ir::{Endpoint, Feature, Ir, Limit, LimitSubject, LimitWindow};

const USER: &str = "model User { id: uuid\n tenantId?: uuid @tenant }\nprincipal User\n";
const ROOM: &str = "model Room { id: uuid }\n";

fn service(ann: &str) -> String {
    format!(
        "{USER}{ROOM}@auth(bearer)\nservice Rooms at \"/r\" {{\n  {ann}\n  get \"/{{id}}\" one(id: uuid) -> Room\n  get \"/\" all() -> Room[]\n}}"
    )
}

fn find<'a>(ir: &'a Ir, name: &str) -> &'a Endpoint {
    ir.services[0]
        .endpoints
        .iter()
        .find(|e| e.name == name)
        .unwrap()
}

fn limit(count: u32, per: LimitWindow, by: LimitSubject) -> Option<Limit> {
    Some(Limit { count, per, by })
}

#[test]
fn an_endpoint_limit_lowers_and_declares_the_error() {
    let ir = build(&service("@limit(10, per: minute, by: user)"));
    assert_eq!(
        find(&ir, "one").limit,
        limit(10, LimitWindow::Minute, LimitSubject::User)
    );
    assert!(find(&ir, "all").limit.is_none());
    let errors: Vec<String> = find(&ir, "one")
        .errors
        .iter()
        .map(|e| format!("{} {}", e.status, ir.decls[e.decl.index()].name))
        .collect();
    assert!(
        errors.contains(&"429 TooManyRequests".to_string()),
        "{errors:?}"
    );
    let used = ir.features_used(&[ir.services[0].id]);
    assert!(used.contains(&Feature::Limits), "{used:?}");
}

#[test]
fn a_service_limit_is_the_default_and_an_endpoint_limit_replaces_it() {
    let src = format!(
        "{USER}{ROOM}@limit(100, per: hour, by: ip)\n@auth(bearer)\nservice Rooms at \"/r\" {{\n  @limit(5, per: second, by: tenant)\n  get \"/{{id}}\" one(id: uuid) -> Room\n  get \"/\" all() -> Room[]\n  get \"/ticks\" ticks() -> stream Room\n}}"
    );
    let ir = build(&src);
    assert_eq!(
        find(&ir, "one").limit,
        limit(5, LimitWindow::Second, LimitSubject::Tenant)
    );
    assert_eq!(
        find(&ir, "all").limit,
        limit(100, LimitWindow::Hour, LimitSubject::Ip)
    );
    assert!(
        find(&ir, "ticks").limit.is_none(),
        "a stream is not counted by call"
    );
}

#[test]
fn every_window_and_subject_is_accepted_in_either_order() {
    for per in ["second", "minute", "hour", "day"] {
        for by in ["user", "tenant", "ip"] {
            let src = service(&format!("@limit(1, by: {by}, per: {per})"));
            assert_eq!(codes(&src), Vec::<&str>::new(), "{per} {by}");
        }
    }
}

#[test]
fn a_bad_count_window_or_subject_is_e1120() {
    for bad in [
        "@limit(0, per: minute, by: user)",
        "@limit(-1, per: minute, by: user)",
        "@limit(1.5, per: minute, by: user)",
        "@limit(\"ten\", per: minute, by: user)",
        "@limit(99999999999, per: minute, by: user)",
        "@limit(10, per: week, by: user)",
        "@limit(10, per: 5, by: user)",
        "@limit(10, per: minute, by: group)",
        "@limit(10, per: minute)",
        "@limit(10, by: user)",
        "@limit(per: minute, by: user)",
        "@limit",
        "@limit(10, per: minute, by: user, by: ip)",
        "@limit(10, 20, per: minute, by: user)",
        "@limit(10, per: minute, by: user, burst: 3)",
    ] {
        assert_eq!(codes(&service(bad)), vec!["E1120"], "{bad}");
    }
}

#[test]
fn two_limits_on_one_endpoint_are_e1120() {
    let src = service("@limit(1, per: second, by: ip)\n  @limit(2, per: second, by: ip)");
    assert_eq!(codes(&src), vec!["E1120"]);
}

#[test]
fn a_limit_on_a_model_or_a_field_is_e0602() {
    assert!(codes("@limit(1, per: second, by: ip)\nmodel M { a: int }\n").contains(&"E0602"));
    assert!(codes("model M { a: int @limit(1, per: second, by: ip) }\n").contains(&"E0602"));
}

#[test]
fn by_user_and_by_tenant_need_auth() {
    for by in ["user", "tenant"] {
        let src = format!(
            "{USER}{ROOM}@auth(none)\nservice Rooms at \"/r\" {{\n  @limit(1, per: second, by: {by})\n  get \"/\" all() -> Room[]\n}}"
        );
        assert_eq!(codes(&src), vec!["E1120"], "{by}");
    }
    let ip = format!(
        "{USER}{ROOM}@auth(none)\nservice Rooms at \"/r\" {{\n  @limit(1, per: second, by: ip)\n  get \"/\" all() -> Room[]\n}}"
    );
    assert_eq!(codes(&ip), Vec::<&str>::new());
}

#[test]
fn a_service_default_that_does_not_fit_an_endpoint_is_e1120_at_the_endpoint() {
    let src = format!(
        "{ROOM}@limit(1, per: second, by: user)\nservice Rooms at \"/r\" {{\n  @auth(none)\n  get \"/\" all() -> Room[]\n}}"
    );
    assert_eq!(codes(&src), vec!["E1120"]);
}

#[test]
fn by_tenant_needs_a_principal_with_a_tenant() {
    let none = format!(
        "{ROOM}@auth(bearer)\nservice Rooms at \"/r\" {{\n  @limit(1, per: second, by: tenant)\n  get \"/\" all() -> Room[]\n}}"
    );
    assert_eq!(codes(&none), vec!["E1120"]);
    let plain = format!(
        "model User {{ id: uuid }}\nprincipal User\n{ROOM}@auth(bearer)\nservice Rooms at \"/r\" {{\n  @limit(1, per: second, by: tenant)\n  get \"/\" all() -> Room[]\n}}"
    );
    assert_eq!(codes(&plain), vec!["E1120"]);
}

#[test]
fn a_limit_on_a_stream_of_its_own_is_e1120() {
    let src = format!(
        "{ROOM}service Rooms at \"/r\" {{\n  @limit(1, per: second, by: ip)\n  get \"/ticks\" ticks() -> stream Room\n}}"
    );
    assert_eq!(codes(&src), vec!["E1120"]);
}

#[test]
fn a_contract_without_a_limit_keeps_its_json_and_its_errors() {
    let ir = build(&format!(
        "{ROOM}service Rooms at \"/r\" {{ get \"/\" all() -> Room[] }}"
    ));
    let json = serde_json::to_string(&ir).unwrap();
    assert!(
        !json.contains("\"limit\"") && !json.contains("TooManyRequests"),
        "{json}"
    );
    assert!(
        !ir.features_used(&[ir.services[0].id])
            .contains(&Feature::Limits)
    );
}

#[test]
fn limit_and_its_words_stay_ordinary_names() {
    assert!(
        one("model M { limit: int\n per: string\n by: string\n user: bool\n tenant: int\n ip: string\n second: int }\n")
            .ir
            .is_some()
    );
}
