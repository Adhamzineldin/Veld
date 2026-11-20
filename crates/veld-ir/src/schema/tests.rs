use serde_json::{Value, json};

use super::*;
use crate::*;

fn origin() -> Origin {
    Origin {
        file: "a.veld".into(),
        line: 1,
        col: 1,
    }
}

fn all_literals() -> Vec<Literal> {
    let probe = Literal::Null;
    // Adding a variant breaks this match, so the sample below cannot silently miss it.
    match probe {
        Literal::String(_) | Literal::Int(_) | Literal::Float(_) | Literal::Bool(_) => {}
        Literal::Null | Literal::EnumVariant { .. } | Literal::EmptyList => {}
    }
    vec![
        Literal::String("s".into()),
        Literal::Int(1),
        Literal::Float(1.5),
        Literal::Bool(true),
        Literal::Null,
        Literal::EnumVariant {
            name: "A".into(),
            wire: "a".into(),
        },
        Literal::EmptyList,
    ]
}

fn all_types() -> Vec<Type> {
    let probe = Type::Decl(DeclId(0));
    match probe {
        Type::Scalar(_) | Type::List(_) | Type::Map(_) | Type::Page(_) | Type::Decl(_) => {}
    }
    let mut v: Vec<Type> = Scalar::ALL.into_iter().map(Type::Scalar).collect();
    v.push(Type::List(Box::new(Type::Scalar(Scalar::Int))));
    v.push(Type::Map(Box::new(Type::Decl(DeclId(0)))));
    v.push(Type::Page(Box::new(Type::Decl(DeclId(0)))));
    v.push(Type::Decl(DeclId(1)));
    v
}

fn field(name: &str, ty: Type, lit: Literal) -> Field {
    Field {
        name: name.into(),
        wire: name.into(),
        ty,
        optional: true,
        default: Some(lit.clone()),
        readonly: false,
        inherited_from: Some(DeclId(0)),
        constraints: Constraints {
            min: Some(0.0),
            max: Some(9.0),
            min_len: Some(1),
            max_len: Some(2),
            pattern: Some("^a$".into()),
            format: Some(StringFormat::Email),
        },
        meta: vec!["m".into()],
        example: Some(lit),
        docs: Some("d".into()),
        deprecated: Some("x".into()),
        origin: origin(),
    }
}

fn decl(id: u32, name: &str, kind: DeclKind) -> Decl {
    Decl {
        id: DeclId(id),
        package: PackageId(0),
        module: "m".into(),
        name: name.into(),
        qualified: format!("p:m.{name}"),
        sig: "veld:p/m.X@0".into(),
        docs: Some("d".into()),
        deprecated: None,
        origin: origin(),
        kind,
    }
}

fn decls() -> Vec<Decl> {
    let probe = DeclKind::Enum(Enum { variants: vec![] });
    match probe {
        DeclKind::Model(_)
        | DeclKind::Error(_)
        | DeclKind::Enum(_)
        | DeclKind::Union(_)
        | DeclKind::Const(_) => {}
    }
    let fields: Vec<Field> = all_types()
        .into_iter()
        .zip(all_literals().into_iter().cycle())
        .enumerate()
        .map(|(i, (t, l))| field(&format!("f{i}"), t, l))
        .collect();
    let model = Model {
        parent: Some(DeclId(0)),
        fields,
    };
    let variant = Variant {
        name: "A".into(),
        wire: "a".into(),
        docs: Some("d".into()),
        deprecated: None,
    };
    vec![
        decl(
            0,
            "A",
            DeclKind::Model(Model {
                parent: None,
                fields: vec![],
            }),
        ),
        decl(1, "B", DeclKind::Model(model.clone())),
        decl(
            2,
            "E",
            DeclKind::Error(ErrorDecl {
                default_status: Some(404),
                fields: model,
            }),
        ),
        decl(
            3,
            "En",
            DeclKind::Enum(Enum {
                variants: vec![variant],
            }),
        ),
        decl(
            4,
            "U",
            DeclKind::Union(Union {
                discriminator: "kind".into(),
                arms: vec![UnionArm {
                    label: "a".into(),
                    decl: DeclId(1),
                }],
            }),
        ),
        decl(
            5,
            "C",
            DeclKind::Const(Const {
                ty: Type::Scalar(Scalar::Int),
                value: Literal::Int(1),
            }),
        ),
    ]
}

