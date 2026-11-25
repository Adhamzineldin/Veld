use std::collections::BTreeMap;

use veld_compiler::{CompileInput, PackageInput, SourceFile, compile};
use veld_ir::{Ir, PackageKind};

use super::*;
use crate::testutil::ir;

fn files_ir(kind: PackageKind, files: &[(&str, &str)]) -> Ir {
    let out = compile(&CompileInput {
        packages: vec![PackageInput {
            name: "app".into(),
            kind,
            files: files
                .iter()
                .map(|(p, t)| SourceFile {
                    path: (*p).into(),
                    text: (*t).into(),
                })
                .collect(),
            base_url: None,
        }],
        consumes: BTreeMap::new(),
    });
    let text = out.render();
    out.ir.unwrap_or_else(|| panic!("{text}"))
}

fn rules(src: &str) -> Vec<(&'static str, String)> {
    lint(&ir(src), &LintConfig::default())
        .into_iter()
        .map(|i| (i.rule, i.subject))
        .collect()
}

fn has(found: &[(&'static str, String)], rule: &str, subject: &str) -> bool {
    found.iter().any(|(r, s)| *r == rule && s == subject)
}

const CLEAN: &str = "\
/// A user.
model User {
  id: uuid
  name: string
}

/// Users.
@auth(none)
service Users at \"/users\" {
  /// Fetch one.
  get \"/{id}\" getUser(id: uuid) -> User
}
";

#[test]
fn a_clean_contract_has_no_findings() {
    assert_eq!(rules(CLEAN), vec![]);
}

#[test]
fn unused_model_follows_fields_and_extends() {
    let src = "\
model Address { street: string }
model Base { id: uuid }
model Child extends Base { home: Address }
model Orphan { x: int }
enum Color { red }
model Painted { c: Color }
/// S.
service S at \"/s\" {
  /// e.
  get \"/\" e() -> Child
}
";
    let f = rules(src);
    assert!(has(&f, "unused-model", "Orphan"));
    assert!(has(&f, "unused-model", "Painted"));
    assert!(has(&f, "unused-model", "Color"));
    assert!(!has(&f, "unused-model", "Address"));
    assert!(!has(&f, "unused-model", "Base"));
    assert!(!has(&f, "unused-model", "Child"));
}

#[test]
fn a_parent_of_an_unused_model_counts_as_used() {
    let src = "\
model Base { id: uuid }
model Child extends Base { n: int }
/// S.
service S at \"/s\" {
  /// e.
  get \"/\" e() -> 204
}
";
    let f = rules(src);
    assert!(!has(&f, "unused-model", "Base"));
    assert!(has(&f, "unused-model", "Child"));
}

#[test]
fn libraries_are_not_checked_for_unused_models() {
    let ir = files_ir(
        PackageKind::Library,
        &[("m.veld", "model Lonely { x: int }\n")],
    );
    let f = lint(&ir, &LintConfig::default());
    assert!(f.iter().all(|i| i.rule != "unused-model"), "{f:?}");
}

#[test]
fn empty_service_model_and_constants() {
    let f = rules(
        "model Nothing {}\nconst EMPTY: string = \"\"\nconst OK: int = 1\n\
         /// S.\nservice Idle at \"/i\" {}\n",
    );
    assert!(has(&f, "empty-service", "Idle"));
    assert!(has(&f, "empty-model", "Nothing"));
    assert!(has(&f, "empty-constants", "EMPTY"));
    assert!(!has(&f, "empty-constants", "OK"));
}

#[test]
fn missing_descriptions_are_reported_per_service_and_endpoint() {
    let f = rules("model M { a: int }\nservice S at \"/s\" {\n  get \"/\" e() -> M\n}\n");
    assert!(has(&f, "missing-description", "S"));
    assert!(has(&f, "missing-description", "S.e"));
}

#[test]
fn deprecations_are_inventoried() {
    let f = rules(
        "/// M.\n@deprecated(\"gone\")\nmodel M {\n  old: int @deprecated(\"use b\")\n  b: int\n}\n\
         /// S.\nservice S at \"/s\" {\n  /// e.\n  @deprecated(\"use f\")\n  get \"/\" e() -> M\n}\n",
    );
    assert!(has(&f, "deprecated-endpoint", "S.e"));
    assert!(has(&f, "deprecated-field", "M.old"));
    assert!(has(&f, "deprecated-declaration", "M"));
}

#[test]
fn inherited_fields_are_reported_once_on_the_declaring_model() {
    let f = rules(
        "model Base {\n  old: int @deprecated(\"x\")\n}\nmodel Kid extends Base { n: int }\n\
         /// S.\nservice S at \"/s\" {\n  /// e.\n  get \"/\" e() -> Kid\n}\n",
    );
    assert!(has(&f, "deprecated-field", "Base.old"));
    assert!(!has(&f, "deprecated-field", "Kid.old"));
}

