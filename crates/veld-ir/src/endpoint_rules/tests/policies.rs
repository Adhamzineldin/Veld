//! Policies, the principal and `allow` in the IR (`spec/rules.md` section 11.5).

use serde_json::json;

use super::*;

fn ir_with_policies(allows: Vec<EndpointAllow>) -> Ir {
    let rules = EndpointRules {
        allows,
        ..EndpointRules::default()
    };
    let mut service = service_with(vec![endpoint_with(rules)]);
    service.principal = Some(ServicePrincipal {
        decl: DeclId(3),
        name: "user".into(),
    });
    let mut ir = ir_with(vec![service], vec![]);
    ir.policies = sample_policies();
    ir
}

#[test]
fn policies_a_principal_and_allows_round_trip() {
    let v = round_trip(&ir_with_policies(sample_allows()));
    assert_eq!(v["policies"][0]["name"], json!("ManageRoom"));
    assert_eq!(v["policies"][0]["clauses"][0]["effect"], json!("permit"));
    assert_eq!(v["policies"][0]["clauses"][1]["effect"], json!("forbid"));
    assert_eq!(
        v["services"][0]["principal"],
        json!({ "decl": 3, "name": "user" })
    );
    assert_eq!(
        v["services"][0]["endpoints"][0]["rules"]["allows"][0]["policy"],
        json!(0)
    );
}

#[test]
fn nothing_about_policies_is_emitted_without_them() {
    let ir = ir_with(
        vec![service_with(vec![endpoint_with(sample_rules())])],
        vec![],
    );
    let v = serde_json::to_value(&ir).unwrap();
    assert!(v.get("policies").is_none());
    assert!(v["services"][0].get("principal").is_none());
    assert!(
        v["services"][0]["endpoints"][0]["rules"]
            .get("allows")
            .is_none()
    );
    let text = serde_json::to_string(&ir).unwrap();
    assert!(
        !text.contains("allows") && !text.contains("principal"),
        "{text}"
    );
    let back: Ir = serde_json::from_str(&text).unwrap();
    assert_eq!(back, ir);
}

#[test]
fn the_exact_json_text_of_a_policy_a_principal_and_an_allow() {
    let allow = EndpointAllow {
        policy: PolicyId(0),
        args: vec![],
        reads: vec!["id".into()],
        origin: origin(8),
    };
    assert_eq!(
        serde_json::to_string(&allow).unwrap(),
        r#"{"policy":0,"args":[],"reads":["id"],"origin":{"file":"bookings.veld","line":8,"col":3}}"#
    );
    let rules = EndpointRules {
        allows: vec![allow],
        ..EndpointRules::default()
    };
    assert_eq!(
        serde_json::to_string(&rules).unwrap(),
        concat!(
            r#"{"facts":[],"requires":[],"ensures":[],"allows":[{"policy":0,"args":[],"#,
            r#""reads":["id"],"origin":{"file":"bookings.veld","line":8,"col":3}}]}"#
        )
    );
    let principal = ServicePrincipal {
        decl: DeclId(3),
        name: "user".into(),
    };
    assert_eq!(
        serde_json::to_string(&principal).unwrap(),
        r#"{"decl":3,"name":"user"}"#
    );
    let mut policy = sample_policies().remove(0);
    policy.params.truncate(1);
    policy.clauses.truncate(1);
    policy.clauses[0].expr = path(
        Root::Param("user".into()),
        vec![step("suspended")],
        bool_ty(),
    );
    policy.docs = None;
    assert_eq!(
        serde_json::to_string(&policy).unwrap(),
        concat!(
            r#"{"name":"ManageRoom","package":0,"module":"rooms","qualified":"rooms.ManageRoom","#,
            r#""id":"veld:bookings/ManageRoom@0123456789abcdef","#,
            r#""params":[{"name":"user","type":{"decl":3}}],"#,
            r#""clauses":[{"effect":"permit","expr":{"t":{"scalar":"bool"},"#,
            r#""e":{"path":{"root":{"param":"user"},"steps":[{"name":"suspended","wire":"suspended","optional":false}]}}},"#,
            r#""message":null,"origin":{"file":"bookings.veld","line":5,"col":3}}],"#,
            r#""docs":null,"origin":{"file":"bookings.veld","line":4,"col":3}}"#
        )
    );
}

