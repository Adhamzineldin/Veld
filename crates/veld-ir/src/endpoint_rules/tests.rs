use serde_json::{Value, json};

use crate::*;

pub(crate) fn origin(line: u32) -> Origin {
    Origin {
        file: "bookings.veld".into(),
        line,
        col: 3,
    }
}

fn int_ty() -> ExprType {
    ExprType::Type(Type::Scalar(Scalar::Int))
}

fn bool_ty() -> ExprType {
    ExprType::Type(Type::Scalar(Scalar::Bool))
}

fn step(name: &str) -> Step {
    Step {
        name: name.into(),
        wire: name.into(),
        optional: false,
    }
}

fn path(root: Root, steps: Vec<Step>, t: ExprType) -> Expr {
    Expr {
        t,
        e: ExprKind::Path(PathRef { root, steps }),
    }
}

fn le(l: Expr, r: Expr) -> Expr {
    Expr {
        t: bool_ty(),
        e: ExprKind::Bin(Box::new(BinExpr { op: Op::Le, l, r })),
    }
}

/// One of every statement form: a fact with a key, a named `require` with `else` and a payload,
/// an unnamed `require` without `else`, a named `ensure` and an unnamed one.
pub(crate) fn sample_rules() -> EndpointRules {
    let guests = path(Root::Param("body".into()), vec![step("guests")], int_ty());
    let capacity = path(Root::Fact("room".into()), vec![step("capacity")], int_ty());
    EndpointRules {
        allows: vec![],
        version: Some(EndpointVersion {
            id: "veld:bookings/Bookings.book.version@0123456789abcdef".into(),
            fact: "room".into(),
            field: "revision".into(),
            wire: "rev".into(),
            origin: origin(10),
        }),
        transition: Some(EndpointTransition {
            since: vec![step("pendingSince")],
            ..sample_transition()
        }),
        facts: vec![EndpointFact {
            name: "room".into(),
            ty: Type::Decl(DeclId(1)),
            optional: false,
            key: Some(path(
                Root::Param("roomId".into()),
                vec![],
                ExprType::Type(Type::Scalar(Scalar::Uuid)),
            )),
            origin: origin(10),
        }],
        requires: vec![
            EndpointRequire {
                name: Some("fits".into()),
                id: "veld:bookings/Bookings.book.fits@0123456789abcdef".into(),
                expr: le(guests.clone(), capacity.clone()),
                message: Some("too many guests".into()),
                reads: vec!["body".into()],
                else_error: Some(ElseError {
                    decl: DeclId(2),
                    status: 409,
                    fields: vec![ElseField {
                        name: "capacity".into(),
                        wire: "capacity".into(),
                        value: capacity,
                    }],
                }),
                docs: Some("a doc".into()),
                rollout: None,
                origin: origin(11),
            },
            EndpointRequire {
                name: None,
                id: "veld:bookings/Bookings.book.require@fedcba9876543210".into(),
                expr: le(guests, path(Root::Param("max".into()), vec![], int_ty())),
                message: None,
                reads: vec!["body".into(), "max".into()],
                else_error: None,
                docs: None,
                rollout: None,
                origin: origin(12),
            },
        ],
        ensures: vec![
            EndpointEnsure {
                name: Some("pending".into()),
                id: "veld:bookings/Bookings.book.pending@0011223344556677".into(),
                expr: Expr {
                    t: bool_ty(),
                    e: ExprKind::Lit(ExprLit::Bool(true)),
                },
                message: Some("must be pending".into()),
                reads: vec![],
                docs: None,
                rollout: None,
                origin: origin(13),
            },
            EndpointEnsure {
                name: None,
                id: "veld:bookings/Bookings.book.ensure@7766554433221100".into(),
                expr: Expr {
                    t: bool_ty(),
                    e: ExprKind::Now(Always),
                },
                message: None,
                reads: vec![],
                docs: Some("d".into()),
                rollout: None,
                origin: origin(14),
            },
        ],
    }
}

