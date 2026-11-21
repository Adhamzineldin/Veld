//! Unit tests for the semantic passes, driven through the public pipeline.

use crate::testing::{codes, ir, multi, one};

mod resolve {
    use super::*;

    #[test]
    fn unknown_type_is_e0205_with_import_hint() {
        let out = multi(&[
            ("app", "a.veld", "model A { x: B }"),
            ("app", "b.veld", "model B { y: int }"),
        ]);
        assert_eq!(out.diagnostics[0].code, "E0205");
        assert_eq!(out.diagnostics[0].help.as_deref(), Some("add `use b.{B}`"));
    }

    #[test]
    fn imports_resolve_across_modules_and_packages() {
        let out = multi(&[
            (
                "app",
                "a.veld",
                "use b.{B}\nuse lib:c.{C as Cc}\nmodel A { b: B, c: Cc }",
            ),
            ("app", "b.veld", "model B { y: int }"),
            ("lib", "c.veld", "model C { z: int }"),
        ]);
        assert!(out.ir.is_some(), "{}", out.render());
    }

    #[test]
    fn module_import_cycles_are_allowed() {
        let out = multi(&[
            ("app", "a.veld", "use b.{B}\nmodel A { b?: B }"),
            ("app", "b.veld", "use a.{A}\nmodel B { a?: A }"),
        ]);
        assert!(out.ir.is_some(), "{}", out.render());
    }

    #[test]
    fn import_errors_e0202_to_e0205() {
        let b = ("lib", "b.veld", "model B { y: int }");
        let first = |a: &'static str| multi(&[("app", "a.veld", a), b]).diagnostics[0].code;
        assert_eq!(first("use lib:b.{B}\nmodel B { a: int }"), "E0202");
        assert_eq!(first("use nope:b.{B}\nmodel A { a: int }"), "E0203");
        assert_eq!(first("use lib:zz.{B}\nmodel A { a: int }"), "E0204");
        assert_eq!(first("use lib:b.{Nope}\nmodel A { a: int }"), "E0205");
    }

    #[test]
    fn two_imports_with_the_same_local_name_collide() {
        let out = multi(&[
            (
                "app",
                "a.veld",
                "use b.{Dup}\nuse lib:c.{Dup}\nmodel A { d: Dup }",
            ),
            ("app", "b.veld", "model Dup { y: int }"),
            ("lib", "c.veld", "model Dup { y: int }"),
        ]);
        assert_eq!(out.diagnostics[0].code, "E0202");
    }

    #[test]
    fn unused_import_is_w0201_and_used_import_is_not() {
        let out = multi(&[
            ("app", "a.veld", "use b.{B, C}\nmodel A { b: B }"),
            ("app", "b.veld", "model B { y: int }\nmodel C { y: int }"),
        ]);
        let c: Vec<_> = out.diagnostics.iter().map(|d| d.code).collect();
        assert_eq!(c, vec!["W0201"]);
        assert!(out.ir.is_some(), "warnings do not block the IR");
    }

    #[test]
    fn prelude_is_in_scope_and_shadowing_warns() {
        assert!(codes("service S at \"/\" { get \"/\" f() -> NotFound }").is_empty());
        let c = codes(
            "error NotFound 404 { m: string }\nservice S at \"/\" { get \"/\" f() -> NotFound }",
        );
        assert_eq!(c, vec!["W0202"]);
    }

    #[test]
    fn duplicate_declaration_in_package_is_e0201_even_across_modules() {
        let out = multi(&[
            ("app", "a.veld", "model X { a: int }"),
            ("app", "b.veld", "model X { a: int }"),
        ]);
        assert_eq!(out.diagnostics[0].code, "E0201");
        assert_eq!(one("model string { a: int }").diagnostics[0].code, "E0201");
        assert_eq!(
            codes("model X { a: int }\nservice X at \"/\" { }"),
            vec!["E0201"]
        );
    }

    #[test]
    fn generics_map_page_and_misuse() {
        assert!(codes("model A { m: Map<string, int[]>, p?: Page<A> }").is_empty());
        assert_eq!(codes("model A { m: Map<int, int> }"), vec!["E0301"]);
        assert_eq!(
            codes("model B { x: int }\nmodel A { m: B<int> }"),
            vec!["E0302"]
        );
        assert_eq!(codes("model A { m: Map<string> }"), vec!["E0302"]);
        assert_eq!(codes("model A { m: string<int> }"), vec!["E0302"]);
        assert_eq!(codes("model A { m: Page }"), vec!["E0302"]);
    }

