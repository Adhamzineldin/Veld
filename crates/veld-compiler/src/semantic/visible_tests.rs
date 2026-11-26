//! `@visible(when: e)` (`spec/platform.md` section 2): `E1101`, `W1101`, the scope of the
//! expression and what it lowers to.

use crate::testing::{codes, ir as build, one};
use veld_ir::{Feature, Ir};

const USER: &str = "model User { id: uuid\n roles: string[] }\nprincipal User\n";

fn booking(extra: &str) -> String {
    format!(
        "{USER}model Booking {{\n  guestId: uuid\n  guestPhone?: string @visible(when: {extra})\n}}\n\
         @auth(bearer)\nservice Bookings at \"/b\" {{ get \"/{{id}}\" one(id: uuid) -> Booking }}\n"
    )
}

const PHONE: &str = "user.id == guestId || \"support\" in user.roles";

fn field<'a>(ir: &'a Ir, decl: &str, name: &str) -> &'a veld_ir::Field {
    ir.decls
        .iter()
        .find(|d| d.name == decl)
        .and_then(|d| d.fields().iter().find(|f| f.name == name))
        .unwrap()
}

#[test]
fn the_expression_lowers_to_the_field_and_uses_the_feature() {
    let ir = build(&booking(PHONE));
    assert!(field(&ir, "Booking", "guestPhone").visible.is_some());
    assert!(field(&ir, "Booking", "guestId").visible.is_none());
    let used = ir.features_used(&[ir.services[0].id]);
    assert!(used.contains(&Feature::Visibility), "{used:?}");
}

#[test]
fn it_may_read_now_and_the_caller_and_the_fields() {
    let src = booking("user.id == guestId && now > now");
    assert_eq!(codes(&src), Vec::<&str>::new());
}

#[test]
fn the_principal_may_be_renamed() {
    let src = "model User { id: uuid }\nprincipal User as caller\nmodel B { owner: uuid\n x?: string @visible(when: caller.id == owner) }\n\
         @auth(bearer)\nservice S at \"/s\" { get \"/\" one() -> B }\n";
    assert_eq!(codes(src), Vec::<&str>::new());
}

#[test]
fn a_field_that_is_not_optional_is_e1101() {
    let src =
        format!("{USER}model B {{ owner: uuid\n x: string @visible(when: user.id == owner) }}\n");
    assert_eq!(codes(&src), vec!["E1101"]);
}

#[test]
fn a_package_without_a_principal_is_e1101() {
    let src = "model B { owner: uuid\n x?: string @visible(when: owner == owner) }\n";
    assert_eq!(codes(src), vec!["E1101"]);
}

#[test]
fn an_event_field_and_an_event_that_reaches_one_are_e1101() {
    let src = format!(
        "{USER}model P {{ owner: uuid\n x?: string @visible(when: user.id == owner) }}\n\
         event Made {{ p: P }}\nevent Plain {{ n: int }}\nchannel C {{ carries Made, Plain }}\n"
    );
    assert!(codes(&src).contains(&"E1101"), "{:?}", codes(&src));
    let direct = format!(
        "{USER}event Made {{ x?: string @visible(when: user.id == user.id) }}\nchannel C {{ carries Made }}\n"
    );
    assert!(codes(&direct).contains(&"E1101"), "{:?}", codes(&direct));
}

#[test]
fn an_expression_that_does_not_type_is_reported_with_its_own_code() {
    assert_eq!(codes(&booking("guestId")), vec!["E0902"]);
    assert_eq!(codes(&booking("nobody == 1")), vec!["E0901"]);
    assert_eq!(codes(&booking("result == 1")), vec!["E0904"]);
}

#[test]
fn a_field_named_like_the_caller_is_e0920() {
    let src = format!("{USER}model B {{ user: string\n x?: string @visible(when: true) }}\n");
    assert_eq!(codes(&src), vec!["E0920"]);
}

#[test]
fn the_expression_must_guard_an_optional_field_it_reads() {
    let src =
        format!("{USER}model B {{ note?: string\n x?: string @visible(when: note == \"a\") }}\n");
    assert_eq!(codes(&src), vec!["E0903"]);
    let ok = format!(
        "{USER}model B {{ note?: string\n x?: string @visible(when: has(note) && note == \"a\") }}\n"
    );
    assert_eq!(codes(&ok), Vec::<&str>::new());
}

#[test]
fn another_argument_or_none_is_e0602() {
    for bad in [
        "@visible",
        "@visible(true)",
        "@visible(who: true)",
    ] {
        let src = format!("{USER}model B {{ x?: string {bad} }}\n");
        assert!(codes(&src).contains(&"E0602"), "{bad}: {:?}", codes(&src));
    }
    let on_model = format!("{USER}@visible(when: true)\nmodel B {{ x?: string }}\n");
    assert!(codes(&on_model).contains(&"E0602"));
}

#[test]
fn an_argument_after_the_expression_is_a_syntax_error() {
    let src = format!("{USER}model B {{ x?: string @visible(when: true, x: 1) }}\n");
    assert_eq!(codes(&src), vec!["E0003"]);
}

#[test]
fn an_endpoint_without_a_caller_warns_w1101() {
    let public = format!(
        "{USER}model B {{ owner: uuid\n x?: string @visible(when: user.id == owner) }}\n\
         @auth(none)\nservice S at \"/s\" {{ get \"/\" one() -> B }}\n"
    );
    assert_eq!(codes(&public), vec!["W1101"]);
    let authed = public.replace("@auth(none)", "@auth(bearer)");
    assert_eq!(codes(&authed), Vec::<&str>::new());
}

#[test]
fn an_inherited_field_keeps_the_expression() {
    let src = format!(
        "{}model Vip extends Booking {{ tier: int }}\n",
        booking(PHONE)
    );
    let ir = build(&src);
    assert!(field(&ir, "Vip", "guestPhone").visible.is_some());
}

#[test]
fn it_is_part_of_no_signature_and_a_contract_without_it_keeps_its_json() {
    let with = build(&booking(PHONE));
    let without = build(&booking(PHONE).replace(&format!(" @visible(when: {PHONE})"), ""));
    let sig = |ir: &Ir| {
        ir.decls
            .iter()
            .find(|d| d.name == "Booking")
            .unwrap()
            .sig
            .clone()
    };
    assert_eq!(sig(&with), sig(&without));
    assert!(
        !serde_json::to_string(&without)
            .unwrap()
            .contains("\"visible\"")
    );
    assert!(
        serde_json::to_string(&with)
            .unwrap()
            .contains("\"visible\"")
    );
}

#[test]
fn when_and_visible_stay_ordinary_names() {
    let src = "model A { when: string\n visible: bool }\n";
    assert!(one(src).ir.is_some());
}
