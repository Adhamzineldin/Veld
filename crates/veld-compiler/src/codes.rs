//! The diagnostic code table: the single source of truth for every `E####` and `W####` code
//! in the workspace (compiler, project config, plugin host and generators).
//!
//! `spec/diagnostics.md` is generated from [`CODES`] by `cargo xtask codegen`. The compiler's
//! `Diagnostic` constructors check every code against this table, and a workspace test
//! (`xtask`) scans all sources for code literals, so a code cannot exist undocumented.

/// Whether a code is an error or a warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Error,
    Warning,
    /// A lint rule finding (`L####`): legal but suspicious. Never blocks generation by itself.
    Lint,
}

/// One diagnostic code.
#[derive(Debug, Clone, Copy)]
pub struct CodeInfo {
    pub code: &'static str,
    pub kind: Kind,
    /// Which crate reports it.
    pub source: &'static str,
    /// One line: what the code means.
    pub meaning: &'static str,
}

const fn e(code: &'static str, source: &'static str, meaning: &'static str) -> CodeInfo {
    CodeInfo {
        code,
        kind: Kind::Error,
        source,
        meaning,
    }
}

const fn w(code: &'static str, source: &'static str, meaning: &'static str) -> CodeInfo {
    CodeInfo {
        code,
        kind: Kind::Warning,
        source,
        meaning,
    }
}

const fn l(code: &'static str, source: &'static str, meaning: &'static str) -> CodeInfo {
    CodeInfo {
        code,
        kind: Kind::Lint,
        source,
        meaning,
    }
}

const C: &str = "veld-compiler";
const P: &str = "veld-project";
const H: &str = "veld-plugin-host";
const G: &str = "veld-codegen";
const GEN: &str = "generators";
const T: &str = "veld-tools";