pub(crate) fn sample_service_facts() -> Vec<ServiceFact> {
    vec![
        ServiceFact {
            name: "room".into(),
            ty: Type::Decl(DeclId(1)),
            key: Some(Type::Scalar(Scalar::Uuid)),
            optional: false,
        },
        ServiceFact {
            name: "tenant".into(),
            ty: Type::Scalar(Scalar::String),
            key: None,
            optional: true,
        },
    ]
}

fn field(name: &str, value: ExampleValue) -> ExampleField {
    ExampleField {
        name: name.into(),
        wire: name.into(),
        value,
    }
}

/// Every `ExampleValue` form and every outcome form.
pub(crate) fn sample_examples() -> Vec<Example> {
    let given_ref = ExampleValue::Ref(ExampleRef {
        given: "room".into(),
        path: vec!["id".into()],
    });
    let tags = ExampleValue::List(vec![
        ExampleValue::Json(json!("a")),
        ExampleValue::Json(Value::Null),
    ]);
    let body = ExampleValue::Object(vec![
        field("roomId", given_ref),
        field("guests", ExampleValue::Json(json!(3))),
        field("tags", tags),
    ]);
    let capacity = ExampleValue::Object(vec![field("capacity", ExampleValue::Json(json!(2)))]);
    let full = Example {
        name: "a full room is rejected".into(),
        package: PackageId(0),
        service: Some(ServiceId(0)),
        endpoint: "createBooking".into(),
        givens: vec![
            ExampleGiven {
                fact: "room".into(),
                value: Some(capacity.clone()),
            },
            ExampleGiven {
                fact: "tenant".into(),
                value: None,
            },
        ],
        principal: Some(ExampleValue::Json(json!({"id": "u1"}))),
        now: Some("2026-03-01T09:00:00Z".into()),
        args: vec![ExampleArg {
            param: "body".into(),
            value: body,
        }],
        returns: Some(ExampleValue::Json(json!({"status": "PENDING"}))),
        if_match: None,
        key: None,
        outcome: Some(ExampleOutcome::Error(ExampleErrorOutcome {
            decl: DeclId(2),
            pattern: Some(capacity),
        })),
        event: None,
        expect: None,
        docs: Some("d".into()),
        origin: origin(20),
    };
    let ok = Example {
        name: "it works".into(),
        givens: vec![],
        principal: None,
        now: None,
        returns: None,
        if_match: Some(ExampleIfMatch::Any),
        key: Some(ExampleKey {
            value: "k-2".into(),
            store: KeyStore::Other,
        }),
        outcome: Some(ExampleOutcome::Ok(None)),
        docs: None,
        ..full.clone()
    };
    let ok_value = Example {
        name: "it answers a value".into(),
        if_match: Some(ExampleIfMatch::Version("3".into())),
        key: Some(ExampleKey {
            value: "k-1".into(),
            store: KeyStore::Stored(ExampleOutcome::Ok(None)),
        }),
        outcome: Some(ExampleOutcome::Ok(Some(ExampleValue::List(vec![])))),
        ..ok.clone()
    };
    let fresh = Example {
        name: "a key nobody used".into(),
        key: Some(ExampleKey {
            value: "k-4".into(),
            store: KeyStore::Running,
        }),
        if_match: None,
        ..ok.clone()
    };
    vec![full, ok, ok_value, fresh]
}

/// One event example per expectation form.
pub(crate) fn sample_event_examples() -> Vec<Example> {
    let call = Example {
        name: "a paid booking sends a receipt".into(),
        package: PackageId(0),
        service: None,
        endpoint: String::new(),
        givens: vec![],
        principal: None,
        now: None,
        args: vec![],
        returns: None,
        if_match: None,
        key: None,
        outcome: None,
        docs: None,
        event: Some(ExampleEvent {
            decl: DeclId(6),
            channel: ChannelId(0),
            value: ExampleValue::Object(vec![field("amount", ExampleValue::Json(json!("120.00")))]),
        }),
        expect: Some(EventExpect::Calls(vec![
            ExpectedCall {
                service: ServiceId(0),
                handler: "sendReceipt".into(),
                called: true,
            },
            ExpectedCall {
                service: ServiceId(0),
                handler: "sendApology".into(),
                called: false,
            },
        ])),
        origin: origin(30),
    };
    let rejected = Example {
        name: "an event that breaks its rule reaches no handler".into(),
        expect: Some(EventExpect::Rejected),
        ..call.clone()
    };
    vec![call, rejected]
}

