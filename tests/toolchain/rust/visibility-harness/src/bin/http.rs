//! Field visibility over the production handler (`spec/platform.md` section 2), generated from
//! `tests/toolchain/contract/platform_p1/visibility.veld`: a field hidden for one caller and present
//! for another, inside a list and a page, an expression that cannot be evaluated, the clock of the
//! request to the millisecond, an `ensure` that still sees the field, the data of a declared error,
//! an endpoint with no caller, and the settings that change nothing (`validate = false`, the rule
//! output `off`). The program prints `visibility http ok`.

use serde_json::json;
use visibility_harness::{ALICE, OPENS_MS, Servers, send, serve};

fn phone(doc: &serde_json::Value) -> bool {
    doc["guest"].get("phone").is_some()
}

/// The guest's own phone is there for the guest and for support, not for another caller; the note
/// is for support only; `ratio` has a divisor that works.
fn one_value(s: &Servers) {
    let id = "/desk/00000000-0000-4000-8000-000000000001";
    let alice = send(&s.desk, "GET", id, Some("alice"));
    assert_eq!(alice.status, 200, "{}", alice.doc);
    assert!(phone(&alice.doc) && alice.doc.get("note").is_none() && alice.doc.get("ratio").is_some(), "{}", alice.doc);
    assert_eq!(alice.doc["guest"]["id"], json!(ALICE));
    let bob = send(&s.desk, "GET", id, Some("bob"));
    assert!(!phone(&bob.doc) && bob.doc.get("note").is_none(), "{}", bob.doc);
    let support = send(&s.desk, "GET", id, Some("support"));
    assert!(phone(&support.doc) && support.doc.get("note").is_some(), "{}", support.doc);
}

/// Each element of a list and each item of a page is judged for the caller.
fn list_and_page(s: &Servers) {
    let list = send(&s.desk, "GET", "/desk", Some("alice"));
    let items = list.doc.as_array().expect("a list");
    assert!(phone(&items[0]) && !phone(&items[1]), "{}", list.doc);
    let page = send(&s.desk, "GET", "/desk/page", Some("alice"));
    let items = page.doc["items"].as_array().expect("a page");
    assert!(phone(&items[0]) && !phone(&items[1]), "{}", page.doc);
    assert_eq!(page.doc["total"], json!("2"), "a long is text on the wire");
}

/// An expression that cannot be evaluated (a division by zero) hides the field for everyone,
/// support included; a request does not fail because of it.
fn an_expression_that_fails_hides_the_field(s: &Servers) {
    *s.world.divisor.lock().unwrap() = 0;
    for token in ["alice", "support"] {
        let a = send(&s.desk, "GET", "/desk/00000000-0000-4000-8000-000000000001", Some(token));
        assert_eq!(a.status, 200, "{}", a.doc);
        assert!(a.doc.get("ratio").is_none(), "{token}: {}", a.doc);
    }
    *s.world.divisor.lock().unwrap() = 1;
}

/// `now` is the clock of the request: not shown before the opening time, shown at the exact
/// millisecond and after.
fn the_clock_of_the_request(s: &Servers) {
    let id = "/desk/00000000-0000-4000-8000-000000000001";
    for (at, shown) in [(OPENS_MS - 1, false), (OPENS_MS, true), (OPENS_MS + 1, true)] {
        *s.world.clock_ms.lock().unwrap() = at;
        let a = send(&s.desk, "GET", id, Some("alice"));
        assert_eq!(a.doc.get("early").is_some(), shown, "at {at}: {}", a.doc);
    }
}

/// An `ensure` sees the whole value (the note the caller then does not get); one that fails is a
/// `500`, whoever the caller is (unless the rule output is `off`).
fn an_ensure_sees_the_field(s: &Servers, off: bool) {
    let id = "/desk/checked/00000000-0000-4000-8000-000000000001";
    let a = send(&s.desk, "GET", id, Some("alice"));
    assert_eq!(a.status, 200, "{}", a.doc);
    assert!(a.doc.get("note").is_none(), "{}", a.doc);
    // With the output off nothing checks the `ensure`: the answer is the value, without the field.
    *s.world.note.lock().unwrap() = false;
    assert_eq!(send(&s.desk, "GET", id, Some("support")).status, if off { 200 } else { 500 });
    *s.world.note.lock().unwrap() = true;
}

/// The data of a declared error is filtered too: `who` is for support.
fn a_declared_error(s: &Servers) {
    *s.world.divisor.lock().unwrap() = 0;
    let path = "/desk/00000000-0000-4000-8000-000000000001/cancel";
    let alice = send(&s.desk, "POST", path, Some("alice"));
    assert_eq!(alice.status, 409, "{}", alice.doc);
    assert_eq!(alice.doc["data"]["reason"], json!("closed"));
    assert!(alice.doc["data"].get("who").is_none(), "{}", alice.doc);
    let support = send(&s.desk, "POST", path, Some("support"));
    assert_eq!(support.doc["data"]["who"], json!("front desk"), "{}", support.doc);
    *s.world.divisor.lock().unwrap() = 1;
}

/// An endpoint with no caller drops every `@visible` field; a request with no token is `401`.
fn no_caller(s: &Servers) {
    let a = send(&s.notices, "GET", "/notices/00000000-0000-4000-8000-000000000001", None);
    assert_eq!(a.status, 200, "{}", a.doc);
    for field in ["note", "ratio", "early"] {
        assert!(a.doc.get(field).is_none(), "{field}: {}", a.doc);
    }
    assert!(!phone(&a.doc), "{}", a.doc);
    assert_eq!(send(&s.desk, "GET", "/desk/00000000-0000-4000-8000-000000000001", None).status, 401);
}

fn main() {
    // The settings that change nothing: validation of requests off, the rule output off.
    for (validate, off) in [(true, false), (false, false), (true, true), (false, true)] {
        let s = serve(validate, off);
        one_value(&s);
        list_and_page(&s);
        an_expression_that_fails_hides_the_field(&s);
        the_clock_of_the_request(&s);
        an_ensure_sees_the_field(&s, off);
        a_declared_error(&s);
        no_caller(&s);
    }
    println!("visibility http ok");
}
