//! One table-driven case per diff rule.

use super::ChangeKind::{self, *};
use super::Severity::{self, Breaking as B, NonBreaking as N};
use super::{diff, has_breaking};
use crate::testutil::ir;

struct Case {
    name: &'static str,
    old: &'static str,
    new: &'static str,
    expect: &'static [(ChangeKind, Severity)],
}

fn run(old: &str, new: &str) -> Vec<(ChangeKind, Severity)> {
    let mut v: Vec<_> = diff(&ir(old), &ir(new))
        .into_iter()
        .map(|c| (c.kind, c.severity))
        .collect();
    v.sort();
    v
}

fn check(cases: &[Case]) {
    for c in cases {
        let mut want = c.expect.to_vec();
        want.sort();
        assert_eq!(run(c.old, c.new), want, "case `{}`", c.name);
    }
}

// Shapes used in requests (`body`) and in responses (return type).
const REQ: &str = "service S at \"/s\" { post \"/\" make(body: M) }";
const RESP: &str = "service S at \"/s\" { get \"/\" fetch() -> M }";

#[test]
fn identical_contracts_have_no_changes() {
    let src = "model M { a: int }\nservice S at \"/s\" { get \"/\" fetch() -> M | NotFound }";
    assert!(run(src, src).is_empty());
}

#[test]
fn declaration_rules() {
    check(&[
        Case {
            name: "removed decl",
            old: "model A { a: int }\nmodel Gone { a: int }\nservice S at \"/s\" { get \"/\" f() -> A }",
            new: "model A { a: int }\nservice S at \"/s\" { get \"/\" f() -> A }",
            expect: &[(DeclRemoved, B)],
        },
        Case {
            name: "renamed decl is a removal plus an addition",
            old: "model Old { a: int }",
            new: "model New { a: int }",
            expect: &[(DeclRemoved, B), (DeclAdded, N)],
        },
        Case {
            name: "added decl",
            old: "model A { a: int }",
            new: "model A { a: int }\nmodel B { b: int }",
            expect: &[(DeclAdded, N)],
        },
        Case {
            name: "decl kind changed",
            old: "model A { a: int }",
            new: "enum A { x, y }",
            expect: &[(DeclKindChanged, B)],
        },
    ]);
}

fn with(model: &str, svc: &str) -> String {
    format!("{model}\n{svc}")
}

#[test]
fn field_rules() {
    let (req, resp) = (REQ, RESP);
    let cases = [
        (
            "removed (request)",
            "model M { a: int, b: int }",
            "model M { a: int }",
            req,
            vec![(FieldRemoved, B)],
        ),
        (
            "removed (response)",
            "model M { a: int, b: int }",
            "model M { a: int }",
            resp,
            vec![(FieldRemoved, B)],
        ),
        (
            "renamed is removed plus added",
            "model M { a: int }",
            "model M { b: int }",
            resp,
            vec![(FieldRemoved, B), (FieldAdded, N)],
        ),
        (
            "type changed",
            "model M { a: int }",
            "model M { a: string }",
            resp,
            vec![(FieldTypeChanged, B)],
        ),
        (
            "list element changed",
            "model M { a: int[] }",
            "model M { a: long[] }",
            resp,
            vec![(FieldTypeChanged, B)],
        ),
        (
            "optional field added",
            "model M { a: int }",
            "model M { a: int, b?: int }",
            req,
            vec![(FieldAdded, N)],
        ),
        (
            "required field added (response)",
            "model M { a: int }",
            "model M { a: int, b: int }",
            resp,
            vec![(FieldAdded, N)],
        ),
        (
            "required field added (request)",
            "model M { a: int }",
            "model M { a: int, b: int }",
            req,
            vec![(RequiredFieldAdded, B)],
        ),
        (
            "defaulted field added (request)",
            "model M { a: int }",
            "model M { a: int, b: int = 1 }",
            req,
            vec![(FieldAdded, N)],
        ),
        (
            "optional to required (request)",
            "model M { a?: int }",
            "model M { a: int }",
            req,
            vec![(FieldBecameRequired, B)],
        ),
        (
            "optional to required (response)",
            "model M { a?: int }",
            "model M { a: int }",
            resp,
            vec![(FieldBecameRequired, N)],
        ),
        (
            "required to optional (response)",
            "model M { a: int }",
            "model M { a?: int }",
            resp,
            vec![(FieldBecameOptional, B)],
        ),
        (
            "required to optional (request)",
            "model M { a: int }",
            "model M { a?: int }",
            req,
            vec![(FieldBecameOptional, N)],
        ),
        (
            "default removed makes it required (request)",
            "model M { a: int = 1 }",
            "model M { a: int }",
            req,
            vec![(FieldBecameRequired, B)],
        ),
    ];
    for (name, old, new, svc, expect) in cases {
        let mut want = expect;
        want.sort();
        assert_eq!(run(&with(old, svc), &with(new, svc)), want, "case `{name}`");
    }
}