pub(crate) fn sample_allows() -> Vec<EndpointAllow> {
    vec![EndpointAllow {
        policy: PolicyId(0),
        args: vec![
            path(
                Root::Principal,
                vec![],
                ExprType::Type(Type::Decl(DeclId(3))),
            ),
            path(
                Root::Fact("room".into()),
                vec![],
                ExprType::Type(Type::Decl(DeclId(1))),
            ),
        ],
        reads: vec!["roomId".into()],
        origin: origin(8),
    }]
}

pub(crate) fn sample_policies() -> Vec<Policy> {
    let owner = Expr {
        t: bool_ty(),
        e: ExprKind::Bin(Box::new(BinExpr {
            op: Op::Eq,
            l: path(
                Root::Param("user".into()),
                vec![step("id")],
                ExprType::Type(Type::Scalar(Scalar::Uuid)),
            ),
            r: path(
                Root::Param("room".into()),
                vec![step("ownerId")],
                ExprType::Type(Type::Scalar(Scalar::Uuid)),
            ),
        })),
    };
    let suspended = path(
        Root::Param("user".into()),
        vec![step("suspended")],
        bool_ty(),
    );
    vec![Policy {
        name: "ManageRoom".into(),
        package: PackageId(0),
        module: "rooms".into(),
        qualified: "rooms.ManageRoom".into(),
        id: "veld:bookings/ManageRoom@0123456789abcdef".into(),
        params: vec![
            PolicyParam {
                name: "user".into(),
                ty: Type::Decl(DeclId(3)),
            },
            PolicyParam {
                name: "room".into(),
                ty: Type::Decl(DeclId(1)),
            },
        ],
        clauses: vec![
            PolicyClause {
                effect: PolicyEffect::Permit,
                expr: owner,
                message: None,
                origin: origin(5),
            },
            PolicyClause {
                effect: PolicyEffect::Forbid,
                expr: suspended,
                message: Some("A suspended account cannot manage rooms".into()),
                origin: origin(6),
            },
        ],
        docs: Some("Who may manage a room.".into()),
        origin: origin(4),
    }]
}

fn endpoint_with(rules: EndpointRules) -> Endpoint {
    Endpoint {
        name: "book".into(),
        method: Method::Post,
        path: "/".into(),
        sig: "veld:bookings/Bookings.book@0".into(),
        docs: None,
        deprecated: None,
        auth: Auth::none(),
        middleware: vec![],
        params: vec![],
        success: Success {
            status: 201,
            ty: None,
            stream: false,
        },
        errors: vec![],
        rules,
        idempotency: None,
        origin: origin(9),
    }
}

fn service_with(endpoints: Vec<Endpoint>) -> Service {
    Service {
        id: ServiceId(0),
        package: PackageId(0),
        name: "Bookings".into(),
        prefix: "/".into(),
        base_url: None,
        docs: None,
        auth: Auth::none(),
        middleware: vec![],
        endpoints,
        facts: vec![],
        rules_digest: None,
        principal: None,
        publishes: vec![],
        subscriptions: vec![],
        origin: origin(1),
    }
}

fn ir_with(services: Vec<Service>, examples: Vec<Example>) -> Ir {
    Ir {
        ir_version: IR_VERSION.into(),
        packages: vec![],
        decls: vec![],
        services,
        consumes: Default::default(),
        examples,
        policies: vec![],
        channels: vec![],
    }
}

fn round_trip<T>(v: &T) -> Value
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let text = serde_json::to_string(v).unwrap();
    let back: T = serde_json::from_str(&text).unwrap();
    assert_eq!(&back, v);
    serde_json::from_str(&text).unwrap()
}

#[test]
fn an_endpoint_with_every_statement_form_round_trips() {
    let v = round_trip(&endpoint_with(sample_rules()));
    assert_eq!(v["rules"]["facts"].as_array().map(Vec::len), Some(1));
    assert_eq!(v["rules"]["requires"].as_array().map(Vec::len), Some(2));
    assert_eq!(v["rules"]["ensures"].as_array().map(Vec::len), Some(2));
    assert!(v["rules"]["requires"][1]["else"].is_null());
    assert!(v["rules"]["requires"][1]["name"].is_null());
}