    #[test]
    fn defaults_are_type_checked() {
        assert!(codes("enum S { a, b }\nmodel A { x: int = 1, y: S = .a, z: string[] = [], w: float = 2, b: bool = true }").is_empty());
        assert_eq!(codes("model A { x: int = \"s\" }"), vec!["E0303"]);
        assert_eq!(codes("model A { x: int = 99999999999 }"), vec!["E0303"]);
        assert_eq!(
            codes("enum S { a }\nmodel A { y: S = .zzz }"),
            vec!["E0304"]
        );
        assert_eq!(
            codes("model B { x: int }\nmodel A { y: B = 1 }"),
            vec!["E0303"]
        );
        assert_eq!(codes("model A { x?: int = 1 }"), vec!["E0303"]);
        assert_eq!(codes("model A { x: string = null }"), vec!["E0303"]);
    }

    #[test]
    fn enum_rules() {
        assert_eq!(codes("enum S { a = \"X\", b = \"X\" }"), vec!["E0403"]);
        assert_eq!(codes("enum S { a, a = \"Z\" }"), vec!["E0410"]);
    }

    #[test]
    fn error_rules() {
        assert_eq!(codes("error E 200 { a: int }"), vec!["E0406"]);
        assert_eq!(
            codes("error E 404 { a: int }\nmodel M { e: E }"),
            vec!["E0405"]
        );
    }

    #[test]
    fn duplicate_fields_and_wires_are_e0410() {
        assert_eq!(codes("model A { a: int, a: string }"), vec!["E0410"]);
        assert_eq!(
            codes("model A { a: int @wire(\"x\"), b: int @wire(\"x\") }"),
            vec!["E0410"]
        );
    }

    #[test]
    fn extends_and_union_arm_kind_errors() {
        assert_eq!(
            codes("enum E { a }\nmodel A extends E { x: int }"),
            vec!["E0408"]
        );
        assert_eq!(codes("enum E { a }\nunion U { e: E }"), vec!["E0409"]);
        assert_eq!(codes("union U { }"), vec!["E0409"]);
        assert_eq!(
            codes("model A { a: int }\nunion U { x: A, x: A }"),
            vec!["E0410"]
        );
    }

    #[test]
    fn const_values_are_checked() {
        assert!(codes("const MAX: int = 5\nconst NAME: string = \"x\"").is_empty());
        assert_eq!(codes("const MAX: int = \"x\""), vec!["E0303"]);
    }

    #[test]
    fn consumes_validation() {
        use crate::{CompileInput, PackageInput, SourceFile, compile};
        use std::collections::BTreeMap;
        let pkg = |n: &str| PackageInput {
            name: n.to_string(),
            kind: veld_ir::PackageKind::Service,
            files: vec![SourceFile {
                path: "a.veld".into(),
                text: "model M { a: int }".into(),
            }],
            base_url: None,
        };
        let mut consumes = BTreeMap::new();
        consumes.insert(
            "a".to_string(),
            vec!["a".to_string(), "zzz".to_string(), "b".to_string()],
        );
        consumes.insert("nope".to_string(), vec!["a".to_string()]);
        let out = compile(&CompileInput {
            packages: vec![pkg("a"), pkg("b")],
            consumes,
        });
        let mut c: Vec<_> = out.diagnostics.iter().map(|d| d.code).collect();
        c.sort_unstable();
        assert_eq!(c, vec!["E0702", "E0702", "W0701"]);
    }
}

mod inherit {
    use super::*;

    #[test]
    fn fields_flatten_parent_first_with_inherited_from() {
        let ir =
            ir("model A { a: int }\nmodel B extends A { b: int }\nmodel C extends B { c: int }");
        let c = ir
            .decls
            .iter()
            .find(|d| d.name == "C")
            .map(|d| d.fields().to_vec())
            .unwrap_or_default();
        let names: Vec<&str> = c.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
        let a_id = ir.decls.iter().find(|d| d.name == "A").map(|d| d.id);
        assert_eq!(
            c[0].inherited_from, a_id,
            "grandparent field points at its declarer"
        );
        assert!(c[2].inherited_from.is_none());
    }