#[test]
fn enum_rules() {
    let both = "service S at \"/s\" { post \"/\" make(body: M) -> M }";
    let cases = [
        (
            "removed (response)",
            RESP,
            "enum E { a, b }",
            "enum E { a }",
            vec![(EnumVariantRemoved, B)],
        ),
        (
            "removed (request)",
            REQ,
            "enum E { a, b }",
            "enum E { a }",
            vec![(EnumVariantRemoved, B)],
        ),
        (
            "added (response only)",
            RESP,
            "enum E { a }",
            "enum E { a, b }",
            vec![(EnumVariantAdded, B)],
        ),
        (
            "added (request)",
            REQ,
            "enum E { a }",
            "enum E { a, b }",
            vec![(EnumVariantAdded, N)],
        ),
        (
            "added (request and response)",
            both,
            "enum E { a }",
            "enum E { a, b }",
            vec![(EnumVariantAdded, N)],
        ),
        (
            "wire value changed",
            RESP,
            "enum E { a }",
            "enum E { a = \"A\" }",
            vec![(EnumVariantRemoved, B), (EnumVariantAdded, B)],
        ),
        (
            "source rename keeping the wire value",
            RESP,
            "enum E { a = \"A\" }",
            "enum E { b = \"A\" }",
            vec![],
        ),
    ];
    for (name, svc, old, new, expect) in cases {
        let model = |e: &str| format!("{e}\nmodel M {{ e: E }}\n{svc}");
        let mut want = expect;
        want.sort();
        assert_eq!(run(&model(old), &model(new)), want, "case `{name}`");
    }
}

#[test]
fn union_rules() {
    let base = "model A { x: int }\nmodel B { y: int }\nmodel C { z: int }\n";
    let wrap = |u: &str, svc: &str| format!("{base}{u}\nmodel M {{ u: U }}\n{svc}");
    let cases = [
        (
            "arm removed",
            RESP,
            "union U { a: A, b: B }",
            "union U { a: A }",
            vec![(UnionArmRemoved, B)],
        ),
        (
            "arm added (response)",
            RESP,
            "union U { a: A }",
            "union U { a: A, b: B }",
            vec![(UnionArmAdded, B)],
        ),
        (
            "arm added (request)",
            REQ,
            "union U { a: A }",
            "union U { a: A, b: B }",
            vec![(UnionArmAdded, N)],
        ),
        (
            "arm retargeted",
            RESP,
            "union U { a: A }",
            "union U { a: C }",
            vec![(UnionArmChanged, B)],
        ),
        (
            "discriminator changed",
            RESP,
            "union U { a: A }",
            "@discriminator(\"type\")\nunion U { a: A }",
            vec![(DiscriminatorChanged, B)],
        ),
    ];
    for (name, svc, old, new, expect) in cases {
        let mut want = expect;
        want.sort();
        assert_eq!(run(&wrap(old, svc), &wrap(new, svc)), want, "case `{name}`");
    }
}

#[test]
fn endpoint_rules() {
    check(&[
        Case {
            name: "endpoint removed",
            old: "service S at \"/s\" { get \"/a\" a()\n get \"/b\" b() }",
            new: "service S at \"/s\" { get \"/a\" a() }",
            expect: &[(EndpointRemoved, B)],
        },
        Case {
            name: "endpoint renamed",
            old: "service S at \"/s\" { get \"/a\" a() }",
            new: "service S at \"/s\" { get \"/a\" b() }",
            expect: &[(EndpointRemoved, B), (EndpointAdded, N)],
        },
        Case {
            name: "endpoint added",
            old: "service S at \"/s\" { get \"/a\" a() }",
            new: "service S at \"/s\" { get \"/a\" a()\n get \"/b\" b() }",
            expect: &[(EndpointAdded, N)],
        },
        Case {
            name: "method changed",
            old: "service S at \"/s\" { get \"/a\" a() }",
            new: "service S at \"/s\" { delete \"/a\" a() }",
            expect: &[(MethodChanged, B)],
        },
        Case {
            name: "path changed",
            old: "service S at \"/s\" { get \"/a\" a() }",
            new: "service S at \"/s\" { get \"/b\" a() }",
            expect: &[(PathChanged, B)],
        },
        Case {
            name: "prefix changed",
            old: "service S at \"/s\" { get \"/a\" a() }",
            new: "service S at \"/t\" { get \"/a\" a() }",
            expect: &[(PathChanged, B)],
        },
    ]);
}