fn endpoint(method: Method, binding: Binding, auth: AuthKind) -> Endpoint {
    Endpoint {
        name: "op".into(),
        method,
        path: "/x".into(),
        sig: "veld:p/S.op@0".into(),
        docs: Some("d".into()),
        deprecated: Some("x".into()),
        auth: Auth {
            kind: auth,
            header: Some("X-Key".into()),
        },
        middleware: vec!["m".into()],
        params: vec![Param {
            name: "p".into(),
            binding,
            header: Some("H".into()),
            ty: Type::Scalar(Scalar::String),
            optional: true,
            stream: true,
            constraints: Constraints::default(),
        }],
        success: Success {
            status: 200,
            ty: Some(Type::Decl(DeclId(1))),
            stream: false,
        },
        errors: vec![ErrorResponse {
            status: 404,
            decl: DeclId(2),
        }],
        origin: origin(),
    }
}

const METHODS: [Method; 8] = [
    Method::Get,
    Method::Post,
    Method::Put,
    Method::Patch,
    Method::Delete,
    Method::Head,
    Method::Options,
    Method::Ws,
];
const BINDINGS: [Binding; 5] = [
    Binding::Path,
    Binding::Query,
    Binding::QueryObject,
    Binding::Header,
    Binding::Body,
];
const AUTHS: [AuthKind; 4] = [
    AuthKind::None,
    AuthKind::Bearer,
    AuthKind::Basic,
    AuthKind::ApiKey,
];
const KINDS: [PackageKind; 4] = [
    PackageKind::Service,
    PackageKind::Library,
    PackageKind::Registry,
    PackageKind::Prelude,
];

/// Adding a variant to one of these enums breaks this function, which points to the lists above.
fn exhaustive() {
    match Method::Get {
        Method::Get
        | Method::Post
        | Method::Put
        | Method::Patch
        | Method::Delete
        | Method::Head
        | Method::Options
        | Method::Ws => {}
    }
    match Binding::Path {
        Binding::Path | Binding::Query | Binding::QueryObject | Binding::Header | Binding::Body => {
        }
    }
    match AuthKind::None {
        AuthKind::None | AuthKind::Bearer | AuthKind::Basic | AuthKind::ApiKey => {}
    }
    match PackageKind::Service {
        PackageKind::Service
        | PackageKind::Library
        | PackageKind::Registry
        | PackageKind::Prelude => {}
    }
}

/// An IR that uses every variant of every enum and fills every optional field.
fn full_ir() -> Ir {
    exhaustive();
    let endpoints = METHODS
        .into_iter()
        .enumerate()
        .map(|(i, m)| endpoint(m, BINDINGS[i % BINDINGS.len()], AUTHS[i % AUTHS.len()]))
        .collect();
    let packages = KINDS
        .into_iter()
        .enumerate()
        .map(|(i, kind)| Package {
            id: PackageId(i as u32),
            name: format!("p{i}"),
            kind,
        })
        .collect();
    Ir {
        ir_version: IR_VERSION.into(),
        packages,
        decls: decls(),
        services: vec![Service {
            id: ServiceId(0),
            package: PackageId(0),
            name: "S".into(),
            prefix: "/".into(),
            base_url: Some("http://x".into()),
            docs: Some("d".into()),
            auth: Auth::none(),
            middleware: vec!["m".into()],
            endpoints,
            origin: origin(),
        }],
        consumes: [("S".to_string(), vec!["T".to_string()])]
            .into_iter()
            .collect(),
    }
}

#[test]
fn a_sample_using_every_variant_matches_the_schema() {
    let value = serde_json::to_value(full_ir()).unwrap();
    check("Ir", &value).unwrap();
}