/// Every diagnostic code: errors first, then warnings, each sorted by code. Codes are never
/// reused.
pub const CODES: &[CodeInfo] = &[
    e("E0001", C, "Veld v1 syntax that is not valid in v2"),
    e(
        "E0002",
        C,
        "Lexical error (bad character, unterminated string, integer literal out of range)",
    ),
    e(
        "E0003",
        C,
        "Unexpected token: the parser expected something else",
    ),
    e("E0004", C, "Reserved word used as a name"),
    e(
        "E0005",
        C,
        "Nesting too deep: types and annotation call arguments are limited to 64 levels, rule expressions to 32",
    ),
    e("E0006", C, "`extends` chain longer than 64 models"),
    e(
        "E0101",
        C,
        "Module path derived from a file path is invalid",
    ),
    e("E0201", C, "Duplicate declaration in a module"),
    e(
        "E0202",
        C,
        "Imported name collides with another name in the module",
    ),
    e("E0203", C, "Unknown package in a `use` declaration"),
    e("E0204", C, "Unknown module in a `use` declaration"),
    e("E0205", C, "Unknown name"),
    e(
        "E0206",
        C,
        "`use @org/name:...` of a registry package that is not a direct dependency (of the project, or of the importing package)",
    ),
    e("E0301", C, "Map key type is not `string`"),
    e(
        "E0302",
        C,
        "Type arguments used wrongly (wrong count, or the type is not generic)",
    ),
    e(
        "E0303",
        C,
        "Default or example literal does not match the field type (also: optional field with a default)",
    ),
    e("E0304", C, "Enum default names an unknown variant"),
    e(
        "E0305",
        C,
        "Float literal is not finite (it overflows a 64-bit float, for example `1.0e999`)",
    ),
    e(
        "E0401",
        C,
        "Field redeclares a field inherited from a parent model",
    ),
    e("E0402", C, "Inheritance cycle"),
    e("E0403", C, "Two enum variants share a wire value"),
    e(
        "E0404",
        C,
        "A union arm already has a field named like the discriminator of the union",
    ),
    e(
        "E0405",
        C,
        "`error` declaration used outside the error part of a response",
    ),
    e("E0406", C, "Error status is not 4xx or 5xx"),
    e(
        "E0407",
        C,
        "Type contains itself through required fields (no finite value can exist)",
    ),
    e("E0408", C, "`extends` target is not a model"),
    e(
        "E0409",
        C,
        "Union arm is not a model, or the union is empty",
    ),
    e("E0410", C, "Duplicate field, wire name, variant or arm"),
    e(
        "E0501",
        C,
        "Service prefix or endpoint path does not start with `/`, or a service with endpoints has no `at`",
    ),
    e(
        "E0502",
        C,
        "Path parameter is optional or has an invalid type",
    ),
    e("E0503", C, "`body` is not allowed on this HTTP method"),
    e(
        "E0504",
        C,
        "Invalid `query` parameter or `@query` binding type",
    ),
    e(
        "E0505",
        C,
        "Parameter has no binding, or a header parameter has an invalid type",
    ),
    e(
        "E0506",
        C,
        "Path template names a parameter that does not exist",
    ),
    e("E0507", C, "Success status is not 2xx"),
    e(
        "E0508",
        C,
        "Error has neither a status here nor a default status",
    ),
    e(
        "E0509",
        C,
        "Endpoint or event handler declared twice in a service",
    ),
    e("E0510", C, "Two endpoints collide on method and path"),
    e("E0511", C, "`stream` used where it is not allowed"),
    e("E0512", C, "Duplicate parameter"),
    e("E0601", C, "Unknown annotation"),
    e(
        "E0602",
        C,
        "Annotation used wrongly (wrong target, arguments or repeated)",
    ),
    e("E0603", C, "Pattern is outside the portable regex subset"),
    e("E0700", P, "Invalid `veld.toml`"),
    e(
        "E0701",
        C,
        "Unknown key in `veld.toml`, or a reserved package name",
    ),
    e("E0702", C, "`consumes` or `uses` names an unknown service"),
    e(
        "E0703",
        P,
        "Two targets have overlapping output directories",
    ),
    e(
        "E0704",
        P,
        "Output directory is forbidden (workspace root or a contracts directory)",
    ),
    e("E0705", H, "Plugin executable hash does not match `sha256`"),
    e("E0706", H, "Plugin protocol error"),
    e("E0707", H, "Plugin returned invalid or unsafe files"),
    e("E0708", H, "Plugin process failed or timed out"),
    e(
        "E0709",
        P,
        "`veld.toml` is in the layout of earlier betas (`[project]`, `[services.*]`, `[libraries.*]` or `[[targets]]`); the message says what to write instead",
    ),
    e(
        "E0710",
        P,
        "Invalid workspace structure (no member table, more than one, a nested workspace, a member no workspace lists, a bad `members` pattern, a missing member file, a missing, invalid or duplicate member or target name)",
    ),
    e(
        "E0711",
        P,
        "A table that is not allowed in this `veld.toml` (a root-only table in a member file, a `[server]` in an app, a target in a library)",
    ),
    e(
        "E0712",
        P,
        "`--affected` cannot read the changes (no git work tree, `git` not found, unknown ref)",
    ),
    e(
        "E0801",
        G,
        "Feature not supported by the generator for this role",
    ),
    e(
        "E0802",
        G,
        "Invalid generator options (also: an unsupported language version, such as `java = \"19\"`)",
    ),
    e(
        "E0803",
        G,
        "Invalid generation request (unknown framework or framework version, a language older than the framework needs, unknown role or service, nothing selected, a `-p` name or a target that is not a selected member)",
    ),
    e(
        "E0804",
        GEN,
        "Constant cannot be represented by the generator",
    ),
    e(
        "E0805",
        GEN,
        "Generated file cannot be added (bad or duplicate path)",
    ),
    e(
        "E0899",
        G,
        "Generator or plugin failure without a more specific code",
    ),
    e(
        "E0901",
        C,
        "Unknown name in an expression, or a lambda variable that shadows another name",
    ),
    e(
        "E0902",
        C,
        "Type error in an expression: operand types do not fit the operator, a rule that is not `bool`, a chained comparison, an enum literal with no enum to infer",
    ),
    e(
        "E0903",
        C,
        "An optional value is read where it is not known to be present",
    ),
    e(
        "E0904",
        C,
        "`now`, `result` or `old` used where it is not available",
    ),
    e(
        "E0905",
        C,
        "Operation not supported on this type: `/` or `%` on decimal, ordering strings, comparing models, lists, maps, bytes or json, a `const` of an unsupported type",
    ),
    e(
        "E0906",
        C,
        "Unknown function, wrong number or type of arguments, lambda where none is allowed",
    ),
    e(
        "E0907",
        C,
        "Duplicate rule name in a declaration, counting inherited rules",
    ),
    e("E0908", C, "Numeric literal out of range for its type"),
    e(
        "E0920",
        C,
        "A fact named like a parameter, another fact, the principal, `now` or `result`; a parameter or fact named `transition` on an endpoint that has one",
    ),
    e(
        "E0921",
        C,
        "The error after `else` is not one of the endpoint's declared errors",
    ),
    e(
        "E0922",
        C,
        "Two facts with one name but different types, key types or optionality in a service; an unknown or missing field in an `else` payload",
    ),
    e("E0923", C, "A fact read in `ensure` outside `old(...)`"),
    e(
        "E0924",
        C,
        "`ensure` on an endpoint whose success is a stream",
    ),
    e(
        "E0930",
        C,
        "A `flow` or `final` names something that is not a variant of the enum",
    ),
    e(
        "E0931",
        C,
        "A `final` state with an outgoing flow, or `final` in an enum with no flow",
    ),
    e(
        "E0932",
        C,
        "The path of a `transition` does not start at a fact, does not end in a state machine, or passes through an optional step",
    ),
    e(
        "E0933",
        C,
        "A `transition` lists a move that is not a flow of the machine",
    ),
    e("E0934", C, "More than one `transition` in an endpoint"),
    e("E0940", C, "The type after `principal` is not a model"),
    e("E0941", C, "More than one `principal` in a package"),
    e(
        "E0942",
        C,
        "`allow` names an unknown policy, or its arguments do not fit the policy's parameters",
    ),
    e(
        "E0950",
        C,
        "Duplicate example name; unknown or ambiguous endpoint in an example",
    ),
    e(
        "E0951",
        C,
        "`given` names something that is not a fact of the called endpoint",
    ),
    e(
        "E0952",
        C,
        "A value in an example does not fit the type it is used at",
    ),
    e(
        "E0953",
        "veld-rules",
        "A value an example leaves out cannot be filled in, or the filled-in value breaks an invariant",
    ),
    e(
        "E0970",
        "veld-prove",
        "`--prove`: the rules contradict each other, a `require` can never pass, or a policy permits nobody",
    ),
    e(
        "E1001",
        C,
        "An event is used as a type, or an event field has an event type",
    ),
    e(
        "E1002",
        C,
        "`carries` names something that is not an event, names an event twice, or is missing",
    ),
    e(
        "E1003",
        C,
        "The topic of a channel is not a legal topic name",
    ),
    e(
        "E1004",
        C,
        "Two channels of the workspace have the same topic",
    ),
    e(
        "E1005",
        C,
        "The `key` of a channel is not a required field of the same key type in every carried event, or the channel has two keys",
    ),
    e(
        "E1006",
        C,
        "`publishes` or `subscribes` names something that is not a channel, or names a channel twice",
    ),
    e(
        "E1007",
        C,
        "The group of a subscription is not a legal group name",
    ),
    e(
        "E1008",
        C,
        "An `on` line names an event the channel does not carry or names an event twice, or the subscription has no `on` line",
    ),
    e(
        "E1009",
        C,
        "A subscription to a channel of this workspace that no service publishes",
    ),
    e(
        "E1010",
        C,
        "An event example that is not well formed: the channel cannot be chosen, an `expect` names no handler of that channel, or `rejected` is combined with `called`",
    ),
    w("W0001", C, "Doc comment is not attached to anything"),
    w("W0201", C, "Unused import"),
    w("W0202", C, "Declaration shadows a prelude declaration"),
    w("W0701", C, "Service consumes itself"),
    w("W0901", C, "`==` or `!=` on `float`"),
    w(
        "W0902",
        C,
        "Nested comprehension over a list without `@length(max: n)`",
    ),
    w("W0903", C, "A rule that reads no field"),
    w(
        "W0904",
        "generators",
        "A target with request validation turned off reaches rules, which it will not enforce",
    ),
    w("W0930", C, "The same flow declared twice"),
    w("W0931", C, "A state that is not final and has no way out"),
    w(
        "W0932",
        C,
        "A variant of a state machine that takes part in no flow and is not final",
    ),
    w("W0940", C, "A policy with no `permit` permits nobody"),
    w(
        "W0970",
        "veld-prove",
        "`--prove`: some questions ended unknown or unsupported",
    ),
    w(
        "W1001",
        C,
        "A published channel that nobody in the workspace subscribes to",
    ),
    w(
        "W1002",
        C,
        "An event of a published channel that no subscription handles",
    ),
    l(
        "L0001",
        T,
        "`unused-model`: declaration is never used by any endpoint, model or constant",
    ),
    l("L0002", T, "`empty-service`: service has no endpoints"),
    l("L0003", T, "`empty-model`: model has no fields"),
    l("L0004", T, "`empty-constants`: constant has an empty value"),
    l(
        "L0005",
        T,
        "`missing-description`: service or endpoint has no doc comment",
    ),
    l("L0006", T, "`deprecated-endpoint`: endpoint is deprecated"),
    l("L0007", T, "`deprecated-field`: field is deprecated"),
    l(
        "L0008",
        T,
        "`file-naming-convention`: file name suffix does not match its contents",
    ),
    l(
        "L0009",
        T,
        "`type-name-case`: type or service name is not PascalCase",
    ),
    l("L0010", T, "`field-name-case`: field name is not camelCase"),
    l(
        "L0011",
        T,
        "`endpoint-name-case`: endpoint name is not camelCase",
    ),
    l(
        "L0012",
        T,
        "`variant-name-case`: enum variant is neither camelCase nor snake_case",
    ),
    l(
        "L0013",
        T,
        "`deprecated-declaration`: declaration or enum variant is deprecated",
    ),
    l(
        "L0014",
        T,
        "`auth-required`: an endpoint whose auth comes from neither its own `@auth` nor its service's",
    ),
    l(
        "L0015",
        T,
        "`pii-without-auth`: an endpoint with no auth can answer with a `@pii` field",
    ),
    l(
        "L0016",
        T,
        "`secret-in-output`: a `@secret` field that a response, a declared error, a stream item or an event can contain",
    ),
];