#[test]
fn param_rules() {
    check(&[
        Case {
            name: "required param added",
            old: "service S at \"/s\" { get \"/\" a() }",
            new: "service S at \"/s\" { get \"/\" a(@query q: string) }",
            expect: &[(ParamAdded, B), (ErrorAdded, B)],
        },
        Case {
            name: "optional param added",
            old: "service S at \"/s\" { get \"/\" a(@query p: string) }",
            new: "service S at \"/s\" { get \"/\" a(@query p: string, @query q?: string) }",
            expect: &[(ParamAdded, N)],
        },
        Case {
            name: "param removed",
            old: "service S at \"/s\" { get \"/\" a(@query p: string, @query q?: string) }",
            new: "service S at \"/s\" { get \"/\" a(@query p: string) }",
            expect: &[(ParamRemoved, B)],
        },
        Case {
            name: "binding changed",
            old: "service S at \"/s\" { get \"/\" a(@query q: string) }",
            new: "service S at \"/s\" { get \"/\" a(@header(\"q\") q: string) }",
            expect: &[(ParamBindingChanged, B)],
        },
        Case {
            name: "header name changed",
            old: "service S at \"/s\" { get \"/\" a(@header(\"X-A\") q: string) }",
            new: "service S at \"/s\" { get \"/\" a(@header(\"X-B\") q: string) }",
            expect: &[(ParamBindingChanged, B)],
        },
        Case {
            name: "param type changed",
            old: "service S at \"/s\" { get \"/{id}\" a(id: string) }",
            new: "service S at \"/s\" { get \"/{id}\" a(id: uuid) }",
            expect: &[(ParamTypeChanged, B)],
        },
        Case {
            name: "param became required",
            old: "service S at \"/s\" { get \"/\" a(@query q?: string) }",
            new: "service S at \"/s\" { get \"/\" a(@query q: string) }",
            expect: &[(ParamBecameRequired, B)],
        },
        Case {
            name: "param became optional",
            old: "service S at \"/s\" { get \"/\" a(@query q: string) }",
            new: "service S at \"/s\" { get \"/\" a(@query q?: string) }",
            expect: &[(ParamBecameOptional, N)],
        },
        Case {
            name: "path parameter rename keeps the wire route but renames the parameter",
            old: "service S at \"/s\" { get \"/{id}\" a(id: string) }",
            new: "service S at \"/s\" { get \"/{key}\" a(key: string) }",
            expect: &[(ParamRemoved, B), (ParamAdded, B)],
        },
    ]);
}

#[test]
fn response_rules() {
    check(&[
        Case {
            name: "success status changed",
            old: "model M { a: int }\nservice S at \"/s\" { post \"/\" a() -> 201 M }",
            new: "model M { a: int }\nservice S at \"/s\" { post \"/\" a() -> 200 M }",
            expect: &[(SuccessChanged, B)],
        },
        Case {
            name: "success type changed",
            old: "model M { a: int }\nmodel N { a: int }\nservice S at \"/s\" { get \"/\" a() -> M }",
            new: "model M { a: int }\nmodel N { a: int }\nservice S at \"/s\" { get \"/\" a() -> N }",
            expect: &[(SuccessChanged, B)],
        },
        Case {
            name: "success body removed",
            old: "model M { a: int }\nservice S at \"/s\" { get \"/\" a() -> M }",
            new: "model M { a: int }\nservice S at \"/s\" { get \"/\" a() -> 200 }",
            expect: &[(SuccessChanged, B)],
        },
        Case {
            name: "declared error removed",
            old: "service S at \"/s\" { get \"/\" a() -> 204 | NotFound | Conflict }",
            new: "service S at \"/s\" { get \"/\" a() -> 204 | NotFound }",
            expect: &[(ErrorRemoved, B)],
        },
        Case {
            name: "declared error added",
            old: "service S at \"/s\" { get \"/\" a() -> 204 | NotFound }",
            new: "service S at \"/s\" { get \"/\" a() -> 204 | NotFound | Conflict }",
            expect: &[(ErrorAdded, B)],
        },
        Case {
            name: "error status changed at the use site",
            old: "service S at \"/s\" { get \"/\" a() -> 204 | NotFound }",
            new: "service S at \"/s\" { get \"/\" a() -> 204 | 410 NotFound }",
            expect: &[(ErrorRemoved, B), (ErrorAdded, B)],
        },
    ]);
}