#[test]
fn an_allow_uses_the_policies_feature() {
    let with = ir_with_policies(sample_allows());
    let used = with.features_used(&[ServiceId(0)]);
    assert!(used.contains(&Feature::RulesPolicies), "{used:?}");
    let without = ir_with_policies(vec![]);
    assert!(without.features_used(&[ServiceId(0)]).is_empty());
    // A policy no endpoint uses needs no feature.
    let mut unused = ir_with_policies(vec![]);
    unused.policies = sample_policies();
    assert!(
        !unused
            .features_used(&[ServiceId(0)])
            .contains(&Feature::RulesPolicies)
    );
}

#[test]
fn reading_the_principal_without_an_allow_needs_the_policies_feature_too() {
    let principal = path(Root::Principal, vec![step("suspended")], bool_ty());
    let require = EndpointRequire {
        name: None,
        id: "veld:bookings/Bookings.book.require@0123456789abcdef".into(),
        expr: principal.clone(),
        message: None,
        reads: vec![],
        else_error: None,
        docs: None,
        rollout: None,
        origin: origin(14),
    };
    assert!(principal.reads_principal());
    let rules = EndpointRules {
        requires: vec![require],
        ..EndpointRules::default()
    };
    assert_eq!(rules.first_policy_origin().map(|o| o.line), Some(14));
    let ir = ir_with(vec![service_with(vec![endpoint_with(rules)])], vec![]);
    let used = ir.features_used(&[ServiceId(0)]);
    assert!(used.contains(&Feature::RulesPolicies), "{used:?}");
    assert!(used.contains(&Feature::RulesEndpoint), "{used:?}");
    // A rule that does not read the caller needs no policies.
    assert!(sample_rules().first_policy_origin().is_none());
    assert!(
        !ir_with(
            vec![service_with(vec![endpoint_with(sample_rules())])],
            vec![]
        )
        .features_used(&[ServiceId(0)])
        .contains(&Feature::RulesPolicies)
    );
}

#[test]
fn the_principal_is_found_inside_every_expression_form() {
    let p = || {
        path(
            Root::Principal,
            vec![step("id")],
            ExprType::Type(Type::Scalar(Scalar::Uuid)),
        )
    };
    let wrap = |e: ExprKind| Expr { t: bool_ty(), e };
    assert!(wrap(ExprKind::Not(Box::new(p()))).reads_principal());
    assert!(
        wrap(ExprKind::Has(PathRef {
            root: Root::Principal,
            steps: vec![]
        }))
        .reads_principal()
    );
    assert!(wrap(ExprKind::List(vec![p()])).reads_principal());
    assert!(
        wrap(ExprKind::Bin(Box::new(BinExpr {
            op: Op::Eq,
            l: p(),
            r: p()
        })))
        .reads_principal()
    );
    let plain = path(Root::Param("x".into()), vec![], bool_ty());
    assert!(!plain.reads_principal());
    assert!(!wrap(ExprKind::Now(Always)).reads_principal());
}

#[test]
fn the_declared_mark_of_an_auth_is_neither_json_nor_equality() {
    let declared = Auth::none().declared();
    assert!(declared.declared);
    assert_eq!(declared, Auth::none());
    let json = serde_json::to_string(&declared).unwrap();
    assert_eq!(json, r#"{"kind":"none","header":null}"#);
    let back: Auth = serde_json::from_str(&json).unwrap();
    assert!(!back.declared);
}

#[test]
fn the_first_origin_counts_an_allow() {
    let rules = EndpointRules {
        allows: sample_allows(),
        requires: sample_rules().requires,
        ..EndpointRules::default()
    };
    assert_eq!(rules.first_origin().map(|o| o.line), Some(8));
    assert!(!rules.is_empty());
}
