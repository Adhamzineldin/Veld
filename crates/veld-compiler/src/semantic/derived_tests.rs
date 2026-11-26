//! Derived fields (`spec/rules.md` section 17.2): scope, typing, cycles, shape, lowering to the IR,
//! identity, the rules digest and the features.

use crate::testing::{codes, ir, one};
use veld_ir::{DeclKind, Feature, Field, Ir};

const ORDER: &str = "\
model Line { price: decimal, qty: int, total: decimal = price * qty }
model Order {
  items: Line[] @length(max: 10)
  discount: decimal = 0
  subtotal: decimal = sum(items, i => i.total)
  total: decimal = subtotal - discount
  rule fits: discount <= subtotal
}
service S at \"/s\" { post \"/\" f(body: Order) -> 201 Order }
";

fn field<'a>(ir: &'a Ir, decl: &str, name: &str) -> &'a Field {
    let found = ir
        .decls
        .iter()
        .filter(|d| d.name == decl)
        .flat_map(|d| d.fields())
        .find(|f| f.name == name);
    match found {
        Some(f) => f,
        None => panic!("no field {decl}.{name}"),
    }
}

// ---- the IR ----------------------------------------------------------------------------------

#[test]
fn a_derived_field_carries_its_expression_and_its_id_and_no_default() {
    let ir = ir(ORDER);
    let f = field(&ir, "Order", "total");
    assert!(f.derived.is_some() && f.default.is_none() && !f.optional);
    let id = f.derived_id.clone().unwrap_or_default();
    assert!(id.starts_with("veld:app/Order.total@"), "{id}");
    assert_eq!(id.len(), "veld:app/Order.total@".len() + 16);
    assert!(field(&ir, "Order", "discount").default.is_some());
    assert!(field(&ir, "Order", "discount").derived.is_none());
}

#[test]
fn the_expression_reads_the_other_fields_by_name() {
    let ir = ir(ORDER);
    let f = field(&ir, "Order", "total");
    assert_eq!(f.derived_reads(), ["subtotal", "discount"]);
}

#[test]
fn a_contract_without_derived_fields_has_neither_key() {
    let ir = ir("model M { a: int = 1, b: string }\nservice S at \"/s\" { get \"/\" f() -> M }");
    let json = serde_json::to_string(&ir).unwrap_or_default();
    assert!(!json.contains("derived"), "{json}");
}

#[test]
fn the_id_ignores_the_message_formatting_and_the_source_name() {
    let a = ir(
        "model M { a: int @wire(\"A\")\n b: int = a + 1 }\nservice S at \"/s\" { get \"/\" f() -> M }",
    );
    let b = ir(
        "model M { x: int @wire(\"A\")\n b: int = x +   1 }\nservice S at \"/s\" { get \"/\" f() -> M }",
    );
    assert_eq!(
        field(&a, "M", "b").derived_id,
        field(&b, "M", "b").derived_id
    );
    let c = ir(
        "model M { a: int @wire(\"A\")\n b: int = a + 2 }\nservice S at \"/s\" { get \"/\" f() -> M }",
    );
    assert_ne!(
        field(&a, "M", "b").derived_id,
        field(&c, "M", "b").derived_id
    );
}

#[test]
fn an_inherited_derived_field_keeps_the_id_of_its_parent() {
    let ir = ir(
        "model P { a: int, b: int = a + 1 }\nmodel C extends P { c: int = b + 1 }\nservice S at \"/s\" { get \"/\" f() -> C }",
    );
    let (p, c) = (field(&ir, "P", "b"), field(&ir, "C", "b"));
    assert_eq!(p.derived_id, c.derived_id);
    assert!(c.inherited_from.is_some());
    assert!(field(&ir, "C", "c").derived_id.is_some());
}

#[test]
fn an_error_and_an_event_may_have_derived_fields() {
    let ir = ir(
        "error E 422 { a: int, b: int = a + 1 }\nservice S at \"/s\" { get \"/\" f() -> 200 | E }",
    );
    assert!(field(&ir, "E", "b").derived.is_some());
    assert!(matches!(
        ir.decls.iter().find(|d| d.name == "E").map(|d| &d.kind),
        Some(DeclKind::Error(_))
    ));
}

// ---- the signature, the digest and the features ------------------------------------------------

#[test]
fn a_derived_expression_is_not_part_of_the_declaration_signature() {
    let plain = ir("model M { a: int, b: int }\nservice S at \"/s\" { get \"/\" f() -> M }");
    let derived =
        ir("model M { a: int, b: int = a + 1 }\nservice S at \"/s\" { get \"/\" f() -> M }");
    let other =
        ir("model M { a: int, b: int = a + 2 }\nservice S at \"/s\" { get \"/\" f() -> M }");
    let sig = |ir: &Ir| {
        ir.decls
            .iter()
            .find(|d| d.name == "M")
            .map(|d| d.sig.clone())
    };
    assert_eq!(sig(&plain), sig(&derived));
    assert_eq!(sig(&derived), sig(&other));
}

