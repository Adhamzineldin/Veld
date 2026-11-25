use serde_json::{Value, json};
use veld_ir::ServiceId;

use super::openapi;
use crate::testutil::ir;

fn doc(src: &str) -> Value {
    let ir = ir(src);
    let ids: Vec<ServiceId> = ir.services.iter().map(|s| s.id).collect();
    openapi(&ir, &ids)
}

const ORDERS: &str = r#"
enum Status { pending, paid = "PAID" }
model Base { id: uuid @readonly }
model Order extends Base {
  qty: int @min(1) @max(9)
  total: long
  price: decimal
  blob: bytes
  status: Status = .pending
  tags: string[] @length(max: 3)
  note?: string @length(min: 1) @format(email)
}
error OutOfStock 409 { sku: string }
@auth(bearer)
service Orders at "/orders" {
  /// Fetch one.
  get "/{id}" getOrder(id: uuid) -> Order | NotFound
  post "/" create(body: Order) -> 201 Order | OutOfStock
  get "/" list(@query page?: int) -> Page<Order>
  @auth(none)
  get "/ping" ping() -> 204
}
"#;

#[test]
fn top_level_shape_and_key_order() {
    let d = doc(ORDERS);
    let keys: Vec<&String> = d
        .as_object()
        .map(|m| m.keys().collect())
        .unwrap_or_default();
    assert_eq!(
        keys,
        [
            "openapi",
            "info",
            "paths",
            "components",
            "x-veld-ir-version"
        ]
    );
    assert_eq!(d["openapi"], "3.1.0");
}

#[test]
fn output_is_deterministic() {
    let a = serde_json::to_string(&doc(ORDERS)).unwrap_or_default();
    let b = serde_json::to_string(&doc(ORDERS)).unwrap_or_default();
    assert_eq!(a, b);
}

#[test]
fn schemas_are_named_package_dot_name_with_prelude_as_veld() {
    let d = doc(ORDERS);
    let names: Vec<&String> = d["components"]["schemas"]
        .as_object()
        .map(|m| m.keys().collect())
        .unwrap_or_default();
    assert!(names.contains(&&"app.Order".to_string()), "{names:?}");
    assert!(names.contains(&&"veld.NotFound".to_string()), "{names:?}");
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
}

#[test]
fn extends_uses_all_of_and_only_own_fields() {
    let d = doc(ORDERS);
    let order = &d["components"]["schemas"]["app.Order"];
    assert_eq!(
        order["allOf"][0],
        json!({"$ref": "#/components/schemas/app.Base"})
    );
    assert!(order["allOf"][1]["properties"].get("id").is_none());
    assert!(order["allOf"][1]["properties"].get("qty").is_some());
    assert!(
        order["x-veld-sig"]
            .as_str()
            .is_some_and(|s| s.starts_with("veld:app/Order@"))
    );
}

#[test]
fn scalar_and_constraint_mapping() {
    let d = doc(ORDERS);
    let p = &d["components"]["schemas"]["app.Order"]["allOf"][1]["properties"];
    assert_eq!(
        p["qty"],
        json!({"type": "integer", "format": "int32", "minimum": 1, "maximum": 9})
    );
    assert_eq!(p["total"], json!({"type": "string", "format": "int64"}));
    assert_eq!(p["price"], json!({"type": "string", "format": "decimal"}));
    assert_eq!(
        p["blob"],
        json!({"type": "string", "contentEncoding": "base64"})
    );
    assert_eq!(p["tags"]["maxItems"], 3);
    assert_eq!(p["note"]["minLength"], 1);
    assert_eq!(p["note"]["format"], "email");
    assert_eq!(p["status"]["default"], "pending");
}

#[test]
fn defaulted_and_optional_fields_are_not_required() {
    let d = doc(ORDERS);
    let req = &d["components"]["schemas"]["app.Order"]["allOf"][1]["required"];
    assert_eq!(req, &json!(["qty", "total", "price", "blob", "tags"]));
}

#[test]
fn readonly_and_enum_wire_values() {
    let d = doc(ORDERS);
    assert_eq!(
        d["components"]["schemas"]["app.Base"]["properties"]["id"]["readOnly"],
        true
    );
    assert_eq!(
        d["components"]["schemas"]["app.Status"]["enum"],
        json!(["pending", "PAID"])
    );
}