#[test]
fn the_schema_lists_every_string_variant_of_the_rust_enums() {
    let names = |def: &str| -> Vec<String> {
        match find(def) {
            Some(Def::Strings(_, v)) => v.iter().map(|s| (*s).to_string()).collect(),
            other => panic!("{def}: {other:?}"),
        }
    };
    let scalars: Vec<String> = Scalar::ALL.iter().map(|s| s.name().to_string()).collect();
    assert_eq!(names("Scalar"), scalars);
    let features: Vec<String> = Feature::ALL.iter().map(|f| f.name().to_string()).collect();
    assert_eq!(names("Feature"), features);
    let methods: Vec<String> = METHODS.iter().map(|m| m.as_str().to_string()).collect();
    assert_eq!(names("Method"), methods);
    for (def, values) in [
        (
            "Binding",
            BINDINGS
                .iter()
                .map(|v| serde_json::to_value(v).unwrap())
                .collect::<Vec<_>>(),
        ),
        (
            "AuthKind",
            AUTHS
                .iter()
                .map(|v| serde_json::to_value(v).unwrap())
                .collect(),
        ),
        (
            "PackageKind",
            KINDS
                .iter()
                .map(|v| serde_json::to_value(v).unwrap())
                .collect(),
        ),
    ] {
        let want: Vec<String> = values
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(names(def), want, "{def}");
    }
    for n in ["email", "uri", "hostname", "ipv4", "ipv6"] {
        assert!(StringFormat::from_name(n).is_some(), "{n}");
    }
    assert_eq!(
        names("StringFormat"),
        ["email", "uri", "hostname", "ipv4", "ipv6"]
    );
}

#[test]
fn drift_is_reported_with_the_path_of_the_mismatch() {
    let good = serde_json::to_value(full_ir()).unwrap();
    let mut extra = good.clone();
    extra["decls"][1]["fields"][0]["brand_new"] = json!(1);
    let err = check("Ir", &extra).unwrap_err();
    assert!(
        err.contains("Ir.decls[1].fields[0]") && err.contains("unknown key \"brand_new\""),
        "{err}"
    );
    let mut missing = good.clone();
    missing["services"][0]["endpoints"][0]
        .as_object_mut()
        .unwrap()
        .remove("sig");
    assert!(
        check("Ir", &missing)
            .unwrap_err()
            .contains("missing key \"sig\"")
    );
    let mut wrong_type = good;
    wrong_type["services"][0]["name"] = json!(3);
    assert!(
        check("Ir", &wrong_type)
            .unwrap_err()
            .contains("expected a string")
    );
}

#[test]
fn protocol_messages_validate() {
    check(
        "HostHello",
        &json!({"type":"hello","protocol":"1","veld_version":"2.0.0"}),
    )
    .unwrap();
    let hello = json!({"type":"hello","protocol":"1","name":"k","version":"1","roles":["client"],"capabilities":["auth"]});
    check("PluginHello", &hello).unwrap();
    check(
        "GenerateResult",
        &json!({"type":"result","files":[{"path":"a","content":"b"}]}),
    )
    .unwrap();
    assert!(
        check(
            "GenerateResult",
            &json!({"type":"result","files":[{"path":"a"}]})
        )
        .is_err()
    );
    let ir = serde_json::to_value(full_ir()).unwrap();
    let request = json!({"type":"generate","role":"client","ir":ir,"services":[0],"options":{},"consumes":[]});
    check("GenerateRequest", &request).unwrap();
}

#[test]
fn typescript_is_deterministic_and_complete() {
    let ts = typescript();
    assert_eq!(ts, typescript());
    for needle in [
        "export interface Ir {",
        "export type DeclId = number;",
        "export type Scalar = \"string\" | \"int\"",
        "  | { scalar: Scalar }",
        "  | { lit: \"null\" }",
        "      kind: \"model\";",
        "      sig: string;",
        "  options_schema?: unknown;",
        "  consumes: Record<string, string[]>;",
        "  default_status: number | null;",
        "export interface GenerateRequest {",
    ] {
        assert!(ts.contains(needle), "missing `{needle}` in:\n{ts}");
    }
    for def in DEFS {
        let name = def.name();
        assert!(
            ts.contains(&format!("export interface {name} "))
                || ts.contains(&format!("export type {name} ")),
            "{name}"
        );
    }
}

#[test]
fn definitions_are_unique_and_every_reference_resolves() {
    let mut seen = std::collections::BTreeSet::new();
    for d in DEFS {
        assert!(seen.insert(d.name()), "duplicate definition {}", d.name());
    }
    // `check` reports an undefined reference as an error, so a clean run proves they all resolve
    // for every type the sample reaches.
    let value: Value = serde_json::to_value(full_ir()).unwrap();
    assert!(check("Ir", &value).is_ok());
}