#[test]
fn the_rules_digest_covers_the_derived_fields_a_service_reaches() {
    let digest = |src: &str| ir(src).services[0].rules_digest.clone();
    let plain = digest("model M { a: int, b: int }\nservice S at \"/s\" { get \"/\" f() -> M }");
    assert_eq!(plain, None);
    let one =
        digest("model M { a: int, b: int = a + 1 }\nservice S at \"/s\" { get \"/\" f() -> M }");
    let two =
        digest("model M { a: int, b: int = a + 2 }\nservice S at \"/s\" { get \"/\" f() -> M }");
    assert!(one.is_some() && two.is_some());
    assert_ne!(one, two);
}

#[test]
fn the_digest_of_a_contract_with_invariants_only_does_not_change() {
    let with = |extra: &str| {
        ir(&format!(
            "model M {{ a: int\n rule r: a > 0 }}\n{extra}\nservice S at \"/s\" {{ get \"/\" f() -> M }}"
        ))
        .services[0]
            .rules_digest
            .clone()
    };
    assert_eq!(with(""), with("model Unreached { a: int, b: int = a + 1 }"));
}

#[test]
fn a_derived_field_uses_rules_derived_and_only_that() {
    let ir = ir(ORDER);
    let used = ir.features_used(&[veld_ir::ServiceId(0)]);
    assert!(used.contains(&Feature::RulesDerived), "{used:?}");
    let plain =
        crate::testing::ir("model M { a: int }\nservice S at \"/s\" { get \"/\" f() -> M }");
    assert!(
        !plain
            .features_used(&[veld_ir::ServiceId(0)])
            .contains(&Feature::RulesDerived)
    );
}

#[test]
fn a_derived_field_on_a_fact_only_model_uses_the_feature() {
    let ir = ir(
        "model Room { id: uuid, seats: int, free: int = seats - 1 }\n\
model Booking { id: uuid }\n\
service S at \"/s\" { get \"/{id}\" f(id: uuid) -> Booking { fact room: Room = id } }",
    );
    let used = ir.features_used(&[veld_ir::ServiceId(0)]);
    assert!(used.contains(&Feature::RulesDerived), "{used:?}");
}

// ---- the checks --------------------------------------------------------------------------------

#[test]
fn a_type_that_does_not_fit_is_e0902() {
    assert_eq!(codes("model M { a: int, b: string = a }"), ["E0902"]);
    assert_eq!(codes("model M { a: long, b: int = a }"), ["E0902"]);
}

#[test]
fn widening_is_exact_so_int_fits_long_and_decimal() {
    assert!(codes("model M { a: int, b: long = a, c: decimal = a, d: float = a }").is_empty());
}

#[test]
fn a_cycle_is_e0960_once_per_cycle() {
    assert_eq!(
        codes("model M { a: int = b, b: int = a, c: int = c, d: int = a }"),
        ["E0960", "E0960"]
    );
}

#[test]
fn a_chain_in_any_order_is_not_a_cycle() {
    assert!(codes("model M { a: int = b + 1, b: int = c + 1, c: int = 1 + 1 }").is_empty());
}

#[test]
fn the_shape_of_a_derived_field_is_e0961() {
    assert_eq!(codes("model M { a: int, b?: int = a }"), ["E0961"]);
    assert_eq!(
        codes("model M { a: int\n @readonly\n b: int = a }"),
        ["E0961"]
    );
    assert_eq!(
        codes("model M { a: int\n @example(1)\n b: int = a }"),
        ["E0961"]
    );
}

#[test]
fn reading_an_optional_field_needs_a_guard() {
    assert_eq!(codes("model M { a?: int, b: int = a + 1 }"), ["E0903"]);
    assert!(codes("model M { a?: int, b: bool = has(a) && a > 0 }").is_empty());
}

#[test]
fn now_and_unknown_names_are_the_errors_of_an_invariant() {
    assert_eq!(
        codes("model M { a: datetime, b: bool = now > a }"),
        ["E0904"]
    );
    assert_eq!(codes("model M { a: int, b: int = nope + 1 }"), ["E0901"]);
}

#[test]
fn an_invariant_may_read_a_derived_field_and_a_derived_field_a_parent_field() {
    assert!(
        codes("model P { a: int }\nmodel C extends P { b: int = a + 1\n rule r: b > a }")
            .is_empty()
    );
}

#[test]
fn a_derived_field_that_reads_nothing_is_allowed() {
    assert!(codes("model M { a: int = 1 + 2 }").is_empty());
}

#[test]
fn the_else_payload_never_has_to_give_a_derived_field() {
    let src = "error Low 409 { have: int, need: int, gap: int = need - have }\n\
model M { n: int }\n\
service S at \"/s\" { post \"/\" f(body: M) -> 201 M | Low {\n\
  require body.n > 0 else Low { have: 1, need: 2 }\n  } }";
    assert!(one(src).ir.is_some(), "{}", one(src).render());
}

#[test]
fn an_example_may_not_write_a_derived_field_but_may_name_it_after_ok() {
    let base = "model L { a: int, b: int = a + 1 }\n\
service S at \"/s\" { post \"/\" f(body: L) -> 201 L }\n";
    let writes = format!("{base}example \"x\" {{ f(body: {{ a: 1, b: 2 }}) -> ok }}");
    assert_eq!(codes(&writes), ["E0952"]);
    let names = format!(
        "{base}example \"x\" {{ f(body: {{ a: 1 }}) returns {{ a: 1 }} -> ok {{ b: 2 }} }}"
    );
    assert!(codes(&names).is_empty(), "{:?}", codes(&names));
}