    #[test]
    fn redeclaring_an_inherited_field_is_e0401() {
        assert_eq!(
            codes("model A { a: int }\nmodel B extends A { a: string }"),
            vec!["E0401"]
        );
        assert_eq!(
            codes("model A { a: int }\nmodel B extends A { b: int @wire(\"a\") }"),
            vec!["E0401"]
        );
    }

    #[test]
    fn inheritance_cycles_are_e0402() {
        assert_eq!(
            codes("model A extends B { a: int }\nmodel B extends A { b: int }"),
            vec!["E0402", "E0402"]
        );
        assert_eq!(codes("model A extends A { a: int }"), vec!["E0402"]);
    }
}

mod typeck {
    use super::*;

    #[test]
    fn required_self_cycle_is_e0407() {
        assert_eq!(codes("model A { a: A }"), vec!["E0407"]);
        assert_eq!(codes("model A { b: B }\nmodel B { a: A }"), vec!["E0407"]);
    }

    #[test]
    fn cycles_broken_by_optional_list_map_or_union_are_fine() {
        assert!(codes("model A { a?: A }").is_empty());
        assert!(codes("model A { a: A[] }").is_empty());
        assert!(codes("model A { a: Map<string, A> }").is_empty());
        assert!(codes("model A { u: U }\nunion U { a: A }").is_empty());
        assert!(codes("model N { next?: N, kids: N[] }").is_empty());
    }

    #[test]
    fn cycle_through_inheritance_is_e0407() {
        assert_eq!(
            codes("model A extends B { x: int }\nmodel B { a: A }"),
            vec!["E0407"]
        );
    }

    #[test]
    fn discriminator_clash_is_e0404() {
        assert_eq!(
            codes("model C { kind: string }\nunion U { c: C }"),
            vec!["E0404"]
        );
        assert!(
            codes("model C { kind: string }\n@discriminator(\"type\")\nunion U { c: C }")
                .is_empty()
        );
        assert_eq!(
            codes("model C { type: string }\n@discriminator(\"type\")\nunion U { c: C }"),
            vec!["E0404"]
        );
    }
}

mod services {
    use super::*;