#[test]
fn error_responses_are_problem_json_with_signatures() {
    let d = doc(ORDERS);
    let op = &d["paths"]["/orders"]["post"];
    let r = &op["responses"]["409"]["content"]["application/problem+json"]["schema"];
    let sig = op["x-veld-op-sig"].as_str().unwrap_or_default();
    assert_eq!(r["properties"]["operation"], json!({"const": sig}));
    assert!(
        r["properties"]["type"]["const"]
            .as_str()
            .is_some_and(|s| s.starts_with("veld:app/OutOfStock@"))
    );
    assert_eq!(
        r["properties"]["data"],
        json!({"$ref": "#/components/schemas/app.OutOfStock"})
    );
    assert!(
        op["responses"].get("422").is_some(),
        "implicit ValidationError"
    );
}

#[test]
fn page_is_inlined() {
    let d = doc(ORDERS);
    let s =
        &d["paths"]["/orders"]["get"]["responses"]["200"]["content"]["application/json"]["schema"];
    assert_eq!(
        s["properties"]["total"],
        json!({"type": "string", "format": "int64"})
    );
    assert_eq!(
        s["properties"]["items"]["items"],
        json!({"$ref": "#/components/schemas/app.Order"})
    );
}

#[test]
fn security_comes_from_auth() {
    let d = doc(ORDERS);
    assert_eq!(
        d["paths"]["/orders/{id}"]["get"]["security"],
        json!([{"bearerAuth": []}])
    );
    assert!(d["paths"]["/orders/ping"]["get"].get("security").is_none());
    assert_eq!(
        d["components"]["securitySchemes"]["bearerAuth"],
        json!({"type": "http", "scheme": "bearer"})
    );
}

#[test]
fn bodyless_success_has_no_content() {
    let d = doc(ORDERS);
    assert_eq!(
        d["paths"]["/orders/ping"]["get"]["responses"]["204"],
        json!({"description": "No content"})
    );
}