#[test]
fn a_fact_and_a_require_match_the_spec_json() {
    let rules = sample_rules();
    let fact = serde_json::to_value(&rules.facts[0]).unwrap();
    assert_eq!(
        fact,
        json!({
            "name": "room",
            "type": { "decl": 1 },
            "optional": false,
            "key": { "t": { "scalar": "uuid" },
                     "e": { "path": { "root": { "param": "roomId" }, "steps": [] } } },
            "origin": { "file": "bookings.veld", "line": 10, "col": 3 }
        })
    );
    let require = serde_json::to_value(&rules.requires[0]).unwrap();
    assert_eq!(require["else"]["decl"], json!(2));
    assert_eq!(require["else"]["status"], json!(409));
    assert_eq!(require["else"]["fields"][0]["name"], json!("capacity"));
    assert_eq!(require["else"]["fields"][0]["wire"], json!("capacity"));
    assert_eq!(require["reads"], json!(["body"]));
    assert_eq!(require["message"], json!("too many guests"));
    let text = serde_json::to_string(&rules.ensures[1]).unwrap();
    let head = r#"{"name":null,"id":"veld:bookings/Bookings.book.ensure@7766554433221100","expr":{"t":{"scalar":"bool"},"e":{"now":true}},"message":null,"reads":[],"docs":"d","origin":"#;
    assert!(text.starts_with(head), "{text}");
}

#[test]
fn service_facts_match_the_spec_json() {
    let facts = sample_service_facts();
    assert_eq!(
        serde_json::to_string(&facts[1]).unwrap(),
        r#"{"name":"tenant","type":{"scalar":"string"},"key":null,"optional":true}"#
    );
}

#[test]
fn an_example_with_every_value_form_round_trips() {
    let v = round_trip(&sample_examples());
    assert_eq!(
        v[0]["givens"][1],
        json!({ "fact": "tenant", "value": null })
    );
    assert_eq!(v[1]["outcome"], json!({ "ok": null }));
    assert_eq!(v[2]["outcome"], json!({ "ok": { "list": [] } }));
    assert_eq!(
        v[0]["outcome"],
        json!({ "error": { "decl": 2, "pattern": { "object": [
            { "name": "capacity", "wire": "capacity", "value": { "json": 2 } }
        ] } } })
    );
}

#[test]
fn example_values_match_the_spec_json() {
    let r = ExampleValue::Ref(ExampleRef {
        given: "room".into(),
        path: vec!["id".into()],
    });
    assert_eq!(
        serde_json::to_string(&r).unwrap(),
        r#"{"ref":{"given":"room","path":["id"]}}"#
    );
    let o = ExampleValue::Object(vec![field("guests", ExampleValue::Json(json!(3)))]);
    assert_eq!(
        serde_json::to_string(&o).unwrap(),
        r#"{"object":[{"name":"guests","wire":"guests","value":{"json":3}}]}"#
    );
    let l = ExampleValue::List(vec![ExampleValue::Json(Value::Null)]);
    assert_eq!(
        serde_json::to_string(&l).unwrap(),
        r#"{"list":[{"json":null}]}"#
    );
    let back: ExampleValue = serde_json::from_str(r#"{"json":null}"#).unwrap();
    assert_eq!(back, ExampleValue::Json(Value::Null));
}

#[test]
fn an_example_has_the_keys_of_the_spec() {
    let v = serde_json::to_value(&sample_examples()[0]).unwrap();
    let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "args",
            "docs",
            "endpoint",
            "givens",
            "name",
            "now",
            "origin",
            "outcome",
            "package",
            "principal",
            "returns",
            "service"
        ]
    );
}

#[test]
fn nothing_new_is_serialised_when_it_is_empty() {
    let endpoint = endpoint_with(EndpointRules::default());
    let ir = ir_with(vec![service_with(vec![endpoint])], vec![]);
    let v = serde_json::to_value(&ir).unwrap();
    assert!(v.get("examples").is_none());
    let service = &v["services"][0];
    assert!(service.get("facts").is_none());
    assert!(service.get("rules_digest").is_none());
    assert!(service["endpoints"][0].get("rules").is_none());
    // A document written before these keys existed reads back as empty.
    let back: Ir = serde_json::from_value(v).unwrap();
    assert_eq!(back, ir);
}

