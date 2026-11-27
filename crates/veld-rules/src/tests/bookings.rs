//! A small bookings service that follows the example of `spec/rules.md` section 9, built by hand.

use serde_json::{Value as Json, json};
use veld_ir::{EndpointRules, Ir, Literal, Root, Type};

use super::kit::{
    Kit, boolean, datetime, div, eq, field, ge, gt, i, implies, int, le, list_of, not, now, opt, p,
    string, sub, uuid, variant,
};
use super::service_kit::{
    decl_id, else_err, endpoint, ensure, fact, hasr, old, param, require, rp, service,
};
use crate::pipeline::Request;

pub const CLOCK: &str = "2026-01-01T00:00:00Z";
pub const CLOCK_MS: i64 = 1_767_225_600_000;
pub const ROOM_ID: &str = "11111111-1111-4111-8111-111111111111";

pub struct Bookings {
    pub ir: Ir,
    pub room_full: Type,
    pub room_closed: Type,
    pub not_found: Type,
    pub internal: Type,
    pub validation: Type,
}

pub fn bookings() -> Bookings {
    let mut k = Kit::new();
    let status = k.enum_("BookingStatus", &[("pending", "pending"), ("paid", "paid")]);
    let room = k.model(
        "Room",
        vec![
            field("id", uuid()),
            field("capacity", int()),
            field("closed", boolean()),
            field("openedAt", datetime()),
        ],
        vec![],
    );
    let coupon = k.model("Coupon", vec![field("percent", int())], vec![]);
    let mut guests = field("guests", int());
    guests.constraints.min = Some(1.0);
    let new_booking = k.model(
        "NewBooking",
        vec![
            field("roomId", uuid()),
            guests,
            field("start", datetime()),
            field("end", datetime()),
            opt(field("couponId", uuid())),
        ],
        vec![(
            "endAfterStart".to_owned(),
            gt(p("end", datetime()), p("start", datetime())),
        )],
    );
    let mut booking_guests = field("guests", int());
    booking_guests.example = Some(Literal::Int(1));
    let booking = k.model(
        "Booking",
        vec![
            field("id", uuid()),
            field("status", status.clone()),
            booking_guests,
        ],
        vec![("guestsPositive".to_owned(), gt(p("guests", int()), i(0)))],
    );
    let issue = k.model(
        "ValidationIssue",
        vec![
            field("path", string()),
            field("code", string()),
            field("message", string()),
        ],
        vec![],
    );
    let validation = k.error(
        "ValidationError",
        vec![field("issues", list_of(issue))],
        vec![],
    );
    let room_full = k.error("RoomFull", vec![field("capacity", int())], vec![]);
    let room_closed = k.error("RoomClosed", vec![], vec![]);
    let not_found = k.error("NotFound", vec![opt(field("resource", string()))], vec![]);
    let internal = k.error("Internal", vec![opt(field("message", string()))], vec![]);

    let errors = [
        (409, &room_full),
        (409, &room_closed),
        (422, &validation),
        (404, &not_found),
        (500, &internal),
    ];
    let create = endpoint(
        "createBooking",
        vec![param("body", veld_ir::Binding::Body, new_booking.clone())],
        Some(booking.clone()),
        &errors,
        create_rules(
            &room,
            &coupon,
            &room_full,
            &room_closed,
            &status,
            &new_booking,
        ),
    );
    let risky = endpoint(
        "risky",
        vec![param("body", veld_ir::Binding::Body, new_booking.clone())],
        Some(booking.clone()),
        &errors,
        risky_rules(&new_booking),
    );
    let plain = endpoint("noBody", vec![], None, &errors, EndpointRules::default());
    let ir = k.ir_with(service(vec![create, risky, plain]));
    Bookings {
        ir,
        room_full,
        room_closed,
        not_found,
        internal,
        validation,
    }
}

fn body_path(steps: &[(&str, bool)], ty: Type) -> veld_ir::Expr {
    rp(Root::Param("body".to_owned()), steps, ty)
}

fn create_rules(
    room: &Type,
    coupon: &Type,
    room_full: &Type,
    room_closed: &Type,
    status: &Type,
    new_booking: &Type,
) -> EndpointRules {
    let _ = new_booking;
    let fact_path =
        |name: &str, field: &str, ty: Type| rp(Root::Fact(name.to_owned()), &[(field, false)], ty);
    let capacity = || fact_path("room", "capacity", int());
    let guests = || body_path(&[("guests", false)], int());
    let facts = vec![
        fact(
            "room",
            room.clone(),
            false,
            Some(body_path(&[("roomId", false)], uuid())),
        ),
        fact(
            "coupon",
            coupon.clone(),
            true,
            Some(body_path(&[("couponId", true)], uuid())),
        ),
    ];
    let requires = vec![
        require(
            None,
            not(fact_path("room", "closed", boolean())),
            &[],
            Some(else_err(room_closed, 409, vec![])),
            None,
        ),
        require(
            None,
            le(guests(), capacity()),
            &["body"],
            Some(else_err(room_full, 409, vec![("capacity", capacity())])),
            None,
        ),
        require(
            Some("notInPast"),
            ge(body_path(&[("start", false)], datetime()), now()),
            &["body"],
            None,
            Some("A booking cannot start in the past"),
        ),
        require(
            Some("couponLimit"),
            implies(
                hasr(Root::Fact("coupon".to_owned()), &[]),
                le(fact_path("coupon", "percent", int()), i(50)),
            ),
            &[],
            None,
            None,
        ),
    ];
    let result = |f: &str, ty: Type| rp(Root::Result, &[(f, false)], ty);
    let ensures = vec![
        ensure(
            None,
            eq(
                result("status", status.clone()),
                variant(status, "pending", "pending"),
            ),
            &[],
        ),
        ensure(
            Some("guestsKept"),
            eq(result("guests", int()), guests()),
            &["body"],
        ),
        ensure(
            Some("seatsLeft"),
            ge(sub(old(capacity()), result("guests", int())), i(0)),
            &[],
        ),
    ];
    EndpointRules {
        facts,
        transition: None,
        requires,
        ensures,
        allows: Vec::new(),
        version: None,
    }
}

fn risky_rules(new_booking: &Type) -> EndpointRules {
    let _ = new_booking;
    let g = || body_path(&[("guests", false)], int());
    let zero = sub(g(), g());
    EndpointRules {
        requires: vec![require(
            Some("ratio"),
            gt(div(i(10), zero), i(0)),
            &["body"],
            None,
            None,
        )],
        ..EndpointRules::default()
    }
}

impl Bookings {
    pub fn room_json(&self) -> Json {
        json!({ "id": ROOM_ID, "capacity": 5, "closed": false, "openedAt": CLOCK })
    }

    pub fn body_json(&self) -> Json {
        json!({
            "roomId": ROOM_ID, "guests": 2,
            "start": "2026-01-02T00:00:00Z", "end": "2026-01-03T00:00:00Z"
        })
    }

    /// A request that passes every step, with no result.
    pub fn request(&self) -> Request {
        let mut r = Request {
            now: CLOCK_MS,
            ..Request::default()
        };
        r.params.insert("body".to_owned(), self.body_json());
        r.facts.insert("room".to_owned(), Some(self.room_json()));
        r
    }

    pub fn result_json(&self, status: &str, guests: i64) -> Json {
        json!({ "id": ROOM_ID, "status": status, "guests": guests })
    }

    pub fn service(&self) -> &veld_ir::Service {
        &self.ir.services[0]
    }

    pub fn id(&self, t: &Type) -> veld_ir::DeclId {
        decl_id(t)
    }
}
