//! Examples (section 12): filling in, references, running, and pattern matching.

use serde_json::{Value as Json, json};
use veld_ir::{DeclId, EndpointRules, Example, Ir, Literal, ServiceId, Type};

use super::bookings::{Bookings, CLOCK_MS, bookings};
use super::kit::{
    Kit, boolean, bytes, date, datetime, dec, field, float, int, list_of, long, map_of, opt, sc,
    string, time, uuid,
};
use super::service_kit::{
    ExampleExt as _, endpoint, eref, example, expect_error, expect_ok, jv, obj, param, service,
};
use crate::{Difference, ExampleError, Expected, Verdict, diff_pattern, fill_example, run_example};

fn report(b: &Bookings, ex: &Example) -> crate::ExampleReport {
    run_example(&b.ir, ex).unwrap()
}

const U1: &str = "00000000-0000-4000-8000-000000000001";
const U2: &str = "00000000-0000-4000-8000-000000000002";

// ---- the examples of a bookings service ------------------------------------------------------

fn full_room(b: &Bookings, expected_capacity: i64) -> Example {
    example(
        "a full room is rejected",
        "createBooking",
        expect_error(
            &b.room_full,
            Some(obj(vec![("capacity", jv(json!(expected_capacity)))])),
        ),
    )
    .given(
        "room",
        Some(obj(vec![
            ("capacity", jv(json!(2))),
            ("closed", jv(json!(false))),
        ])),
    )
    .arg(
        "body",
        obj(vec![
            ("roomId", eref("room", &["id"])),
            ("guests", jv(json!(3))),
        ]),
    )
}

#[test]
fn a_full_room_example_passes_and_its_filled_in_request_is_exact() {
    let b = bookings();
    let ex = full_room(&b, 2);
    let r = report(&b, &ex);
    assert!(r.passed(), "{r}");
    assert_eq!(r.to_string(), "ok   a full room is rejected");

    let f = fill_example(&b.ir, &ex).unwrap();
    assert_eq!(f.clock, "2026-01-01T00:00:00Z");
    assert_eq!(f.request.now, CLOCK_MS);
    // the given is filled first: the uuid counter and the datetime counter both start there
    assert_eq!(
        f.request.facts["room"],
        Some(
            json!({ "id": U1, "capacity": 2, "closed": false, "openedAt": "2026-01-01T00:00:00Z" })
        )
    );
    // the ref reads the filled-in given; the body continues the datetime counter
    assert_eq!(
        f.request.params["body"],
        json!({ "roomId": U1, "guests": 3,
                "start": "2026-01-01T00:00:00.001Z", "end": "2026-01-01T00:00:00.002Z" })
    );
    assert_eq!(f.request.result, None);
    assert_eq!(f.request.principal, None);
}

#[test]
fn a_given_filled_before_the_body_advances_the_counters_the_body_sees() {
    let b = bookings();
    let alone = example("alone", "createBooking", expect_ok(None)).given("room", None);
    let f = fill_example(&b.ir, &alone).unwrap();
    // nothing filled before the body: it starts at the clock and at uuid 1
    assert_eq!(f.request.params["body"]["roomId"], U1);
    assert_eq!(f.request.params["body"]["start"], "2026-01-01T00:00:00Z");
    assert_eq!(f.request.params["body"]["end"], "2026-01-01T00:00:00.001Z");

    let with_given = example("with a given", "createBooking", expect_ok(None));
    let f = fill_example(&b.ir, &with_given).unwrap();
    // the (unmentioned, required) fact room is filled first: uuid 1 and the first datetime
    let room = f.request.facts["room"].clone().unwrap();
    assert_eq!(room["id"], U1);
    assert_eq!(room["openedAt"], "2026-01-01T00:00:00Z");
    assert_eq!(f.request.params["body"]["roomId"], U2);
    assert_eq!(
        f.request.params["body"]["start"],
        "2026-01-01T00:00:00.001Z"
    );
    assert_eq!(f.request.params["body"]["end"], "2026-01-01T00:00:00.002Z");
}

#[test]
fn two_left_out_datetimes_of_one_model_are_a_millisecond_apart_and_end_follows_start() {
    let b = bookings();
    let ex = example("x", "createBooking", expect_ok(None));
    let f = fill_example(&b.ir, &ex).unwrap();
    let body = &f.request.params["body"];
    assert_eq!(body["end"].as_str().unwrap(), "2026-01-01T00:00:00.002Z");
    assert!(body["end"].as_str() > body["start"].as_str());
}