#[test]
fn the_new_keys_appear_when_there_is_something_to_say() {
    let mut service = service_with(vec![endpoint_with(sample_rules())]);
    service.facts = sample_service_facts();
    service.rules_digest = Some("3fa9c2e1b7d04a55".into());
    let v = round_trip(&ir_with(vec![service], sample_examples()));
    assert!(v["examples"].is_array());
    assert_eq!(v["services"][0]["rules_digest"], json!("3fa9c2e1b7d04a55"));
    assert_eq!(v["services"][0]["facts"][0]["name"], json!("room"));
    assert!(v["services"][0]["endpoints"][0]["rules"]["requires"].is_array());
}

#[test]
fn endpoint_rules_use_the_rules_endpoint_feature() {
    let plain = endpoint_with(EndpointRules::default());
    let plain = ir_with(vec![service_with(vec![plain])], vec![]);
    assert!(plain.features_used(&[ServiceId(0)]).is_empty());
    let ruled = ir_with(
        vec![service_with(vec![endpoint_with(sample_rules())])],
        vec![],
    );
    let used = ruled.features_used(&[ServiceId(0)]);
    assert!(used.contains(&Feature::RulesEndpoint), "{used:?}");
    assert!(ruled.features_used(&[]).is_empty());
}

#[test]
fn the_first_origin_is_the_first_statement() {
    let rules = sample_rules();
    assert_eq!(rules.first_origin().map(|o| o.line), Some(10));
    let no_facts = EndpointRules {
        facts: vec![],
        transition: None,
        ..rules
    };
    assert_eq!(no_facts.first_origin().map(|o| o.line), Some(11));
    let only_ensure = EndpointRules {
        requires: vec![],
        ..no_facts
    };
    assert_eq!(only_ensure.first_origin().map(|o| o.line), Some(13));
    assert!(EndpointRules::default().first_origin().is_none());
}

fn sample_transition() -> EndpointTransition {
    EndpointTransition {
        id: "veld:bookings/Bookings.book.transition@0123456789abcdef".into(),
        fact: "booking".into(),
        steps: vec![step("status")],
        decl: DeclId(0),
        from: vec!["pending".into(), "paid".into()],
        to: "cancelled".into(),
        since: vec![],
        origin: origin(12),
    }
}

fn variant(name: &str) -> Variant {
    Variant {
        name: name.into(),
        wire: name.into(),
        docs: None,
        deprecated: None,
    }
}

fn machine(flows: Vec<Flow>, finals: Vec<String>) -> Enum {
    Enum {
        variants: vec![variant("pending"), variant("paid"), variant("cancelled")],
        flows,
        finals,
    }
}

fn flow(from: &str, to: &str) -> Flow {
    Flow {
        from: from.into(),
        to: to.into(),
        after_ms: None,
    }
}

fn enum_decl(e: Enum) -> Decl {
    Decl {
        id: DeclId(0),
        package: PackageId(0),
        module: "bookings".into(),
        name: "Status".into(),
        qualified: "bookings.Status".into(),
        sig: "veld:bookings/Status@0".into(),
        docs: None,
        deprecated: None,
        origin: origin(3),
        kind: DeclKind::Enum(e),
    }
}

#[test]
fn a_transition_matches_the_spec_json() {
    let rules = EndpointRules {
        transition: Some(sample_transition()),
        ..EndpointRules::default()
    };
    let v = round_trip(&rules);
    assert_eq!(
        v["transition"],
        json!({
            "id": "veld:bookings/Bookings.book.transition@0123456789abcdef",
            "fact": "booking",
            "steps": [ { "name": "status", "wire": "status", "optional": false } ],
            "decl": 0,
            "from": ["pending", "paid"],
            "to": "cancelled",
            "origin": { "file": "bookings.veld", "line": 12, "col": 3 }
        })
    );
    assert!(!rules.is_empty());
    assert_eq!(rules.first_origin().map(|o| o.line), Some(12));
}