#[test]
fn auth_rules() {
    check(&[
        Case {
            name: "auth added",
            old: "service S at \"/s\" { get \"/\" a() }",
            new: "@auth(bearer)\nservice S at \"/s\" { get \"/\" a() }",
            expect: &[(AuthAdded, B), (ErrorAdded, B)],
        },
        Case {
            name: "auth kind changed",
            old: "@auth(bearer)\nservice S at \"/s\" { get \"/\" a() }",
            new: "@auth(basic)\nservice S at \"/s\" { get \"/\" a() }",
            expect: &[(AuthChanged, B)],
        },
        Case {
            name: "api key header changed",
            old: "@auth(apiKey(header: \"X-A\"))\nservice S at \"/s\" { get \"/\" a() }",
            new: "@auth(apiKey(header: \"X-B\"))\nservice S at \"/s\" { get \"/\" a() }",
            expect: &[(AuthChanged, B)],
        },
        Case {
            name: "auth removed",
            old: "@auth(bearer)\nservice S at \"/s\" { get \"/\" a() }",
            new: "service S at \"/s\" { get \"/\" a() }",
            expect: &[(AuthRemoved, N), (ErrorRemoved, B)],
        },
        Case {
            name: "endpoint override added",
            old: "@auth(bearer)\nservice S at \"/s\" { get \"/\" a() }",
            new: "@auth(bearer)\nservice S at \"/s\" { @auth(none)\n get \"/\" a() }",
            expect: &[(AuthRemoved, N), (ErrorRemoved, B)],
        },
    ]);
}

#[test]
fn constraint_rules() {
    let (req, resp) = (REQ, RESP);
    let cases = [
        (
            "min raised",
            "model M { a: int @min(1) }",
            "model M { a: int @min(2) }",
            vec![(ConstraintTightened, B)],
        ),
        (
            "min lowered",
            "model M { a: int @min(2) }",
            "model M { a: int @min(1) }",
            vec![(ConstraintLoosened, N)],
        ),
        (
            "max lowered",
            "model M { a: int @max(9) }",
            "model M { a: int @max(5) }",
            vec![(ConstraintTightened, B)],
        ),
        (
            "max raised",
            "model M { a: int @max(5) }",
            "model M { a: int @max(9) }",
            vec![(ConstraintLoosened, N)],
        ),
        (
            "bound added",
            "model M { a: int }",
            "model M { a: int @max(5) }",
            vec![(ConstraintTightened, B)],
        ),
        (
            "bound removed",
            "model M { a: int @max(5) }",
            "model M { a: int }",
            vec![(ConstraintLoosened, N)],
        ),
        (
            "length tightened",
            "model M { a: string @length(min: 1, max: 9) }",
            "model M { a: string @length(min: 2, max: 9) }",
            vec![(ConstraintTightened, B)],
        ),
        (
            "length loosened",
            "model M { a: string @length(max: 5) }",
            "model M { a: string @length(max: 9) }",
            vec![(ConstraintLoosened, N)],
        ),
        (
            "pattern added",
            "model M { a: string }",
            "model M { a: string @pattern(\"^a$\") }",
            vec![(ConstraintTightened, B)],
        ),
        (
            "pattern removed",
            "model M { a: string @pattern(\"^a$\") }",
            "model M { a: string }",
            vec![(ConstraintLoosened, N)],
        ),
        (
            "format added",
            "model M { a: string }",
            "model M { a: string @format(email) }",
            vec![(ConstraintTightened, B)],
        ),
    ];
    for (name, old, new, expect) in cases {
        for svc in [req, resp] {
            let mut want = expect.clone();
            want.sort();
            assert_eq!(run(&with(old, svc), &with(new, svc)), want, "case `{name}`");
        }
    }
}

#[test]
fn param_constraint_rules() {
    check(&[
        Case {
            name: "param constraint tightened",
            old: "service S at \"/s\" { get \"/\" a(@query @min(1) n: int) }",
            new: "service S at \"/s\" { get \"/\" a(@query @min(5) n: int) }",
            expect: &[(ConstraintTightened, B)],
        },
        Case {
            name: "param constraint loosened",
            old: "service S at \"/s\" { get \"/\" a(@query @min(5) n: int) }",
            new: "service S at \"/s\" { get \"/\" a(@query @min(1) n: int) }",
            expect: &[(ConstraintLoosened, N)],
        },
    ]);
}