    fn svc(body: &str) -> Vec<&'static str> {
        codes(&format!(
            "model M {{ a: int }}\nmodel Q {{ n?: int, tags: string[] }}\nservice S at \"/s\" {{\n{body}\n}}"
        ))
    }

    fn endpoint(body: &str) -> veld_ir::Endpoint {
        let ir = ir(&format!(
            "model M {{ a: int }}\nmodel Q {{ n?: int }}\nservice S at \"/s\" {{\n{body}\n}}"
        ));
        ir.services[0].endpoints[0].clone()
    }

    #[test]
    fn prefix_must_start_with_slash() {
        assert_eq!(codes("service S at \"s\" { }"), vec!["E0501"]);
        assert_eq!(
            codes("service S at \"/s\" { get \"x\" f() }"),
            vec!["E0501"]
        );
        assert_eq!(
            codes("service S at \"/s\" { get \"/{a\" f() }"),
            vec!["E0501"]
        );
    }

    #[test]
    fn paths_are_joined_and_normalised() {
        assert_eq!(endpoint("get \"/\" f()").path, "/s");
        assert_eq!(endpoint("get \"/x/\" f()").path, "/s/x");
        let ir = ir("service S at \"/\" { get \"/\" f() }");
        assert_eq!(ir.services[0].endpoints[0].path, "/");
        let ir2 = ir_with_prefix("/api/");
        assert_eq!(ir2, "/api/x");
    }

    fn ir_with_prefix(p: &str) -> String {
        ir(&format!("service S at \"{p}\" {{ get \"/x\" f() }}")).services[0].endpoints[0]
            .path
            .clone()
    }

    #[test]
    fn binding_rules() {
        let e = endpoint(
            "post \"/{id}\" f(id: uuid, body: M, query: Q, @query k: string, @header(\"X-H\") h: string)",
        );
        let b: Vec<_> = e.params.iter().map(|p| p.binding).collect();
        use veld_ir::Binding::{Body, Header, Path, Query, QueryObject};
        assert_eq!(b, vec![Path, Body, QueryObject, Query, Header]);
        assert_eq!(e.params[4].header.as_deref(), Some("X-H"));
    }

    #[test]
    fn path_params_must_be_scalar_and_required_e0502() {
        assert_eq!(svc("get \"/{id}\" f(id: M)"), vec!["E0502"]);
        assert_eq!(svc("get \"/{id}\" f(id?: string)"), vec!["E0502"]);
        assert_eq!(svc("get \"/{id}\" f(id: string[])"), vec!["E0502"]);
    }

    #[test]
    fn body_rules_e0503() {
        assert_eq!(svc("get \"/\" f(body: M)"), vec!["E0503"]);
        assert_eq!(svc("head \"/\" f(body: M)"), vec!["E0503"]);
        assert!(svc("put \"/\" f(body: M)").is_empty());
    }

    #[test]
    fn query_object_rules_e0504() {
        assert_eq!(svc("get \"/\" f(query: string)"), vec!["E0504"]);
        assert_eq!(svc("get \"/\" f(query: M2)").len(), 1);
        assert_eq!(
            codes(
                "model Deep { m: Deep2 }\nmodel Deep2 { a: int }\nservice S at \"/\" { get \"/\" f(query: Deep) }"
            ),
            vec!["E0504"]
        );
        assert_eq!(svc("get \"/\" f(@query k: M)"), vec!["E0504"]);
    }

    #[test]
    fn unbound_param_is_e0505_with_help() {
        let out = one("service S at \"/\" { get \"/\" f(x: int) }");
        assert_eq!(out.diagnostics[0].code, "E0505");
        assert!(
            out.diagnostics[0]
                .help
                .as_deref()
                .is_some_and(|h| h.contains("@query"))
        );
    }

    #[test]
    fn unmatched_path_param_is_e0506() {
        assert_eq!(svc("get \"/{id}\" f()"), vec!["E0506"]);
    }

    #[test]
    fn response_status_defaults() {
        assert_eq!(endpoint("post \"/\" f(body: M) -> M").success.status, 201);
        assert_eq!(endpoint("get \"/\" f() -> M").success.status, 200);
        assert_eq!(endpoint("get \"/\" f()").success.status, 204);
        assert_eq!(endpoint("get \"/\" f() -> 202").success.status, 202);
        assert!(endpoint("get \"/\" f() -> 202").success.ty.is_none());
        assert_eq!(
            endpoint("post \"/\" f(body: M) -> 200 M").success.status,
            200
        );
    }

    #[test]
    fn success_status_must_be_2xx_e0507() {
        assert_eq!(svc("get \"/\" f() -> 404 M"), vec!["E0507"]);
        assert_eq!(svc("get \"/\" f() -> 301"), vec!["E0507"]);
    }

    #[test]
    fn error_responses_e0405_e0508() {
        assert_eq!(svc("get \"/\" f() -> M | M"), vec!["E0405"]);
        assert_eq!(
            codes("error Odd { a: int }\nservice S at \"/\" { get \"/\" f() -> Odd }"),
            vec!["E0508"]
        );
        assert!(
            codes("error Odd { a: int }\nservice S at \"/\" { get \"/\" f() -> 418 Odd }")
                .is_empty()
        );
        assert_eq!(
            codes("error Odd 409 { a: int }\nservice S at \"/\" { get \"/\" f() -> 200 Odd }"),
            vec!["E0406"]
        );
    }

    #[test]
    fn implicit_validation_error_is_added_once() {
        let e = endpoint("post \"/\" f(body: M) -> M | NotFound");
        let statuses: Vec<u16> = e.errors.iter().map(|x| x.status).collect();
        assert_eq!(statuses, vec![404, 422, 500]);
        let e2 = endpoint("post \"/\" f(body: M) -> M | ValidationError");
        assert_eq!(e2.errors.iter().filter(|x| x.status == 422).count(), 1);
        let e3 = endpoint("get \"/\" f() -> M");
        let statuses: Vec<u16> = e3.errors.iter().map(|x| x.status).collect();
        assert_eq!(statuses, vec![500]);
    }

    #[test]
    fn implicit_unauthorized_and_internal() {
        let ir = ir(
            "model M { a: int }\n@auth(bearer)\nservice S at \"/\" {\n get \"/a\" a() -> M\n get \"/b\" b() -> M | Unauthorized | Internal\n @auth(none)\n get \"/c\" c() -> M\n}",
        );
        let st = |i: usize| -> Vec<u16> {
            ir.services[0].endpoints[i]
                .errors
                .iter()
                .map(|x| x.status)
                .collect()
        };
        assert_eq!(st(0), vec![401, 500]);
        assert_eq!(st(1), vec![401, 500]);
        assert_eq!(st(2), vec![500]);
    }
    #[test]
    fn same_status_errors_are_both_kept() {
        let e = endpoint("get \"/\" f() -> M | NotFound | 404 Conflict");
        assert_eq!(e.errors.iter().filter(|x| x.status == 404).count(), 2);
    }

    #[test]
    fn duplicate_endpoint_and_route_collisions() {
        assert_eq!(svc("get \"/a\" f()\nget \"/b\" f()"), vec!["E0509"]);
        assert_eq!(
            svc("get \"/a/{x}\" f(x: int)\nget \"/a/{y}\" g(y: int)"),
            vec!["E0510"]
        );
        assert!(svc("get \"/a/{x}\" f(x: int)\npost \"/a/{y}\" g(y: int, body: M)").is_empty());
        assert_eq!(
            codes("service A at \"/a\" { get \"/\" f() }\nservice B at \"/\" { get \"/a\" g() }"),
            vec!["E0510"]
        );
    }

    #[test]
    fn duplicate_param_name_is_e0512() {
        assert_eq!(svc("post \"/\" f(body: M, body: M)"), vec!["E0512"]);
    }

    #[test]
    fn streaming_rules_e0511() {
        assert!(svc("get \"/e\" f() -> stream M").is_empty());
        assert!(svc("ws \"/w\" w(body: stream M) -> stream M").is_empty());
        assert_eq!(svc("post \"/\" f(body: M) -> stream M"), vec!["E0511"]);
        assert_eq!(svc("post \"/\" f(body: stream M)"), vec!["E0511"]);
        assert_eq!(svc("ws \"/w\" w(@query k: stream string)"), vec!["E0511"]);
        assert_eq!(svc("get \"/\" f() -> M | stream NotFound"), vec!["E0511"]);
    }

    #[test]
    fn auth_and_middleware_resolution() {
        let ir = ir(
            "@auth(bearer)\n@middleware(Rate)\nservice S at \"/\" {\n get \"/a\" a()\n @auth(none)\n @middleware(Extra)\n get \"/b\" b()\n}",
        );
        let eps = &ir.services[0].endpoints;
        assert_eq!(eps[0].auth.kind, veld_ir::AuthKind::Bearer);
        assert_eq!(eps[1].auth.kind, veld_ir::AuthKind::None);
        assert_eq!(eps[0].middleware, vec!["Rate"]);
        assert_eq!(eps[1].middleware, vec!["Rate", "Extra"]);
    }

    #[test]
    fn binding_annotation_conflicts_are_e0602() {
        assert_eq!(svc("get \"/{id}\" f(@query id: string)"), vec!["E0602"]);
    }
}

