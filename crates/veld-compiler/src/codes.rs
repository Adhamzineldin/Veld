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

const C: &str = "veld-compiler";
const P: &str = "veld-project";
const H: &str = "veld-plugin-host";
const G: &str = "veld-codegen";
const GEN: &str = "generators";

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
        "Nesting too deep: types and annotation call arguments are limited to 64 levels",
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
        "Service prefix or endpoint path does not start with `/`",
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
    e("E0509", C, "Endpoint declared twice in a service"),
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
    e(
        "E0702",
        C,
        "`consumes` or `service` names an unknown service",
    ),
    e(
        "E0703",
        P,
        "Two targets have overlapping output directories",
    ),
    e(
        "E0704",
        P,
        "Output directory is forbidden (project root or a contracts directory)",
    ),
    e("E0705", H, "Plugin executable hash does not match `sha256`"),
    e("E0706", H, "Plugin protocol error"),
    e("E0707", H, "Plugin returned invalid or unsafe files"),
    e("E0708", H, "Plugin process failed or timed out"),
    e(
        "E0801",
        G,
        "Feature not supported by the generator for this role",
    ),
    e("E0802", G, "Invalid generator options"),
    e(
        "E0803",
        G,
        "Invalid generation request (unknown framework, role or service, nothing selected)",
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
    w("W0001", C, "Doc comment is not attached to anything"),
    w("W0201", C, "Unused import"),
    w("W0202", C, "Declaration shadows a prelude declaration"),
    w("W0701", C, "Service consumes itself"),
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
        let key = |c: &CodeInfo| (c.kind == Kind::Warning, c.code);
        for pair in CODES.windows(2) {
            assert!(
                key(&pair[0]) < key(&pair[1]),
                "{} / {}",
                pair[0].code,
                pair[1].code
            );
        }
        for c in CODES {
            let prefix = if c.kind == Kind::Error { 'E' } else { 'W' };
            assert!(
                c.code.starts_with(prefix) && c.code.len() == 5,
                "{}",
                c.code
            );
            assert!(!c.meaning.is_empty());
        }
    }

    #[test]
    fn lookup_finds_documented_codes_only() {
        assert!(lookup("E0305").is_some());
        assert!(lookup("E9999").is_none());
    }
}