#[test]
fn non_breaking_rules() {
    check(&[
        Case {
            name: "docs changed",
            old: "/// old\nmodel M { a: int }",
            new: "/// new\nmodel M { a: int }",
            expect: &[(DocsChanged, N)],
        },
        Case {
            name: "field docs changed",
            old: "model M { /// x\n a: int }",
            new: "model M { /// y\n a: int }",
            expect: &[(DocsChanged, N)],
        },
        Case {
            name: "endpoint docs changed",
            old: "service S at \"/s\" { /// x\n get \"/\" a() }",
            new: "service S at \"/s\" { /// y\n get \"/\" a() }",
            expect: &[(DocsChanged, N)],
        },
        Case {
            name: "decl deprecated",
            old: "model M { a: int }",
            new: "@deprecated(\"no\")\nmodel M { a: int }",
            expect: &[(Deprecated, N)],
        },
        Case {
            name: "field deprecated",
            old: "model M { a: int }",
            new: "model M { a: int @deprecated(\"no\") }",
            expect: &[(Deprecated, N)],
        },
        Case {
            name: "endpoint deprecated",
            old: "service S at \"/s\" { get \"/\" a() }",
            new: "service S at \"/s\" { @deprecated(\"no\")\n get \"/\" a() }",
            expect: &[(Deprecated, N)],
        },
        Case {
            name: "deprecation lifted",
            old: "@deprecated(\"no\")\nmodel M { a: int }",
            new: "model M { a: int }",
            expect: &[(Undeprecated, N)],
        },
        Case {
            name: "constant value changed",
            old: "const MAX: int = 1",
            new: "const MAX: int = 2",
            expect: &[(ConstChanged, N)],
        },
        Case {
            name: "constant type changed",
            old: "const MAX: int = 1",
            new: "const MAX: float = 1.0",
            expect: &[(ConstChanged, B)],
        },
        Case {
            name: "middleware and readonly are not wire changes",
            old: "model M { a: int }\n@middleware(A)\nservice S at \"/s\" { get \"/\" a() -> M }",
            new: "model M { a: int @readonly }\n@middleware(B)\nservice S at \"/s\" { get \"/\" a() -> M }",
            expect: &[],
        },
    ]);
}

#[test]
fn messages_name_the_element_and_subject_is_the_qualified_name_or_op() {
    let old = ir("model M { a: int, b: int }\nservice S at \"/s\" { get \"/x\" fetch() -> M }");
    let new = ir("model M { a: int }\nservice S at \"/s\" { get \"/y\" fetch() -> M }");
    let changes = diff(&old, &new);
    assert!(has_breaking(&changes));
    let f = changes.iter().find(|c| c.kind == FieldRemoved);
    assert_eq!(f.map(|c| c.subject.as_str()), Some("app:app.M"));
    assert!(f.is_some_and(|c| c.message.contains("`b`")));
    let p = changes.iter().find(|c| c.kind == PathChanged);
    assert_eq!(p.map(|c| c.subject.as_str()), Some("veld:app/S.fetch"));
}

#[test]
fn transitive_changes_are_reported_on_the_declaration_that_changed() {
    let old = ir(
        "model Inner { a: int }\nmodel Outer { i: Inner }\nservice S at \"/s\" { get \"/\" f() -> Outer }",
    );
    let new = ir(
        "model Inner { a: string }\nmodel Outer { i: Inner }\nservice S at \"/s\" { get \"/\" f() -> Outer }",
    );
    let changes = diff(&old, &new);
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert_eq!(changes[0].subject, "app:app.Inner");
}

#[test]
fn renamed_endpoint_message_points_at_the_new_name() {
    let old = ir("service S at \"/s\" { get \"/a\" a() }");
    let new = ir("service S at \"/s\" { get \"/a\" b() }");
    let msg = diff(&old, &new)
        .into_iter()
        .find(|c| c.kind == EndpointRemoved)
        .map(|c| c.message);
    assert!(msg.is_some_and(|m| m.contains("now served by `b`")));
}

#[test]
fn changes_come_out_in_a_stable_order() {
    let old =
        ir("model A { a: int }\nmodel B { b: int }\nservice S at \"/s\" { get \"/\" f() -> A }");
    let new = ir(
        "model A { a: string }\nmodel B { b: string }\nservice S at \"/s\" { get \"/\" f() -> A }",
    );
    let a = diff(&old, &new);
    let b = diff(&old, &new);
    assert_eq!(a, b);
    assert!(a.windows(2).all(|w| w[0].subject <= w[1].subject));
}