#[test]
fn values_the_example_wrote_do_not_advance_the_counters() {
    let b = bookings();
    let ex = example("x", "createBooking", expect_ok(None))
        .given(
            "room",
            Some(obj(vec![
                ("id", jv(json!("22222222-2222-4222-8222-222222222222"))),
                ("openedAt", jv(json!("2020-05-05T05:05:05Z"))),
            ])),
        )
        .at("2026-06-01T00:00:00Z")
        .arg(
            "body",
            obj(vec![("start", jv(json!("2026-01-01T00:00:00Z")))]),
        );
    let f = fill_example(&b.ir, &ex).unwrap();
    // the room's id and openedAt were written, so the body is the first to use each counter
    assert_eq!(f.request.params["body"]["roomId"], U1);
    assert_eq!(f.request.params["body"]["end"], "2026-06-01T00:00:00Z");
}

#[test]
fn the_other_examples_of_the_service_pass() {
    let b = bookings();
    let closed = example(
        "closed",
        "createBooking",
        expect_error(&b.room_closed, None),
    )
    .given("room", Some(obj(vec![("closed", jv(json!(true)))])));
    let r = report(&b, &closed);
    assert!(r.passed(), "{r}");

    let missing = example(
        "a missing room is not found",
        "createBooking",
        expect_error(
            &b.not_found,
            Some(obj(vec![("resource", jv(json!("room")))])),
        ),
    )
    .given("room", None)
    .arg("body", obj(vec![("guests", jv(json!(1)))]));
    assert!(report(&b, &missing).passed());

    let ok = example(
        "the booking starts as pending",
        "createBooking",
        expect_ok(Some(obj(vec![("status", jv(json!("pending")))]))),
    )
    .given(
        "room",
        Some(obj(vec![
            ("capacity", jv(json!(5))),
            ("closed", jv(json!(false))),
        ])),
    )
    .arg("body", obj(vec![("guests", jv(json!(2)))]))
    .returning(obj(vec![
        ("status", jv(json!("pending"))),
        ("guests", jv(json!(2))),
    ]));
    let r = report(&b, &ok);
    assert!(r.passed(), "{r}");

    let wrong = example(
        "paid result",
        "createBooking",
        expect_error(&b.internal, None),
    )
    .given(
        "room",
        Some(obj(vec![
            ("capacity", jv(json!(5))),
            ("closed", jv(json!(false))),
        ])),
    )
    .returning(obj(vec![("status", jv(json!("paid")))]));
    assert!(report(&b, &wrong).passed());

    let late = example(
        "starts in the past",
        "createBooking",
        expect_error(&b.validation, None),
    )
    .at("2026-06-01T00:00:00Z")
    .given("room", Some(obj(vec![("capacity", jv(json!(5)))])))
    .arg(
        "body",
        obj(vec![("start", jv(json!("2026-01-01T00:00:00Z")))]),
    );
    // both datetimes: start written, end filled from the clock (June), so end > start holds
    assert!(report(&b, &late).passed());
}

#[test]
fn an_ok_pattern_without_returns_is_skipped_offline() {
    let b = bookings();
    let ex = example(
        "x",
        "createBooking",
        expect_ok(Some(obj(vec![("status", jv(json!("pending")))]))),
    )
    .given("room", Some(obj(vec![("capacity", jv(json!(5)))])));
    let r = report(&b, &ex);
    assert!(matches!(r.verdict, Verdict::Skipped(_)), "{r}");
    assert!(!r.passed());
}