#[test]
fn naming_rules_follow_the_spec() {
    let f = rules(
        "model order_item {\n  Item_Name: string\n  okName: int\n}\n\
         enum Mode { fastMode, slow_mode, BadMode }\n",
    );
    assert!(has(&f, "type-name-case", "order_item"));
    assert!(has(&f, "field-name-case", "order_item.Item_Name"));
    assert!(!has(&f, "field-name-case", "order_item.okName"));
    assert!(has(&f, "variant-name-case", "Mode.BadMode"));
    assert!(!has(&f, "variant-name-case", "Mode.fastMode"));
    assert!(!has(&f, "variant-name-case", "Mode.slow_mode"));
}

#[test]
fn service_and_endpoint_names_are_checked() {
    let f = rules(
        "model M { a: int }\n/// S.\nservice user_api at \"/s\" {\n  /// e.\n  get \"/\" Get_All() -> M\n}\n",
    );
    assert!(has(&f, "type-name-case", "user_api"));
    assert!(has(&f, "endpoint-name-case", "user_api.Get_All"));
}

#[test]
fn file_naming_convention_checks_contents() {
    let ir = files_ir(
        PackageKind::Service,
        &[
            (
                "user_model.veld",
                "/// S.\nservice Mixed at \"/m\" {\n  /// e.\n  get \"/\" e() -> 204\n}\n",
            ),
            ("orders_service.veld", "model Bad { a: int }\n"),
            (
                "kinds_enum.veld",
                "enum Kind { a }\nmodel AlsoBad { a: int }\n",
            ),
            ("plain.veld", "model Fine { a: int }\n"),
        ],
    );
    let f: Vec<_> = lint(&ir, &LintConfig::default())
        .into_iter()
        .filter(|i| i.rule == "file-naming-convention")
        .collect();
    assert_eq!(f.len(), 3, "{f:?}");
    assert!(f.iter().any(|i| i.message.contains("`user_model.veld`")));
    assert!(
        f.iter()
            .any(|i| i.message.contains("`orders_service.veld`"))
    );
    assert!(f.iter().any(|i| i.message.contains("`kinds_enum.veld`")));
}

#[test]
fn levels_turn_rules_off_or_into_errors_and_order_is_stable() {
    let ir = ir("model Nothing {}\nmodel Other {}\n");
    let levels: BTreeMap<String, LintLevel> = [
        ("unused-model".to_string(), LintLevel::Off),
        ("empty-model".to_string(), LintLevel::Error),
    ]
    .into();
    let cfg = LintConfig::from_levels(levels.iter()).unwrap();
    let issues = lint(&ir, &cfg);
    assert!(issues.iter().all(|i| i.rule != "unused-model"));
    assert_eq!(issues.len(), 2);
    assert!(
        issues
            .iter()
            .all(|i| i.level == LintLevel::Error && i.code == "L0003")
    );
    assert!(has_errors(&issues));
    assert_eq!(issues, lint(&ir, &cfg));
    assert!(!has_errors(&lint(&ir, &LintConfig::default())));
}

#[test]
fn strict_makes_every_enabled_rule_an_error() {
    let ir = ir("model Nothing {}\n");
    let levels: BTreeMap<String, LintLevel> = [("unused-model".to_string(), LintLevel::Off)].into();
    let cfg = LintConfig::from_levels(levels.iter()).unwrap().strict();
    let issues = lint(&ir, &cfg);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].rule, "empty-model");
    assert_eq!(issues[0].level, LintLevel::Error);
}

#[test]
fn unknown_rule_names_are_rejected_with_the_valid_list() {
    let levels: BTreeMap<String, LintLevel> = [("unused-modle".to_string(), LintLevel::Off)].into();
    let err = LintConfig::from_levels(levels.iter()).unwrap_err();
    assert_eq!(err.name, "unused-modle");
    assert!(err.to_string().contains("unused-model"));
}

#[test]
fn issues_carry_origins_and_codes() {
    let issues = lint(&ir("model Nothing {}\n"), &LintConfig::default());
    let i = issues.iter().find(|i| i.rule == "empty-model").unwrap();
    assert_eq!(i.code, "L0003");
    assert_eq!(i.origin.file, "app.veld");
    assert_eq!(i.origin.line, 1);
}

#[test]
fn rule_names_cover_the_code_table() {
    assert_eq!(rule_names().len(), veld_compiler::codes::LINT_RULES.len());
}