#[test]
fn flows_and_finals_match_the_spec_json() {
    let e = machine(
        vec![flow("pending", "paid"), flow("pending", "cancelled")],
        vec!["cancelled".into()],
    );
    let v = round_trip(&e);
    assert_eq!(
        v["flows"],
        json!([ { "from": "pending", "to": "paid" }, { "from": "pending", "to": "cancelled" } ])
    );
    assert_eq!(v["finals"], json!(["cancelled"]));
    assert!(e.is_state_machine());
}

#[test]
fn nothing_about_state_machines_is_serialised_when_absent() {
    let v = serde_json::to_value(Enum::plain(vec![variant("a")])).unwrap();
    assert_eq!(
        v,
        json!({ "variants": [ { "name": "a", "wire": "a", "docs": null, "deprecated": null } ] })
    );
    let rules = serde_json::to_value(EndpointRules::default()).unwrap();
    assert!(rules.get("transition").is_none());
    let back: Enum = serde_json::from_value(v).unwrap();
    assert!(!back.is_state_machine());
}

#[test]
fn a_transition_uses_the_transitions_feature() {
    let rules = EndpointRules {
        transition: Some(sample_transition()),
        ..EndpointRules::default()
    };
    let ir = ir_with(vec![service_with(vec![endpoint_with(rules)])], vec![]);
    let used = ir.features_used(&[ServiceId(0)]);
    assert!(used.contains(&Feature::RulesTransitions), "{used:?}");
    let plain = ir_with(
        vec![service_with(vec![endpoint_with(EndpointRules {
            transition: None,
            ..sample_rules()
        })])],
        vec![],
    );
    let used = plain.features_used(&[ServiceId(0)]);
    assert!(!used.contains(&Feature::RulesTransitions), "{used:?}");
}

#[test]
fn a_reachable_state_machine_uses_the_transitions_feature() {
    let mut ep = endpoint_with(EndpointRules::default());
    ep.success.ty = Some(Type::Decl(DeclId(0)));
    let mut ir = ir_with(vec![service_with(vec![ep])], vec![]);
    ir.decls = vec![enum_decl(machine(vec![flow("pending", "paid")], vec![]))];
    let used = ir.features_used(&[ServiceId(0)]);
    assert_eq!(
        used.iter().copied().collect::<Vec<_>>(),
        [Feature::RulesTransitions]
    );
    ir.decls = vec![enum_decl(machine(vec![], vec![]))];
    assert!(ir.features_used(&[ServiceId(0)]).is_empty());
}

#[test]
fn the_exact_json_text_of_a_machine_and_a_transition() {
    let e = Enum {
        variants: vec![variant("pending"), variant("paid")],
        flows: vec![flow("pending", "paid")],
        finals: vec!["paid".into()],
    };
    assert_eq!(
        serde_json::to_string(&e).unwrap(),
        concat!(
            r#"{"variants":[{"name":"pending","wire":"pending","docs":null,"deprecated":null},"#,
            r#"{"name":"paid","wire":"paid","docs":null,"deprecated":null}],"#,
            r#""flows":[{"from":"pending","to":"paid"}],"finals":["paid"]}"#
        )
    );
    let rules = EndpointRules {
        transition: Some(sample_transition()),
        ..EndpointRules::default()
    };
    assert_eq!(
        serde_json::to_string(&rules).unwrap(),
        concat!(
            r#"{"facts":[],"requires":[],"ensures":[],"transition":{"#,
            r#""id":"veld:bookings/Bookings.book.transition@0123456789abcdef","#,
            r#""fact":"booking","steps":[{"name":"status","wire":"status","optional":false}],"#,
            r#""decl":0,"from":["pending","paid"],"to":"cancelled","#,
            r#""origin":{"file":"bookings.veld","line":12,"col":3}}}"#
        )
    );
    assert_eq!(
        serde_json::to_string(&EndpointRules::default()).unwrap(),
        r#"{"facts":[],"requires":[],"ensures":[]}"#
    );
}

mod policies;
mod rollout;
mod timed;
mod version;