mod annotations {
    use super::*;

    #[test]
    fn annotation_codes_through_the_pipeline() {
        assert_eq!(codes("@bogus\nmodel A { a: int }"), vec!["E0601"]);
        assert_eq!(codes("@readonly\nmodel A { a: int }"), vec!["E0602"]);
        assert_eq!(
            codes("model A { a: string @pattern(\"(?=x)\") }"),
            vec!["E0603"]
        );
    }

    #[test]
    fn constraints_reach_the_ir() {
        let ir = ir(
            "model A { a: string @length(min: 1, max: 4) @pattern(\"^a+$\") @format(email), n: int @min(1) @max(9) }",
        );
        let f = ir.decls[0].fields();
        assert_eq!(f[0].constraints.min_len, Some(1));
        assert_eq!(f[0].constraints.max_len, Some(4));
        assert_eq!(f[0].constraints.pattern.as_deref(), Some("^a+$"));
        assert_eq!(f[0].constraints.format, Some(veld_ir::StringFormat::Email));
        assert_eq!(f[1].constraints.min, Some(1.0));
        assert_eq!(f[1].constraints.max, Some(9.0));
    }

    #[test]
    fn deprecated_and_example_pass_through() {
        let ir = ir("@deprecated(\"old\")\nmodel A { a: int @example(3) @deprecated(\"x\") }");
        assert_eq!(ir.decls[0].deprecated.as_deref(), Some("old"));
        let f = &ir.decls[0].fields()[0];
        assert_eq!(f.example, Some(veld_ir::Literal::Int(3)));
        assert_eq!(f.deprecated.as_deref(), Some("x"));
    }

