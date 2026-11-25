//! A machine-readable description of the IR's JSON shape and of the plugin protocol messages.
//!
//! `veld plugin new` turns it into `ir.d.ts` ([`typescript`]), so plugin authors get the exact
//! types without writing them by hand. The tests keep it honest: a sample IR that uses every
//! variant is serialized and validated against this description ([`check`]), so adding,
//! removing or renaming a field in the IR makes a test fail until the schema follows.

use serde_json::Value;

/// A type expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum T {
    Str,
    Num,
    Bool,
    /// Any JSON value (`unknown`).
    Any,
    Ref(&'static str),
    List(&'static T),
    /// `Record<string, T>`
    Map(&'static T),
    /// The key is always present; the value may be `null`.
    Nullable(&'static T),
    /// The key may be absent.
    Optional(&'static T),
    /// Any one of these types.
    Union(&'static [T]),
    /// Exactly this string.
    Lit(&'static str),
}

pub type Fields = &'static [(&'static str, T)];

/// One named type of the schema.
#[derive(Debug, Clone, Copy)]
pub enum Def {
    /// A JSON object with exactly these keys.
    Object(&'static str, Fields),
    /// One of these strings.
    Strings(&'static str, &'static [&'static str]),
    Alias(&'static str, T),
    /// Serde's default enum form: `{ "<variant>": <payload> }`.
    External(&'static str, &'static [(&'static str, T)]),
    /// Serde's `tag = "..."` form with fields shared by all variants.
    Internal {
        name: &'static str,
        common: Fields,
        tag: &'static str,
        variants: &'static [(&'static str, Fields)],
    },
    /// Serde's `tag = "..", content = ".."` form; a `None` payload has no content key.
    Adjacent {
        name: &'static str,
        tag: &'static str,
        content: &'static str,
        variants: &'static [(&'static str, Option<T>)],
    },
}

impl Def {
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Def::Object(n, _) | Def::Strings(n, _) | Def::Alias(n, _) | Def::External(n, _) => n,
            Def::Internal { name, .. } | Def::Adjacent { name, .. } => name,
        }
    }
}

const ORIGIN: T = T::Ref("Origin");
const DECL_ID: T = T::Ref("DeclId");
const OPT_STR: T = T::Nullable(&T::Str);
/// Omitted from the JSON when empty (`spec/rules.md` section 13).
const RULES: T = T::Optional(&T::List(&T::Ref("Rule")));
const EXPR: T = T::Ref("Expr");
const MODEL_FIELDS: Fields = &[
    ("parent", T::Nullable(&DECL_ID)),
    ("fields", T::List(&T::Ref("Field"))),
    ("rules", RULES),
];

const DECL_COMMON: Fields = &[
    ("id", DECL_ID),
    ("package", T::Ref("PackageId")),
    ("module", T::Str),
    ("name", T::Str),
    ("qualified", T::Str),
    ("sig", T::Str),
    ("docs", OPT_STR),
    ("deprecated", OPT_STR),
    ("origin", ORIGIN),
];

const DECL_KINDS: &[(&str, Fields)] = &[
    ("model", MODEL_FIELDS),
    (
        "error",
        &[
            ("default_status", T::Nullable(&T::Num)),
            ("parent", T::Nullable(&DECL_ID)),
            ("fields", T::List(&T::Ref("Field"))),
            ("rules", RULES),
        ],
    ),
    (
        "enum",
        &[
            ("variants", T::List(&T::Ref("Variant"))),
            ("flows", T::Optional(&T::List(&T::Ref("Flow")))),
            ("finals", T::Optional(&T::List(&T::Str))),
        ],
    ),
    (
        "union",
        &[
            ("discriminator", T::Str),
            ("arms", T::List(&T::Ref("UnionArm"))),
        ],
    ),
    (
        "const",
        &[("type", T::Ref("Type")), ("value", T::Ref("Literal"))],
    ),
];

const FIELD: Fields = &[
    ("name", T::Str),
    ("wire", T::Str),
    ("type", T::Ref("Type")),
    ("optional", T::Bool),
    ("default", T::Nullable(&T::Ref("Literal"))),
    ("readonly", T::Bool),
    ("inherited_from", T::Nullable(&DECL_ID)),
    ("constraints", T::Ref("Constraints")),
    ("example", T::Nullable(&T::Ref("Literal"))),
    ("docs", OPT_STR),
    ("deprecated", OPT_STR),
    ("origin", ORIGIN),
];

const ENDPOINT: Fields = &[
    ("name", T::Str),
    ("method", T::Ref("Method")),
    ("path", T::Str),
    ("sig", T::Str),
    ("docs", OPT_STR),
    ("deprecated", OPT_STR),
    ("auth", T::Ref("Auth")),
    ("middleware", T::List(&T::Str)),
    ("params", T::List(&T::Ref("Param"))),
    ("success", T::Ref("Success")),
    ("errors", T::List(&T::Ref("ErrorResponse"))),
    ("rules", T::Optional(&T::Ref("EndpointRules"))),
    ("origin", ORIGIN),
];

const SERVICE: Fields = &[
    ("id", T::Ref("ServiceId")),
    ("package", T::Ref("PackageId")),
    ("name", T::Str),
    ("prefix", T::Str),
    ("base_url", OPT_STR),
    ("docs", OPT_STR),
    ("auth", T::Ref("Auth")),
    ("middleware", T::List(&T::Str)),
    ("endpoints", T::List(&T::Ref("Endpoint"))),
    ("facts", T::Optional(&T::List(&T::Ref("ServiceFact")))),
    ("rules_digest", T::Optional(&T::Str)),
    ("principal", T::Optional(&T::Ref("ServicePrincipal"))),
    ("origin", ORIGIN),
];

/// The IR (`spec/ir.md`) and the host/plugin messages (`spec/plugin-protocol.md`).
pub const DEFS: &[Def] = &[
    Def::Object(
        "Ir",
        &[
            ("ir_version", T::Str),
            ("packages", T::List(&T::Ref("Package"))),
            ("decls", T::List(&T::Ref("Decl"))),
            ("services", T::List(&T::Ref("Service"))),
            ("consumes", T::Map(&T::List(&T::Str))),
            ("examples", T::Optional(&T::List(&T::Ref("Example")))),
            ("policies", T::Optional(&T::List(&T::Ref("Policy")))),
        ],
    ),
    Def::Alias("PackageId", T::Num),
    Def::Alias("DeclId", T::Num),
    Def::Alias("ServiceId", T::Num),
    Def::Alias("PolicyId", T::Num),
    Def::Object(
        "Package",
        &[
            ("id", T::Ref("PackageId")),
            ("name", T::Str),
            ("kind", T::Ref("PackageKind")),
        ],
    ),
    Def::Strings(
        "PackageKind",
        &["service", "library", "registry", "prelude"],
    ),
    Def::Object(
        "Origin",
        &[("file", T::Str), ("line", T::Num), ("col", T::Num)],
    ),
    Def::Internal {
        name: "Decl",
        common: DECL_COMMON,
        tag: "kind",
        variants: DECL_KINDS,
    },
    Def::Object(
        "Variant",
        &[
            ("name", T::Str),
            ("wire", T::Str),
            ("docs", OPT_STR),
            ("deprecated", OPT_STR),
        ],
    ),
    Def::Object("Flow", &[("from", T::Str), ("to", T::Str)]),
    Def::Object("UnionArm", &[("label", T::Str), ("decl", DECL_ID)]),
    Def::Object("Field", FIELD),
    Def::Strings(
        "Scalar",
        &[
            "string", "int", "long", "float", "decimal", "bool", "date", "datetime", "time",
            "uuid", "bytes", "json",
        ],
    ),
    Def::External(
        "Type",
        &[
            ("scalar", T::Ref("Scalar")),
            ("list", T::Ref("Type")),
            ("map", T::Ref("Type")),
            ("page", T::Ref("Type")),
            ("decl", DECL_ID),
        ],
    ),
    Def::Object(
        "Constraints",
        &[
            ("min", T::Nullable(&T::Num)),
            ("max", T::Nullable(&T::Num)),
            ("min_len", T::Nullable(&T::Num)),
            ("max_len", T::Nullable(&T::Num)),
            ("pattern", OPT_STR),
            ("format", T::Nullable(&T::Ref("StringFormat"))),
        ],
    ),
    Def::Strings(
        "StringFormat",
        &["email", "uri", "hostname", "ipv4", "ipv6"],
    ),
    Def::Adjacent {
        name: "Literal",
        tag: "lit",
        content: "value",
        variants: &[
            ("string", Some(T::Str)),
            ("int", Some(T::Num)),
            ("float", Some(T::Num)),
            ("bool", Some(T::Bool)),
            ("null", None),
            ("enum_variant", Some(T::Ref("EnumVariantLiteral"))),
            ("empty_list", None),
        ],
    },
    Def::Object("EnumVariantLiteral", &[("name", T::Str), ("wire", T::Str)]),
    // --- rules (spec/rules.md section 13) ---
    Def::Object(
        "Rule",
        &[
            ("name", T::Str),
            ("id", T::Str),
            ("expr", EXPR),
            ("message", OPT_STR),
            ("reads", T::List(&T::Str)),
            ("reads_readonly", T::Bool),
            ("inherited_from", T::Nullable(&DECL_ID)),
            ("docs", OPT_STR),
            ("deprecated", OPT_STR),
            ("origin", ORIGIN),
        ],
    ),
    Def::Object(
        "Expr",
        &[("t", T::Ref("ExprType")), ("e", T::Ref("ExprKind"))],
    ),
    Def::Alias("ExprType", T::Union(&[T::Ref("Type"), T::Lit("duration")])),
    Def::External(
        "ExprKind",
        &[
            ("lit", T::Ref("ExprLit")),
            ("path", T::Ref("PathRef")),
            ("now", T::Bool),
            ("not", EXPR),
            ("neg", EXPR),
            ("bin", T::Ref("BinExpr")),
            ("has", T::Ref("PathRef")),
            ("call", T::Ref("CallExpr")),
            ("quant", T::Ref("QuantExpr")),
            ("list", T::List(&EXPR)),
            ("widen", EXPR),
            ("old", EXPR),
        ],
    ),
    Def::External(
        "ExprLit",
        &[
            ("bool", T::Bool),
            ("int", T::Num),
            ("long", T::Str),
            ("float", T::Num),
            ("decimal", T::Str),
            ("string", T::Str),
            ("enum", T::Ref("EnumLit")),
        ],
    ),
    Def::Object(
        "EnumLit",
        &[("decl", DECL_ID), ("name", T::Str), ("wire", T::Str)],
    ),
    Def::Object(
        "PathRef",
        &[
            ("root", T::Ref("Root")),
            ("steps", T::List(&T::Ref("Step"))),
        ],
    ),
    Def::Object(
        "Step",
        &[("name", T::Str), ("wire", T::Str), ("optional", T::Bool)],
    ),
    Def::Alias(
        "Root",
        T::Union(&[
            T::Lit("self"),
            T::Lit("principal"),
            T::Lit("result"),
            T::Ref("RootRef"),
        ]),
    ),
    Def::External(
        "RootRef",
        &[("var", T::Str), ("param", T::Str), ("fact", T::Str)],
    ),
    Def::Object("BinExpr", &[("op", T::Ref("Op")), ("l", EXPR), ("r", EXPR)]),
    Def::Strings(
        "Op",
        &[
            "and", "or", "implies", "eq", "ne", "lt", "le", "gt", "ge", "in", "add", "sub", "mul",
            "div", "rem",
        ],
    ),
    Def::Object(
        "CallExpr",
        &[("f", T::Ref("Builtin")), ("args", T::List(&EXPR))],
    ),
    Def::Strings(
        "Builtin",
        &[
            "len",
            "matches",
            "starts_with",
            "ends_with",
            "contains",
            "days",
            "hours",
            "minutes",
            "seconds",
            "ms",
        ],
    ),
    Def::Object(
        "QuantExpr",
        &[
            ("q", T::Ref("Quantifier")),
            ("list", EXPR),
            ("var", T::Str),
            ("body", EXPR),
        ],
    ),
    Def::Strings("Quantifier", &["all", "any", "count", "sum"]),
    // --- endpoint rules and examples (spec/rules.md section 13.1) ---
    Def::Object(
        "EndpointRules",
        &[
            ("facts", T::List(&T::Ref("EndpointFact"))),
            ("requires", T::List(&T::Ref("EndpointRequire"))),
            ("ensures", T::List(&T::Ref("EndpointEnsure"))),
            ("transition", T::Optional(&T::Ref("EndpointTransition"))),
            ("allows", T::Optional(&T::List(&T::Ref("EndpointAllow")))),
        ],
    ),
    Def::Object(
        "EndpointTransition",
        &[
            ("id", T::Str),
            ("fact", T::Str),
            ("steps", T::List(&T::Ref("Step"))),
            ("decl", DECL_ID),
            ("from", T::List(&T::Str)),
            ("to", T::Str),
            ("origin", ORIGIN),
        ],
    ),
    Def::Object(
        "EndpointAllow",
        &[
            ("policy", T::Ref("PolicyId")),
            ("args", T::List(&EXPR)),
            ("reads", T::List(&T::Str)),
            ("origin", ORIGIN),
        ],
    ),
    Def::Object(
        "Policy",
        &[
            ("name", T::Str),
            ("package", T::Ref("PackageId")),
            ("module", T::Str),
            ("qualified", T::Str),
            ("id", T::Str),
            ("params", T::List(&T::Ref("PolicyParam"))),
            ("clauses", T::List(&T::Ref("PolicyClause"))),
            ("docs", OPT_STR),
            ("origin", ORIGIN),
        ],
    ),
    Def::Object("PolicyParam", &[("name", T::Str), ("type", T::Ref("Type"))]),
    Def::Object(
        "PolicyClause",
        &[
            ("effect", T::Ref("PolicyEffect")),
            ("expr", EXPR),
            ("message", OPT_STR),
            ("origin", ORIGIN),
        ],
    ),
    Def::Strings("PolicyEffect", &["permit", "forbid"]),
    Def::Object("ServicePrincipal", &[("decl", DECL_ID), ("name", T::Str)]),
    Def::Object(
        "EndpointFact",
        &[
            ("name", T::Str),
            ("type", T::Ref("Type")),
            ("optional", T::Bool),
            ("key", T::Nullable(&EXPR)),
            ("origin", ORIGIN),
        ],
    ),
    Def::Object(
        "EndpointRequire",
        &[
            ("name", OPT_STR),
            ("id", T::Str),
            ("expr", EXPR),
            ("message", OPT_STR),
            ("reads", T::List(&T::Str)),
            ("else", T::Nullable(&T::Ref("ElseError"))),
            ("docs", OPT_STR),
            ("origin", ORIGIN),
        ],
    ),
    Def::Object(
        "ElseError",
        &[
            ("decl", DECL_ID),
            ("status", T::Num),
            ("fields", T::List(&T::Ref("ElseField"))),
        ],
    ),
    Def::Object(
        "ElseField",
        &[("name", T::Str), ("wire", T::Str), ("value", EXPR)],
    ),
    Def::Object(
        "EndpointEnsure",
        &[
            ("name", OPT_STR),
            ("id", T::Str),
            ("expr", EXPR),
            ("message", OPT_STR),
            ("reads", T::List(&T::Str)),
            ("docs", OPT_STR),
            ("origin", ORIGIN),
        ],
    ),
    Def::Object(
        "ServiceFact",
        &[
            ("name", T::Str),
            ("type", T::Ref("Type")),
            ("key", T::Nullable(&T::Ref("Type"))),
            ("optional", T::Bool),
        ],
    ),
    Def::Object(
        "Example",
        &[
            ("name", T::Str),
            ("package", T::Ref("PackageId")),
            ("service", T::Ref("ServiceId")),
            ("endpoint", T::Str),
            ("givens", T::List(&T::Ref("ExampleGiven"))),
            ("principal", T::Nullable(&T::Ref("ExampleValue"))),
            ("now", OPT_STR),
            ("args", T::List(&T::Ref("ExampleArg"))),
            ("returns", T::Nullable(&T::Ref("ExampleValue"))),
            ("outcome", T::Ref("ExampleOutcome")),
            ("docs", OPT_STR),
            ("origin", ORIGIN),
        ],
    ),
    Def::Object(
        "ExampleGiven",
        &[
            ("fact", T::Str),
            ("value", T::Nullable(&T::Ref("ExampleValue"))),
        ],
    ),
    Def::Object(
        "ExampleArg",
        &[("param", T::Str), ("value", T::Ref("ExampleValue"))],
    ),
    Def::External(
        "ExampleOutcome",
        &[
            ("ok", T::Nullable(&T::Ref("ExampleValue"))),
            ("error", T::Ref("ExampleErrorOutcome")),
        ],
    ),
    Def::Object(
        "ExampleErrorOutcome",
        &[
            ("decl", DECL_ID),
            ("pattern", T::Nullable(&T::Ref("ExampleValue"))),
        ],
    ),
    Def::External(
        "ExampleValue",
        &[
            ("json", T::Any),
            ("ref", T::Ref("ExampleRef")),
            ("object", T::List(&T::Ref("ExampleField"))),
            ("list", T::List(&T::Ref("ExampleValue"))),
        ],
    ),
    Def::Object(
        "ExampleRef",
        &[("given", T::Str), ("path", T::List(&T::Str))],
    ),
    Def::Object(
        "ExampleField",
        &[
            ("name", T::Str),
            ("wire", T::Str),
            ("value", T::Ref("ExampleValue")),
        ],
    ),
    Def::Object("Service", SERVICE),
    Def::Strings(
        "Method",
        &[
            "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "WS",
        ],
    ),
    Def::Object("Endpoint", ENDPOINT),
    Def::Strings(
        "Binding",
        &["path", "query", "query_object", "header", "body"],
    ),
    Def::Object(
        "Param",
        &[
            ("name", T::Str),
            ("binding", T::Ref("Binding")),
            ("header", OPT_STR),
            ("type", T::Ref("Type")),
            ("optional", T::Bool),
            ("stream", T::Bool),
            ("constraints", T::Ref("Constraints")),
        ],
    ),
    Def::Object(
        "Success",
        &[
            ("status", T::Num),
            ("type", T::Nullable(&T::Ref("Type"))),
            ("stream", T::Bool),
        ],
    ),
    Def::Object("ErrorResponse", &[("status", T::Num), ("decl", DECL_ID)]),
    Def::Object("Auth", &[("kind", T::Ref("AuthKind")), ("header", OPT_STR)]),
    Def::Strings("AuthKind", &["none", "bearer", "basic", "api_key"]),
    // --- plugin protocol ---
    Def::Strings(
        "Feature",
        &[
            "unions",
            "constraints",
            "auth",
            "middleware",
            "headers",
            "streaming_sse",
            "streaming_ws",
            "page",
            "consts",
            "bytes",
            "rules_invariants",
            "rules_endpoint",
            "rules_transitions",
            "rules_policies",
        ],
    ),
    Def::Strings("Role", &["server", "client"]),
    Def::Object(
        "HostHello",
        &[
            ("type", T::Str),
            ("protocol", T::Str),
            ("veld_version", T::Str),
        ],
    ),
    Def::Object(
        "PluginHello",
        &[
            ("type", T::Str),
            ("protocol", T::Str),
            ("name", T::Str),
            ("version", T::Str),
            ("roles", T::List(&T::Ref("Role"))),
            ("capabilities", T::List(&T::Ref("Feature"))),
            ("options_schema", T::Optional(&T::Any)),
            ("frameworks", T::Optional(&T::Ref("FrameworkSupport"))),
            ("language", T::Optional(&T::Ref("LanguageSupport"))),
        ],
    ),
    Def::Object(
        "FrameworkSupport",
        &[
            ("server", T::Optional(&T::List(&T::Ref("FrameworkDecl")))),
            ("client", T::Optional(&T::List(&T::Ref("FrameworkDecl")))),
        ],
    ),
    Def::Object(
        "FrameworkDecl",
        &[
            ("id", T::Str),
            ("versions", T::Optional(&T::List(&T::Str))),
            ("min_language", T::Optional(&T::Str)),
        ],
    ),
    Def::Object(
        "LanguageSupport",
        &[("id", T::Str), ("versions", T::List(&T::Str))],
    ),
    Def::Object(
        "GenerateRequest",
        &[
            ("type", T::Str),
            ("role", T::Ref("Role")),
            ("ir", T::Ref("Ir")),
            ("services", T::List(&T::Ref("ServiceId"))),
            ("options", T::Map(&T::Any)),
            ("consumes", T::List(&T::Ref("ServiceId"))),
        ],
    ),
    Def::Object(
        "PluginFile",
        &[
            ("path", T::Str),
            ("content", T::Str),
            ("executable", T::Optional(&T::Bool)),
        ],
    ),
    Def::Strings("Severity", &["error", "warning"]),
    Def::Object(
        "PluginDiagnostic",
        &[
            ("code", T::Str),
            ("severity", T::Ref("Severity")),
            ("message", T::Str),
            ("ir_ref", T::Optional(&T::Str)),
        ],
    ),
    Def::Object(
        "GenerateResult",
        &[
            ("type", T::Str),
            ("files", T::List(&T::Ref("PluginFile"))),
            (
                "diagnostics",
                T::Optional(&T::List(&T::Ref("PluginDiagnostic"))),
            ),
        ],
    ),
];

/// The definition called `name`.
#[must_use]
pub fn find(name: &str) -> Option<&'static Def> {
    DEFS.iter().find(|d| d.name() == name)
}

// ---------------------------------------------------------------------------
// TypeScript
// ---------------------------------------------------------------------------

/// The schema as the contents of a TypeScript declaration file.
#[must_use]
pub fn typescript() -> String {
    let mut out = String::from(
        "// AUTO-GENERATED by `veld plugin new` from the IR schema (veld-ir). DO NOT EDIT.\n\
         // Shapes of the IR (spec/ir.md) and of the plugin protocol messages (spec/plugin-protocol.md).\n",
    );
    for def in DEFS {
        out.push('\n');
        out.push_str(&ts_def(def));
    }
    out
}

fn ts_type(t: &T) -> String {
    match t {
        T::Str => "string".into(),
        T::Num => "number".into(),
        T::Bool => "boolean".into(),
        T::Any => "unknown".into(),
        T::Ref(n) => (*n).to_string(),
        T::List(i) => match **i {
            T::Nullable(_) | T::Optional(_) => format!("({})[]", ts_type(i)),
            T::Union(_) => format!("({})[]", ts_type(i)),
            T::Str | T::Num | T::Bool | T::Any | T::Lit(_) | T::Ref(_) | T::List(_) | T::Map(_) => {
                format!("{}[]", ts_type(i))
            }
        },
        T::Map(i) => format!("Record<string, {}>", ts_type(i)),
        T::Nullable(i) => format!("{} | null", ts_type(i)),
        T::Optional(i) => ts_type(i),
        T::Union(items) => items.iter().map(ts_type).collect::<Vec<_>>().join(" | "),
        T::Lit(s) => format!("\"{s}\""),
    }
}

fn ts_field(name: &str, t: &T) -> String {
    let key = if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        name.to_string()
    } else {
        format!("\"{name}\"")
    };
    let opt = if matches!(t, T::Optional(_)) { "?" } else { "" };
    format!("{key}{opt}: {}", ts_type(t))
}

fn ts_object_body(fields: &[(&str, T)], extra: &[String], indent: &str) -> String {
    let mut lines: Vec<String> = extra.to_vec();
    lines.extend(fields.iter().map(|(n, t)| ts_field(n, t)));
    lines.iter().map(|l| format!("{indent}{l};\n")).collect()
}

fn ts_def(def: &Def) -> String {
    match def {
        Def::Object(name, fields) => format!(
            "export interface {name} {{\n{}}}\n",
            ts_object_body(fields, &[], "  ")
        ),
        Def::Strings(name, values) => {
            let items: Vec<String> = values.iter().map(|v| format!("\"{v}\"")).collect();
            format!("export type {name} = {};\n", items.join(" | "))
        }
        Def::Alias(name, t) => format!("export type {name} = {};\n", ts_type(t)),
        Def::External(name, variants) => {
            let arms: String = variants
                .iter()
                .map(|(k, t)| format!("  | {{ {k}: {} }}\n", ts_type(t)))
                .collect();
            format!("export type {name} =\n{arms}")
        }
        Def::Internal {
            name,
            common,
            tag,
            variants,
        } => ts_internal(name, common, tag, variants),
        Def::Adjacent {
            name,
            tag,
            content,
            variants,
        } => {
            let arms: String = variants
                .iter()
                .map(|(v, t)| match t {
                    Some(t) => format!("  | {{ {tag}: \"{v}\"; {content}: {} }}\n", ts_type(t)),
                    None => format!("  | {{ {tag}: \"{v}\" }}\n"),
                })
                .collect();
            format!("export type {name} =\n{arms}")
        }
    }
}

fn ts_internal(name: &str, common: Fields, tag: &str, variants: &[(&str, Fields)]) -> String {
    let mut out = format!("export type {name} =\n");
    for (value, fields) in variants {
        let mut all: Vec<(&str, T)> = common.to_vec();
        all.extend(fields.iter().copied());
        out.push_str("  | {\n");
        out.push_str(&ts_object_body(
            &all,
            &[format!("{tag}: \"{value}\"")],
            "      ",
        ));
        out.push_str("    }\n");
    }
    out
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Checks that `value` has exactly the shape of the definition `root`. The error names the
/// path of the first mismatch, for example `Ir.decls[3].fields[0]: unknown key "foo"`.
pub fn check(root: &str, value: &Value) -> Result<(), String> {
    let def = find(root).ok_or_else(|| format!("the schema has no type `{root}`"))?;
    check_def(def, value, root)
}

fn check_ref(name: &str, value: &Value, path: &str) -> Result<(), String> {
    let def = find(name)
        .ok_or_else(|| format!("{path}: the schema refers to the undefined type `{name}`"))?;
    check_def(def, value, path)
}

fn check_def(def: &Def, value: &Value, path: &str) -> Result<(), String> {
    match def {
        Def::Object(_, fields) => check_object(fields, &[], value, path),
        Def::Strings(_, values) => match value.as_str() {
            Some(s) if values.contains(&s) => Ok(()),
            _ => Err(format!("{path}: expected one of {values:?}, found {value}")),
        },
        Def::Alias(_, t) => check_ty(t, value, path),
        Def::External(_, variants) => check_external(variants, value, path),
        Def::Internal {
            common,
            tag,
            variants,
            ..
        } => {
            let kind = value.get(*tag).and_then(Value::as_str).unwrap_or_default();
            let (_, fields) = variants
                .iter()
                .find(|(v, _)| *v == kind)
                .ok_or_else(|| format!("{path}: unknown `{tag}` value {:?}", value.get(*tag)))?;
            let mut all: Vec<(&str, T)> = common.to_vec();
            all.extend(fields.iter().copied());
            check_object(&all, &[tag], value, path)
        }
        Def::Adjacent {
            tag,
            content,
            variants,
            ..
        } => check_adjacent(tag, content, variants, value, path),
    }
}

fn check_external(variants: &[(&str, T)], value: &Value, path: &str) -> Result<(), String> {
    let obj = value
        .as_object()
        .filter(|o| o.len() == 1)
        .ok_or_else(|| format!("{path}: expected a one-key object"))?;
    let (key, payload) = obj
        .iter()
        .next()
        .ok_or_else(|| format!("{path}: empty object"))?;
    let (_, t) = variants
        .iter()
        .find(|(k, _)| k == key)
        .ok_or_else(|| format!("{path}: unknown variant `{key}`"))?;
    check_ty(t, payload, &format!("{path}.{key}"))
}

fn check_adjacent(
    tag: &str,
    content: &str,
    variants: &[(&str, Option<T>)],
    value: &Value,
    path: &str,
) -> Result<(), String> {
    let kind = value.get(tag).and_then(Value::as_str).unwrap_or_default();
    let (_, payload) = variants
        .iter()
        .find(|(v, _)| *v == kind)
        .ok_or_else(|| format!("{path}: unknown `{tag}` value {:?}", value.get(tag)))?;
    let obj = value
        .as_object()
        .ok_or_else(|| format!("{path}: expected an object"))?;
    match payload {
        Some(t) => {
            let inner = obj
                .get(content)
                .ok_or_else(|| format!("{path}: missing `{content}`"))?;
            if obj.len() != 2 {
                return Err(format!(
                    "{path}: unexpected keys next to `{tag}` and `{content}`"
                ));
            }
            check_ty(t, inner, &format!("{path}.{content}"))
        }
        None if obj.len() == 1 => Ok(()),
        None => Err(format!("{path}: `{kind}` carries no `{content}`")),
    }
}

fn check_object(
    fields: &[(&str, T)],
    tags: &[&str],
    value: &Value,
    path: &str,
) -> Result<(), String> {
    let obj = value
        .as_object()
        .ok_or_else(|| format!("{path}: expected an object, found {value}"))?;
    for key in obj.keys() {
        if !fields.iter().any(|(n, _)| n == key) && !tags.contains(&key.as_str()) {
            return Err(format!(
                "{path}: unknown key \"{key}\" (add it to veld_ir::schema)"
            ));
        }
    }
    for (name, t) in fields {
        match (obj.get(*name), t) {
            (None, T::Optional(_)) => {}
            (None, _) => {
                return Err(format!(
                    "{path}: missing key \"{name}\" (remove it from veld_ir::schema?)"
                ));
            }
            (Some(v), t) => check_ty(t, v, &format!("{path}.{name}"))?,
        }
    }
    Ok(())
}

fn check_ty(t: &T, v: &Value, path: &str) -> Result<(), String> {
    let wrong = |what: &str| Err(format!("{path}: expected {what}, found {v}"));
    match t {
        T::Str if v.is_string() => Ok(()),
        T::Num if v.is_number() => Ok(()),
        T::Bool if v.is_boolean() => Ok(()),
        T::Any => Ok(()),
        T::Str => wrong("a string"),
        T::Num => wrong("a number"),
        T::Bool => wrong("a boolean"),
        T::Lit(s) if v.as_str() == Some(*s) => Ok(()),
        T::Lit(s) => wrong(&format!("the string \"{s}\"")),
        T::Union(items) => {
            if items.iter().any(|t| check_ty(t, v, path).is_ok()) {
                Ok(())
            } else {
                wrong("one of the union's types")
            }
        }
        T::Ref(n) => check_ref(n, v, path),
        T::Nullable(_) if v.is_null() => Ok(()),
        T::Nullable(i) | T::Optional(i) => check_ty(i, v, path),
        T::List(i) => match v.as_array() {
            Some(items) => items
                .iter()
                .enumerate()
                .try_for_each(|(k, x)| check_ty(i, x, &format!("{path}[{k}]"))),
            None => wrong("an array"),
        },
        T::Map(i) => match v.as_object() {
            Some(o) => o
                .iter()
                .try_for_each(|(k, x)| check_ty(i, x, &format!("{path}.{k}"))),
            None => wrong("an object"),
        },
    }
}

#[cfg(test)]
mod tests;