#[test]
fn a_failing_example_prints_the_expected_and_the_actual_outcome() {
    let b = bookings();
    let r = report(&b, &full_room(&b, 3));
    let Verdict::Failed(m) = &r.verdict else {
        panic!("expected a failure: {r}");
    };
    assert_eq!(m.expected, r#"RoomFull {"capacity":3}"#);
    assert!(
        m.actual
            .starts_with(r#"RoomFull 409 {"capacity":2} violations [require:"#),
        "{}",
        m.actual
    );
    assert_eq!(
        m.differences,
        [Difference {
            path: "$.capacity".to_owned(),
            expected: "3".to_owned(),
            actual: "2".to_owned()
        }]
    );
    let text = r.to_string();
    assert!(
        text.starts_with("FAIL a full room is rejected\nexpected: RoomFull"),
        "{text}"
    );
    assert!(
        text.contains("\n  at $.capacity: expected 3, actual 2"),
        "{text}"
    );

    // the wrong error altogether
    let wrong = example("w", "createBooking", expect_error(&b.room_closed, None))
        .given(
            "room",
            Some(obj(vec![
                ("capacity", jv(json!(2))),
                ("closed", jv(json!(false))),
            ])),
        )
        .arg("body", obj(vec![("guests", jv(json!(3)))]));
    let r = report(&b, &wrong);
    let Verdict::Failed(m) = &r.verdict else {
        panic!("{r}")
    };
    assert_eq!(m.expected, "RoomClosed");
    assert!(m.actual.starts_with("RoomFull 409"));
}

#[test]
fn a_deliberate_violation_in_a_written_value_is_a_validation_error_not_e0953() {
    let b = bookings();
    let ex = example(
        "end before start",
        "createBooking",
        expect_error(
            &b.validation,
            Some(obj(vec![(
                "issues",
                ExampleList::of(vec![obj(vec![
                    ("path", jv(json!("body"))),
                    ("code", jv(json!("rule"))),
                ])]),
            )])),
        ),
    )
    .arg(
        "body",
        obj(vec![
            ("start", jv(json!("2026-02-02T00:00:00Z"))),
            ("end", jv(json!("2026-02-01T00:00:00Z"))),
        ]),
    );
    let r = report(&b, &ex);
    assert!(r.passed(), "{r}");
}

struct ExampleList;
impl ExampleList {
    fn of(items: Vec<veld_ir::ExampleValue>) -> veld_ir::ExampleValue {
        veld_ir::ExampleValue::List(items)
    }
}

// ---- E0953 -----------------------------------------------------------------------------------

#[test]
fn an_invariant_that_reads_a_filled_in_field_is_e0953() {
    let b = bookings();
    // start is written, end is filled in from the clock, so end > start is violated
    let ex = example("x", "createBooking", expect_ok(None)).arg(
        "body",
        obj(vec![("start", jv(json!("2030-01-01T00:00:00Z")))]),
    );
    let err = run_example(&b.ir, &ex).unwrap_err();
    assert_eq!(err.code(), "E0953");
    let ExampleError::BreaksInvariant {
        rule,
        fields,
        example,
        ..
    } = &err
    else {
        panic!("{err:?}");
    };
    assert_eq!(rule, "endAfterStart");
    assert_eq!(fields, &["body.end".to_owned()]);
    assert_eq!(example, "x");
    assert!(err.to_string().contains("give body.end"), "{err}");
}

#[test]
fn a_filled_in_result_is_checked_too() {
    let b = bookings();
    // Booking.guests has @example 1, so a result with only a status is valid
    let ok = example("x", "createBooking", expect_ok(None))
        .returning(obj(vec![("status", jv(json!("pending")))]));
    assert!(fill_example(&b.ir, &ok).is_ok());
}

fn signup_ir(pattern: bool, example_value: bool) -> Ir {
    let mut k = Kit::new();
    let mut email = field("email", string());
    if pattern {
        email.constraints.pattern = Some("^[a-z]+@[a-z]+$".to_owned());
    }
    if example_value {
        email.example = Some(Literal::String("a@b".to_owned()));
    }
    let model = k.model("Signup", vec![email], vec![]);
    let ep = endpoint(
        "signup",
        vec![param("body", veld_ir::Binding::Body, model)],
        None,
        &[],
        EndpointRules::default(),
    );
    k.ir_with(service(vec![ep]))
}

#[test]
fn a_pattern_or_format_without_an_example_cannot_be_filled_in() {
    let ex = example("signs up", "signup", expect_ok(None));
    let ir = signup_ir(true, false);
    let err = fill_example(&ir, &ex).unwrap_err();
    assert_eq!(err.code(), "E0953");
    assert!(
        matches!(&err, ExampleError::CannotFill { path, .. } if path == "$.email"),
        "{err:?}"
    );
    assert!(err.to_string().contains("@pattern"), "{err}");

    // an @example makes it fillable; so does writing it
    let ir = signup_ir(true, true);
    let f = fill_example(&ir, &ex).unwrap();
    assert_eq!(f.request.params["body"], json!({ "email": "a@b" }));
    let ir = signup_ir(true, false);
    let written =
        example("w", "signup", expect_ok(None)).arg("body", obj(vec![("email", jv(json!("x@y")))]));
    assert!(fill_example(&ir, &written).is_ok());

    // a @format is the same
    let mut k = Kit::new();
    let mut f = field("email", string());
    f.constraints.format = Some(veld_ir::StringFormat::Email);
    let model = k.model("M", vec![f], vec![]);
    let ep = endpoint(
        "signup",
        vec![param("body", veld_ir::Binding::Body, model)],
        None,
        &[],
        EndpointRules::default(),
    );
    let ir = k.ir_with(service(vec![ep]));
    assert_eq!(fill_example(&ir, &ex).unwrap_err().code(), "E0953");
}

#[test]
fn a_type_that_never_ends_is_an_error_not_a_stack_overflow() {
    let mut k = Kit::new();
    let a = k.model("A", vec![field("u", Type::Decl(DeclId(1)))], vec![]);
    let u = k.union("U", "kind", &[("a", &a)]);
    let ep = endpoint(
        "go",
        vec![param("body", veld_ir::Binding::Body, u)],
        None,
        &[],
        EndpointRules::default(),
    );
    let ir = k.ir_with(service(vec![ep]));
    let err = fill_example(&ir, &example("loop", "go", expect_ok(None))).unwrap_err();
    assert_eq!(err.code(), "E0953");
}

// ---- the table of 12.2 -----------------------------------------------------------------------

fn everything_ir() -> Ir {
    let mut k = Kit::new();
    let color = k.enum_("Color", &[("red", "red"), ("green", "green")]);
    let inner = k.model("Inner", vec![field("n", int())], vec![]);
    let circle = k.model("Circle", vec![field("r", int())], vec![]);
    let square = k.model("Square", vec![field("side", int())], vec![]);
    let shape = k.union("Shape", "kind", &[("circle", &circle), ("square", &square)]);
    let with = |name: &str, ty: Type, f: &dyn Fn(&mut veld_ir::Constraints)| {
        let mut fl = field(name, ty);
        f(&mut fl.constraints);
        fl
    };
    let mut ex = field("ex", string());
    ex.example = Some(Literal::String("hello".to_owned()));
    let mut df = field("df", int());
    df.default = Some(Literal::Int(7));
    let mut opt_ex = opt(field("optEx", string()));
    opt_ex.example = Some(Literal::String("given".to_owned()));
    let fields = vec![
        field("s", string()),
        with("s3", string(), &|c| c.min_len = Some(3)),
        field("i", int()),
        with("imin", int(), &|c| c.min = Some(5.0)),
        with("imax", int(), &|c| c.max = Some(-3.0)),
        field("l", long()),
        with("lmin", long(), &|c| c.min = Some(12.0)),
        field("f", float()),
        with("dmin", dec(), &|c| c.min = Some(0.5)),
        field("b", boolean()),
        field("u1", uuid()),
        field("u2", uuid()),
        field("dt1", datetime()),
        field("dt2", datetime()),
        field("d1", date()),
        field("d2", date()),
        field("t", time()),
        field("by", bytes()),
        field("li", list_of(int())),
        with("lmin2", list_of(int()), &|c| c.min_len = Some(2)),
        field("mp", map_of(int())),
        field("j", sc(veld_ir::Scalar::Json)),
        field("e", color),
        opt(field("o", string())),
        ex,
        df,
        opt_ex,
        field("nested", inner),
        field("un", shape),
        field("ids", list_of(uuid())),
    ];
    let model = k.model("Everything", fields, vec![]);
    let ep = endpoint(
        "fillAll",
        vec![param("body", veld_ir::Binding::Body, model)],
        None,
        &[],
        EndpointRules::default(),
    );
    k.ir_with(service(vec![ep]))
}

#[test]
fn every_row_of_the_filling_table() {
    let ir = everything_ir();
    let ex = example("table", "fillAll", expect_ok(None));
    let f = fill_example(&ir, &ex).unwrap();
    let expected = json!({
        "s": "a", "s3": "aaa",
        "i": 0, "imin": 5, "imax": -3,
        "l": "0", "lmin": "12",
        "f": 0.0, "dmin": "0.5",
        "b": false,
        "u1": U1, "u2": U2,
        "dt1": "2026-01-01T00:00:00Z", "dt2": "2026-01-01T00:00:00.001Z",
        "d1": "2026-01-01", "d2": "2026-01-02",
        "t": "00:00:00",
        "by": "",
        "li": [], "lmin2": [0, 0],
        "mp": {}, "j": {},
        "e": "red",
        "ex": "hello", "df": 7, "optEx": "given",
        "nested": { "n": 0 },
        "un": { "kind": "circle", "r": 0 },
        "ids": [],
    });
    assert_eq!(f.request.params["body"], expected);
    // an optional field with no @example or default stays absent
    assert!(f.request.params["body"].get("o").is_none());
}

#[test]
fn the_clock_sets_dates_and_times_and_the_filling_is_repeatable() {
    let ir = everything_ir();
    let ex = example("table", "fillAll", expect_ok(None)).at("2026-03-01T09:30:15.250+02:00");
    let a = fill_example(&ir, &ex).unwrap();
    let body = &a.request.params["body"];
    // 07:30:15.250 UTC
    assert_eq!(body["t"], "07:30:15.250");
    assert_eq!(body["dt1"], "2026-03-01T07:30:15.250Z");
    assert_eq!(body["dt2"], "2026-03-01T07:30:15.251Z");
    assert_eq!(body["d1"], "2026-03-01");
    assert_eq!(body["d2"], "2026-03-02");
    assert_eq!(a, fill_example(&ir, &ex).unwrap());
}

#[test]
fn written_values_are_completed_inside_nested_values_and_lists() {
    let mut k = Kit::new();
    let item = k.model(
        "Item",
        vec![field("id", uuid()), field("qty", int())],
        vec![],
    );
    let order = k.model("Order", vec![field("items", list_of(item))], vec![]);
    let ep = endpoint(
        "place",
        vec![param("body", veld_ir::Binding::Body, order)],
        None,
        &[],
        EndpointRules::default(),
    );
    let ir = k.ir_with(service(vec![ep]));
    let ex = example("x", "place", expect_ok(None)).arg(
        "body",
        obj(vec![(
            "items",
            ExampleList::of(vec![obj(vec![("qty", jv(json!(4)))]), obj(vec![])]),
        )]),
    );
    let f = fill_example(&ir, &ex).unwrap();
    assert_eq!(
        f.request.params["body"],
        json!({ "items": [ { "qty": 4, "id": U1 }, { "qty": 0, "id": U2 } ] })
    );
}

// ---- references and malformed examples -------------------------------------------------------

#[test]
fn refs_read_the_filled_in_givens_and_a_bad_ref_is_an_error() {
    let b = bookings();
    let ex = example("x", "createBooking", expect_ok(None))
        .given("room", Some(obj(vec![("capacity", jv(json!(8)))])))
        .arg(
            "body",
            obj(vec![
                ("roomId", eref("room", &["id"])),
                ("guests", eref("room", &["capacity"])),
            ]),
        );
    let f = fill_example(&b.ir, &ex).unwrap();
    assert_eq!(f.request.params["body"]["roomId"], U1);
    assert_eq!(f.request.params["body"]["guests"], 8);

    for (given, path) in [("room", vec!["nope"]), ("ghost", vec!["id"])] {
        let bad = example("x", "createBooking", expect_ok(None))
            .arg("body", obj(vec![("roomId", eref(given, &path))]));
        let err = fill_example(&b.ir, &bad).unwrap_err();
        assert!(matches!(err, ExampleError::BadRef { .. }), "{err:?}");
        assert_eq!(err.code(), "E0952");
    }
    // a ref to an absent given
    let absent = example("x", "createBooking", expect_ok(None))
        .given("room", None)
        .arg("body", obj(vec![("roomId", eref("room", &["id"]))]));
    assert!(matches!(
        fill_example(&b.ir, &absent).unwrap_err(),
        ExampleError::BadRef { .. }
    ));
}

#[test]
fn malformed_examples_are_errors_and_never_panics() {
    let b = bookings();
    let base = || example("x", "createBooking", expect_ok(None));
    let mut unknown_service = base();
    unknown_service.service = ServiceId(9);
    let mut unknown_endpoint = base();
    unknown_endpoint.endpoint = "zzz".to_owned();
    let cases: Vec<(Example, &str)> = vec![
        (unknown_service, "E0950"),
        (unknown_endpoint, "E0950"),
        (base().at("yesterday"), "E0952"),
        (base().given("nope", None), "E0951"),
        (base().arg("nope", jv(json!(1))), "E0952"),
        (
            base().returning(jv(json!({}))).arg("body", jv(json!(5))),
            "E0952",
        ),
        (base().arg("body", jv(json!(5))), "E0952"),
        (base().arg("body", jv(json!([1, 2]))), "E0952"),
        (base().arg("body", jv(json!({ "guests": "many" }))), "E0952"),
    ];
    for (ex, code) in cases {
        let err = run_example(&b.ir, &ex).unwrap_err();
        assert_eq!(err.code(), code, "{err}");
    }
    // returns on an endpoint with no success body
    let none = example("x", "noBody", expect_ok(None)).returning(jv(json!({})));
    let err = run_example(&b.ir, &none).unwrap_err();
    assert!(
        matches!(err, ExampleError::UnexpectedReturns { .. }),
        "{err:?}"
    );
    // junk in the result and the givens must not panic either
    for junk in [
        json!(null),
        json!(7),
        json!("s"),
        json!([]),
        json!({ "status": 1 }),
    ] {
        let ex = base()
            .returning(jv(junk.clone()))
            .given("room", Some(jv(junk)));
        let _ = run_example(&b.ir, &ex);
    }
}

// ---- pattern matching ------------------------------------------------------------------------

fn money_ir() -> (Ir, Type) {
    let mut k = Kit::new();
    let money = k.model(
        "Money",
        vec![field("amount", dec()), field("cur", string())],
        vec![],
    );
    let line = k.model(
        "Line",
        vec![
            field("money", money),
            field("tags", list_of(string())),
            field("at", datetime()),
            field("id", uuid()),
            opt(field("note", string())),
        ],
        vec![],
    );
    (k.ir(), line)
}

fn actual_line() -> Json {
    json!({
        "money": { "amount": "2.00", "cur": "EUR" },
        "tags": ["a", "b"],
        "at": "2026-01-01T00:00:00.000Z",
        "id": "AAAAAAAA-0000-4000-8000-000000000001",
    })
}

#[test]
fn patterns_compare_by_the_value_semantics() {
    let (ir, line) = money_ir();
    let diffs = |pat: Json| diff_pattern(&ir, &line, &pat, &actual_line());
    // a decimal across scales, a nested object with unnamed fields ignored
    assert!(diffs(json!({ "money": { "amount": "2" } })).is_empty());
    assert!(diffs(json!({ "money": { "amount": "2.0000" }, "tags": ["a", "b"] })).is_empty());
    // an instant with another offset and extra digits, and a uuid in upper case
    assert!(diffs(json!({ "at": "2026-01-01T02:00:00.000999+02:00" })).is_empty());
    assert!(diffs(json!({ "id": "aaaaaaaa-0000-4000-8000-000000000001" })).is_empty());
    // the empty pattern matches everything
    assert!(diffs(json!({})).is_empty());
}

#[test]
fn patterns_report_where_they_differ() {
    let (ir, line) = money_ir();
    let d = diff_pattern(
        &ir,
        &line,
        &json!({ "money": { "amount": "2.01" } }),
        &actual_line(),
    );
    assert_eq!(
        d,
        [Difference {
            path: "$.money.amount".to_owned(),
            expected: "\"2.01\"".to_owned(),
            actual: "\"2.00\"".to_owned()
        }]
    );
    // lists match in length and order
    let d = diff_pattern(&ir, &line, &json!({ "tags": ["a"] }), &actual_line());
    assert_eq!(d[0].path, "$.tags.length");
    let d = diff_pattern(&ir, &line, &json!({ "tags": ["b", "a"] }), &actual_line());
    assert_eq!(
        d.iter().map(|d| d.path.as_str()).collect::<Vec<_>>(),
        ["$.tags[0]", "$.tags[1]"]
    );
    // a named field the actual value lacks
    let d = diff_pattern(&ir, &line, &json!({ "note": "x" }), &actual_line());
    assert_eq!(d[0].actual, "missing");
    // wrong shapes are differences, not panics
    for pat in [
        json!(1),
        json!([]),
        json!({ "money": 3 }),
        json!({ "at": 5 }),
    ] {
        assert!(!diff_pattern(&ir, &line, &pat, &actual_line()).is_empty());
    }
    assert!(!diff_pattern(&ir, &line, &json!({}), &json!(null)).is_empty());
}

#[test]
fn expected_is_a_plain_value_of_the_example() {
    let b = bookings();
    let ex = full_room(&b, 2);
    let f = fill_example(&b.ir, &ex).unwrap();
    assert_eq!(
        f.expected,
        Expected::Error {
            decl: b.id(&b.room_full),
            pattern: Some(json!({ "capacity": 2 }))
        }
    );
    assert_eq!(f.name, "a full room is rejected");
    assert_eq!(f.endpoint, "createBooking");
}