    #[test]
    fn wire_names_and_readonly() {
        let ir = ir("model A { firstName: string @wire(\"first_name\") @readonly }");
        let f = &ir.decls[0].fields()[0];
        assert_eq!(
            (f.name.as_str(), f.wire.as_str(), f.readonly),
            ("firstName", "first_name", true)
        );
    }
}

mod lowering {
    use super::*;

    #[test]
    fn decls_are_sorted_by_qualified_name_and_ids_are_indices() {
        let ir = ir("model Z { a: int }\nmodel A { a: int }\nenum M { x }");
        let names: Vec<&str> = ir.decls.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["A", "M", "Z"]);
        assert!(ir.decls.iter().enumerate().all(|(i, d)| d.id.index() == i));
    }

    #[test]
    fn prelude_is_included_only_when_referenced() {
        let none = ir("model A { a: int }");
        assert!(
            none.decls
                .iter()
                .all(|d| d.package == none.decls[0].package)
        );
        assert_eq!(none.packages.len(), 1);
        let used = ir("model A { a: int }\nservice S at \"/\" { get \"/\" f() -> NotFound }");
        assert!(
            used.packages
                .iter()
                .any(|p| p.name == "veld" && p.kind == veld_ir::PackageKind::Prelude)
        );
        let names: Vec<&str> = used.decls.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"NotFound"));
        assert!(
            !names.contains(&"Forbidden"),
            "unreferenced prelude decls are dropped"
        );
    }

    #[test]
    fn implicit_validation_error_pulls_in_validation_issue() {
        let ir = ir("model M { a: int }\nservice S at \"/\" { post \"/\" f(body: M) }");
        let names: Vec<&str> = ir.decls.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"ValidationError") && names.contains(&"ValidationIssue"));
    }

    #[test]
    fn services_are_sorted_and_origins_are_relative() {
        let out = multi(&[
            (
                "zeta",
                "x/y.veld",
                "service B at \"/b\" { }\nservice A at \"/a\" { }",
            ),
            ("alpha", "m.veld", "service Q at \"/q\" { }"),
        ]);
        let Some(ir) = out.ir.as_ref() else {
            panic!("{}", out.render())
        };
        let order: Vec<(&str, &str)> = ir
            .services
            .iter()
            .map(|s| (ir.package(s.package).name.as_str(), s.name.as_str()))
            .collect();
        assert_eq!(order, vec![("alpha", "Q"), ("zeta", "A"), ("zeta", "B")]);
        assert_eq!(ir.services[1].origin.file, "x/y.veld");
        assert_eq!(ir.services[1].origin.line, 2);
    }

    #[test]
    fn ir_is_independent_of_input_order() {
        let a = multi(&[
            ("p", "a.veld", "model A { x: int }"),
            ("p", "b.veld", "model B { x: int }"),
        ]);
        let b = multi(&[
            ("p", "b.veld", "model B { x: int }"),
            ("p", "a.veld", "model A { x: int }"),
        ]);
        assert_eq!(a.ir, b.ir);
    }

    #[test]
    fn module_paths_and_qualified_names() {
        let out = multi(&[(
            "orders",
            "billing/invoices.veld",
            "model Invoice { a: int }",
        )]);
        let ir = out.ir.unwrap_or_else(|| panic!("no ir"));
        let d = &ir.decls[0];
        assert_eq!(d.module, "billing.invoices");
        assert_eq!(d.qualified, "orders:billing.invoices.Invoice");
        assert!(d.sig.starts_with("veld:orders/Invoice@"));
    }

    #[test]
    fn invalid_module_file_name_is_e0101() {
        let out = multi(&[("app", "my-file.veld", "model A { a: int }")]);
        assert_eq!(out.diagnostics[0].code, "E0101");
    }

    #[test]
    fn model_parent_and_union_arms_are_remapped() {
        let ir = ir("model B { b: int }\nmodel A extends B { a: int }\nunion U { x: A, y: B }");
        let a = ir
            .decls
            .iter()
            .find(|d| d.name == "A")
            .map(|d| d.kind.clone());
        let b_id = ir.decls.iter().find(|d| d.name == "B").map(|d| d.id);
        match a {
            Some(veld_ir::DeclKind::Model(m)) => assert_eq!(m.parent, b_id),
            other => panic!("unexpected {other:?}"),
        }
    }
}