#[test]
fn operations_carry_docs_and_params() {
    let d = doc(ORDERS);
    let op = &d["paths"]["/orders/{id}"]["get"];
    assert_eq!(op["summary"], "Fetch one.");
    assert_eq!(
        op["parameters"][0],
        json!({"name": "id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}})
    );
}

#[test]
fn unions_use_one_of_and_a_discriminator() {
    let d = doc(
        "model A { x: int }\nmodel B { y: int }\nunion U { a: A, b: B }\nmodel Holder { u: U }\nservice S at \"/\" { get \"/\" g() -> Holder }",
    );
    let u = &d["components"]["schemas"]["app.U"];
    assert_eq!(u["discriminator"]["propertyName"], "kind");
    assert_eq!(
        u["discriminator"]["mapping"]["a"],
        "#/components/schemas/app.A"
    );
    assert_eq!(
        u["oneOf"][0]["allOf"][1]["properties"]["kind"],
        json!({"const": "a"})
    );
}

#[test]
fn websocket_endpoints_are_listed_not_exported() {
    let d = doc(
        "model M { t: string }\nservice S at \"/s\" { ws \"/w\" w(body: stream M) -> stream M }",
    );
    assert_eq!(d["paths"], json!({}));
    assert_eq!(
        d["x-veld-websocket-operations"].as_array().map(Vec::len),
        Some(1)
    );
}

#[test]
fn query_objects_expand_into_parameters() {
    let d =
        doc("model F { q?: string, n: int }\nservice S at \"/s\" { get \"/\" g(query: F) -> 204 }");
    let params = &d["paths"]["/s"]["get"]["parameters"];
    assert_eq!(params[0]["name"], "q");
    assert_eq!(params[0]["required"], false);
    assert_eq!(params[1]["name"], "n");
    assert_eq!(params[1]["required"], true);
}

#[test]
fn errors_sharing_a_status_become_one_of() {
    let d = doc(
        "error A 409 {}\nerror B 409 { n: int }\nservice S at \"/s\" { get \"/\" g() -> 204 | A | B }",
    );
    let r = &d["paths"]["/s"]["get"]["responses"]["409"];
    assert_eq!(r["description"], "A or B");
    let alts = &r["content"]["application/problem+json"]["schema"]["oneOf"];
    assert_eq!(alts.as_array().map(Vec::len), Some(2));
    let props: Vec<&Value> = alts
        .as_array()
        .map(|a| {
            a.iter()
                .map(|s| &s["properties"]["title"]["const"])
                .collect()
        })
        .unwrap_or_default();
    assert!(props.contains(&&json!("A")) && props.contains(&&json!("B")));
}

// --- rules ------------------------------------------------------------------------------

const RULED: &str = r#"
enum St { open, paid = "PAID", done
  flow open -> paid -> done
  final done
}
model Item {
  id: uuid
  st: St
  startAt: datetime @wire("start_at")
  endAt: datetime @wire("end_at")
  rule order: startAt < endAt message "must end later"
}
model Sub extends Item { extra: int }
error Gone 410 { why: string
  rule short: len(why) < 20
}
service S at "/s" {
  post "/{id}/pay" pay(id: uuid, body: Sub) -> Item | Gone {
    fact item: Item = id
    fact other?: Item = id
    transition item.st: open -> paid
    require ok: body.extra > 0 else Gone { why: "no" } message "extra needed"
    require body.extra < 9
    ensure sameId: result.id == old(item.id)
  }
}
"#;

#[test]
fn invariants_become_x_veld_rules_with_wire_names() {
    let d = doc(RULED);
    let item = &d["components"]["schemas"]["app.Item"];
    assert_eq!(
        item["x-veld-rules"][0]["fields"],
        json!(["start_at", "end_at"])
    );
    assert_eq!(item["x-veld-rules"][0]["name"], "order");
    assert_eq!(item["x-veld-rules"][0]["message"], "must end later");
    assert_eq!(item["x-veld-rules"][0]["expression"], "startAt < endAt");
    assert!(
        item["x-veld-rules"][0]["id"]
            .as_str()
            .is_some_and(|s| s.starts_with("veld:app/Item.order@"))
    );
    let keys: Vec<&String> = item["x-veld-rules"][0]
        .as_object()
        .map(|m| m.keys().collect())
        .unwrap_or_default();
    assert_eq!(keys, ["name", "id", "message", "fields", "expression"]);
    // errors carry theirs; a child lists only its own rules (the parent schema has the rest)
    assert_eq!(
        d["components"]["schemas"]["app.Gone"]["x-veld-rules"][0]["name"],
        "short"
    );
    assert!(
        d["components"]["schemas"]["app.Sub"]
            .get("x-veld-rules")
            .is_none()
    );
}

#[test]
fn state_machines_list_flows_and_finals_as_wire_values() {
    let d = doc(RULED);
    let st = &d["components"]["schemas"]["app.St"];
    assert_eq!(
        st["x-veld-flows"],
        json!([{"from": "open", "to": "PAID"}, {"from": "PAID", "to": "done"}])
    );
    assert_eq!(st["x-veld-finals"], json!(["done"]));
}

#[test]
fn operations_document_facts_requires_ensures_and_the_transition() {
    let d = doc(RULED);
    let op = &d["paths"]["/s/{id}/pay"]["post"];
    assert_eq!(op["x-veld-facts"][0]["name"], "item");
    assert_eq!(
        op["x-veld-facts"][0]["type"],
        json!({"$ref": "#/components/schemas/app.Item"})
    );
    assert_eq!(op["x-veld-facts"][1]["optional"], true);
    let req = &op["x-veld-rules"]["requires"];
    assert_eq!(req[0]["name"], "ok");
    assert_eq!(req[0]["expression"], "body.extra > 0");
    assert_eq!(req[0]["else"], "Gone");
    assert_eq!(req[0]["message"], "extra needed");
    assert!(req[1].get("name").is_none() && req[1].get("else").is_none());
    assert_eq!(
        op["x-veld-rules"]["ensures"][0]["expression"],
        "result.id == old(item.id)"
    );
    assert_eq!(op["x-veld-transition"]["field"], "item.st");
    assert_eq!(op["x-veld-transition"]["from"], json!(["open"]));
    assert_eq!(op["x-veld-transition"]["to"], "PAID");
    // the implicit errors are documented because the IR lists them, not added by the exporter
    for status in ["404", "409", "410", "422"] {
        assert!(op["responses"].get(status).is_some(), "{status}");
    }
}

#[test]
fn a_contract_without_rules_has_no_rule_extensions() {
    let text = serde_json::to_string(&doc(ORDERS)).unwrap_or_default();
    for key in [
        "x-veld-rules",
        "x-veld-facts",
        "x-veld-flows",
        "x-veld-finals",
        "x-veld-transition",
    ] {
        assert!(!text.contains(key), "{key}");
    }
}