/// Lint rule names by code, in code order. The names are what `[lint]` in `veld.toml` uses.
pub const LINT_RULES: [(&str, &str); 16] = [
    ("L0001", "unused-model"),
    ("L0002", "empty-service"),
    ("L0003", "empty-model"),
    ("L0004", "empty-constants"),
    ("L0005", "missing-description"),
    ("L0006", "deprecated-endpoint"),
    ("L0007", "deprecated-field"),
    ("L0008", "file-naming-convention"),
    ("L0009", "type-name-case"),
    ("L0010", "field-name-case"),
    ("L0011", "endpoint-name-case"),
    ("L0012", "variant-name-case"),
    ("L0013", "deprecated-declaration"),
    ("L0014", "auth-required"),
    ("L0015", "pii-without-auth"),
    ("L0016", "secret-in-output"),
];

/// The entry for `code`, if it is documented.
#[must_use]
pub fn lookup(code: &str) -> Option<&'static CodeInfo> {
    CODES.iter().find(|c| c.code == code)
}

/// The contents of `spec/diagnostics.md`.
#[must_use]
pub fn markdown() -> String {
    let mut out = String::from(
        "# Diagnostics\n\n\
         Generated by `cargo xtask codegen` from `crates/veld-compiler/src/codes.rs`. Do not edit.\n\n\
         Every diagnostic has a stable code: `E####` is an error and `W####` a warning. Codes are\n\
         never reused. The ranges are listed in `language.md` section 9.\n\n\
         | Code | Kind | Reported by | Meaning |\n|---|---|---|---|\n",
    );
    for c in CODES {
        let kind = match c.kind {
            Kind::Error => "error",
            Kind::Warning => "warning",
            Kind::Lint => "lint",
        };
        out.push_str(&format!(
            "| `{}` | {kind} | {} | {} |\n",
            c.code, c.source, c.meaning
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_unique_and_kinds_match_prefixes() {
        let key = |c: &CodeInfo| (c.kind as u8, c.code);
        for pair in CODES.windows(2) {
            assert!(
                key(&pair[0]) < key(&pair[1]),
                "{} / {}",
                pair[0].code,
                pair[1].code
            );
        }
        for c in CODES {
            let prefix = match c.kind {
                Kind::Error => 'E',
                Kind::Warning => 'W',
                Kind::Lint => 'L',
            };
            assert!(
                c.code.starts_with(prefix) && c.code.len() == 5,
                "{}",
                c.code
            );
            assert!(!c.meaning.is_empty());
        }
    }

    #[test]
    fn every_lint_rule_has_a_lint_code_naming_it() {
        let lints: Vec<_> = CODES.iter().filter(|c| c.kind == Kind::Lint).collect();
        assert_eq!(lints.len(), LINT_RULES.len());
        for (code, name) in LINT_RULES {
            let info = lookup(code).map(|c| c.meaning);
            assert!(
                info.is_some_and(|m| m.starts_with(&format!("`{name}`"))),
                "{code}"
            );
        }
    }

    #[test]
    fn lookup_finds_documented_codes_only() {
        assert!(lookup("E0305").is_some());
        assert!(lookup("E9999").is_none());
    }
}