mod signatures {
    use super::*;

    fn sig_of(src: &str, name: &str) -> String {
        ir(src)
            .decls
            .iter()
            .find(|d| d.name == name)
            .map(|d| d.sig.clone())
            .unwrap_or_default()
    }

    #[test]
    fn signatures_are_deterministic() {
        let s = "model A { a: int, b?: string }";
        assert_eq!(sig_of(s, "A"), sig_of(s, "A"));
        assert!(sig_of(s, "A").starts_with("veld:app/A@"));
    }

    #[test]
    fn reordering_fields_does_not_change_the_signature() {
        assert_eq!(
            sig_of("model A { a: int, b: string }", "A"),
            sig_of("model A { b: string, a: int }", "A")
        );
    }

    #[test]
    fn renaming_a_source_name_with_the_same_wire_keeps_the_signature() {
        assert_eq!(
            sig_of("model A { first: string @wire(\"n\") }", "A"),
            sig_of("model A { renamed: string @wire(\"n\") }", "A")
        );
    }

    #[test]
    fn changing_type_optionality_or_wire_changes_the_signature() {
        let base = sig_of("model A { a: int }", "A");
        assert_ne!(base, sig_of("model A { a: string }", "A"));
        assert_ne!(base, sig_of("model A { a?: int }", "A"));
        assert_ne!(base, sig_of("model A { a: int @wire(\"b\") }", "A"));
        assert_ne!(base, sig_of("model A { a: int, b: int }", "A"));
    }

    #[test]
    fn docs_constraints_defaults_and_deprecations_do_not_matter() {
        let base = sig_of("model A { a: int }", "A");
        assert_eq!(
            base,
            sig_of(
                "/// doc\nmodel A { /// f\n a: int @min(1) @deprecated(\"x\") @readonly }",
                "A"
            )
        );
        assert_eq!(
            sig_of("model A { a: int }", "A"),
            sig_of("model A { a: int = 3 }", "A")
        );
    }

    #[test]
    fn a_referenced_declaration_is_part_of_the_shape() {
        let a1 = sig_of("model B { x: int }\nmodel A { b: B }", "A");
        let a2 = sig_of("model B { x: string }\nmodel A { b: B }", "A");
        assert_ne!(a1, a2);
        let unrelated = sig_of(
            "model B { x: int }\nmodel C { y: int }\nmodel A { b: B }",
            "A",
        );
        assert_eq!(a1, unrelated);
    }

    #[test]
    fn enum_wire_values_matter_and_variant_order_does_not() {
        assert_eq!(
            sig_of("enum E { a, b }", "E"),
            sig_of("enum E { b, a }", "E")
        );
        assert_ne!(
            sig_of("enum E { a, b }", "E"),
            sig_of("enum E { a, b = \"B\" }", "E")
        );
    }

    #[test]
    fn recursive_types_have_a_signature() {
        let s = sig_of("model N { next?: N, kids: N[] }", "N");
        assert!(s.starts_with("veld:app/N@"));
    }

    #[test]
    fn inherited_fields_are_part_of_the_child_shape() {
        let flat = sig_of("model C { a: int, b: int }", "C");
        let inherited = sig_of("model P { a: int }\nmodel C extends P { b: int }", "C");
        // Same wire shape, different qualified root name only when the names differ; here both are `C`.
        assert_eq!(flat, inherited);
    }

    #[test]
    fn operation_signature_ignores_param_names_and_depends_on_method() {
        let a = ir("service S at \"/s\" { get \"/{id}\" f(id: int) }").services[0].endpoints[0]
            .sig
            .clone();
        let b = ir("service S at \"/s\" { get \"/{other}\" f(other: int) }").services[0].endpoints
            [0]
        .sig
        .clone();
        let c = ir("service S at \"/s\" { delete \"/{id}\" f(id: int) }").services[0].endpoints[0]
            .sig
            .clone();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn prelude_signatures_use_the_veld_package() {
        let ir = ir("service S at \"/\" { get \"/\" f() -> NotFound }");
        let nf = ir
            .decls
            .iter()
            .find(|d| d.name == "NotFound")
            .map(|d| d.sig.clone())
            .unwrap_or_default();
        assert!(nf.starts_with("veld:veld/NotFound@"), "{nf}");
    }
}
