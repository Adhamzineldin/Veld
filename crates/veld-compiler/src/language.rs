//! Single source of truth for the Veld language surface: keywords, scalar
//! names, annotations and the prelude. Editor grammars are generated from
//! [`spec`].

use serde::Serialize;

/// Reserved words. They can never be identifiers.
pub const RESERVED: [&str; 14] = [
    "use", "as", "model", "enum", "error", "union", "service", "at", "extends", "const", "stream",
    "true", "false", "null",
];

/// Contextual words. Keywords only where the grammar expects an HTTP method.
pub const METHOD_WORDS: [&str; 8] = [
    "get", "post", "put", "patch", "delete", "head", "options", "ws",
];

/// Contextual words of the rules language (`spec/rules.md` section 2.2). Each is a keyword only
/// where the grammar expects it and an ordinary identifier everywhere else, so a field, variant,
/// parameter, endpoint or declaration may carry any of them. The reserved list never grows.
pub const CONTEXTUAL_RULE_WORDS: [&str; 38] = [
    "rule",
    "message",
    "implies",
    "not",
    "in",
    "has",
    "fact",
    "require",
    "else",
    "ensure",
    "old",
    "now",
    "result",
    "flow",
    "final",
    "emits",
    "transition",
    "after",
    "since",
    "flag",
    "off",
    "policy",
    "permit",
    "forbid",
    "when",
    "allow",
    "principal",
    "example",
    "given",
    "expect",
    "absent",
    "returns",
    "ok",
    "ifMatch",
    "any",
    "stored",
    "other",
    "running",
];

/// Contextual words of events (`spec/events.md` section 2). `emits`, `given` and `expect` are rule
/// words already. Each is a keyword only where the grammar expects it; the reserved list never grows.
pub const CONTEXTUAL_EVENT_WORDS: [&str; 9] = [
    "event",
    "channel",
    "carries",
    "key",
    "publishes",
    "subscribes",
    "on",
    "called",
    "rejected",
];

/// Words that start a declaration.
pub const DECL_KEYWORDS: [&str; 6] = ["model", "enum", "error", "union", "service", "const"];

/// v1 keywords that are syntax errors in v2 (`E0001`).
pub const V1_KEYWORDS: [&str; 3] = ["import", "module", "action"];

/// Built-in generic type names.
pub const GENERICS: [&str; 2] = ["Map", "Page"];

/// Scalar type names, in spec order.
pub const SCALARS: [&str; 12] = [
    "string", "int", "long", "float", "decimal", "bool", "date", "datetime", "time", "uuid",
    "bytes", "json",
];

#[must_use]
pub fn is_reserved(word: &str) -> bool {
    RESERVED.contains(&word)
}

#[must_use]
pub fn is_method_word(word: &str) -> bool {
    METHOD_WORDS.contains(&word)
}

#[must_use]
pub fn is_decl_keyword(word: &str) -> bool {
    DECL_KEYWORDS.contains(&word)
}

/// True when `s` is a valid, non-reserved identifier.
#[must_use]
pub fn is_valid_ident(s: &str) -> bool {
    let mut chars = s.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    first_ok && chars.all(|c| c.is_ascii_alphanumeric() || c == '_') && !is_reserved(s)
}

/// Where an annotation may be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Service,
    Endpoint,
    Model,
    Error,
    Enum,
    Union,
    Const,
    Field,
    Variant,
    Param,
    Rule,
    /// A `require` statement of an endpoint block (`spec/rules.md` section 17.1).
    Require,
    /// An `ensure` statement of an endpoint block.
    Ensure,
    Event,
    Channel,
}

/// What arguments an annotation takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArgShape {
    None,
    OneString,
    OneNumber,
    OneLiteral,
    Auth,
    Idents,
    Length,
    Format,
    /// The word `audit`.
    Rollout,
    /// Nothing, or `header: "Name"` and `required: true|false` in either order.
    Idempotent,
    /// One constant duration: `seconds(2)` (`spec/platform.md` section 12).
    Timeout,
    /// Named objectives: `p50`, `p90`, `p95`, `p99` (durations) and `errors` (a percent), at
    /// least one (`spec/platform.md` section 13).
    Slo,
}

#[derive(Debug, Clone, Copy)]
pub struct AnnotationSpec {
    pub name: &'static str,
    pub targets: &'static [Target],
    pub args: ArgShape,
    pub usage: &'static str,
}

use Target::{
    Channel, Const, Endpoint, Ensure, Enum, Error, Event, Field, Model, Param, Require, Rule,
    Service, Union, Variant,
};

const ANY_DECL: [Target; 7] = [Service, Model, Error, Enum, Union, Const, Endpoint];
const DEPRECATABLE: [Target; 12] = [
    Service, Model, Error, Enum, Union, Const, Endpoint, Field, Variant, Rule, Event, Channel,
];

/// The annotation table of `spec/language.md` section 7.
pub const ANNOTATIONS: [AnnotationSpec; 20] = [
    AnnotationSpec {
        name: "auth",
        targets: &[Service, Endpoint],
        args: ArgShape::Auth,
        usage: "@auth(bearer | basic | none | apiKey(header: \"X\"))",
    },
    AnnotationSpec {
        name: "middleware",
        targets: &[Service, Endpoint],
        args: ArgShape::Idents,
        usage: "@middleware(Name, ...)",
    },
    AnnotationSpec {
        name: "deprecated",
        targets: &DEPRECATABLE,
        args: ArgShape::OneString,
        usage: "@deprecated(\"message\")",
    },
    AnnotationSpec {
        name: "min",
        targets: &[Field, Param],
        args: ArgShape::OneNumber,
        usage: "@min(n)",
    },
    AnnotationSpec {
        name: "max",
        targets: &[Field, Param],
        args: ArgShape::OneNumber,
        usage: "@max(n)",
    },
    AnnotationSpec {
        name: "length",
        targets: &[Field, Param],
        args: ArgShape::Length,
        usage: "@length(min: n, max: n)",
    },
    AnnotationSpec {
        name: "pattern",
        targets: &[Field, Param],
        args: ArgShape::OneString,
        usage: "@pattern(\"regex\")",
    },
    AnnotationSpec {
        name: "format",
        targets: &[Field, Param],
        args: ArgShape::Format,
        usage: "@format(email | uri | hostname | ipv4 | ipv6)",
    },
    AnnotationSpec {
        name: "readonly",
        targets: &[Field],
        args: ArgShape::None,
        usage: "@readonly",
    },
    AnnotationSpec {
        name: "wire",
        targets: &[Field],
        args: ArgShape::OneString,
        usage: "@wire(\"name\")",
    },
    AnnotationSpec {
        name: "example",
        targets: &[Field, Param],
        args: ArgShape::OneLiteral,
        usage: "@example(literal)",
    },
    AnnotationSpec {
        name: "discriminator",
        targets: &[Union],
        args: ArgShape::OneString,
        usage: "@discriminator(\"prop\")",
    },
    AnnotationSpec {
        name: "query",
        targets: &[Param],
        args: ArgShape::None,
        usage: "@query",
    },
    AnnotationSpec {
        name: "header",
        targets: &[Param],
        args: ArgShape::OneString,
        usage: "@header(\"Name\")",
    },
    AnnotationSpec {
        name: "topic",
        targets: &[Channel],
        args: ArgShape::OneString,
        usage: "@topic(\"name\")",
    },
    AnnotationSpec {
        name: "rollout",
        targets: &[Rule, Require, Ensure],
        args: ArgShape::Rollout,
        usage: "@rollout(audit)",
    },
    AnnotationSpec {
        name: "version",
        targets: &[Field],
        args: ArgShape::None,
        usage: "@version",
    },
    AnnotationSpec {
        name: "idempotent",
        targets: &[Endpoint],
        args: ArgShape::Idempotent,
        usage: "@idempotent(header: \"Idempotency-Key\", required: true)",
    },
    AnnotationSpec {
        name: "timeout",
        targets: &[Service, Endpoint],
        args: ArgShape::Timeout,
        usage: "@timeout(seconds(2))",
    },
    AnnotationSpec {
        name: "slo",
        targets: &[Service, Endpoint],
        args: ArgShape::Slo,
        usage: "@slo(p99: ms(300), errors: 0.1)",
    },
];

/// Looks an annotation up by name.
#[must_use]
pub fn annotation(name: &str) -> Option<&'static AnnotationSpec> {
    ANNOTATIONS.iter().find(|a| a.name == name)
}

/// Annotations that apply to any declaration kind (kept for tooling).
#[must_use]
pub fn any_decl_targets() -> &'static [Target] {
    &ANY_DECL
}

/// Prelude source text (`spec/language.md` section 5.5), package `veld`.
pub const PRELUDE_SOURCE: &str = "\
error BadRequest 400      { message: string }
error Unauthorized 401    { message?: string }
error Forbidden 403       { message?: string }
error NotFound 404        { resource?: string }
error Conflict 409        { message: string }
error ValidationError 422 { issues: ValidationIssue[] }
error TooManyRequests 429 { retryAfter?: int }
error Internal 500        { message?: string }
model ValidationIssue     { path: string, code: string, message: string }
error InvalidTransition 409 { field: string, from: string, to: string }
error PreconditionFailed 412 { expected: long, actual: long }
error DeadlineExceeded 504 { timeoutMs: long }
";

/// Name of the reserved prelude package.
pub const PRELUDE_PACKAGE: &str = "veld";
/// Module path of the prelude.
pub const PRELUDE_MODULE: &str = "prelude";
/// Relative file name of the prelude source.
pub const PRELUDE_FILE: &str = "prelude.veld";

#[derive(Debug, Clone, Serialize)]
pub struct AnnotationOut {
    pub name: &'static str,
    pub targets: Vec<Target>,
    pub args: ArgShape,
    pub usage: &'static str,
}

/// Serializable description of the language for editor grammar generation.
#[derive(Debug, Clone, Serialize)]
pub struct LanguageSpec {
    pub reserved: Vec<&'static str>,
    pub contextual: Vec<&'static str>,
    /// Contextual words of the rules language (`spec/rules.md` section 2.2).
    pub rule_words: Vec<&'static str>,
    /// Contextual words of events (`spec/events.md` section 2).
    pub event_words: Vec<&'static str>,
    pub scalars: Vec<&'static str>,
    pub generics: Vec<&'static str>,
    pub annotations: Vec<AnnotationOut>,
    pub prelude: &'static str,
}

#[must_use]
pub fn spec() -> LanguageSpec {
    let annotations: Vec<AnnotationOut> = ANNOTATIONS.iter().map(out).collect();
    LanguageSpec {
        reserved: RESERVED.to_vec(),
        contextual: METHOD_WORDS.to_vec(),
        rule_words: CONTEXTUAL_RULE_WORDS.to_vec(),
        event_words: CONTEXTUAL_EVENT_WORDS.to_vec(),
        scalars: SCALARS.to_vec(),
        generics: GENERICS.to_vec(),
        annotations,
        prelude: PRELUDE_SOURCE,
    }
}

fn out(a: &AnnotationSpec) -> AnnotationOut {
    AnnotationOut {
        name: a.name,
        targets: a.targets.to_vec(),
        args: a.args,
        usage: a.usage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars_match_the_ir() {
        for s in veld_ir::Scalar::ALL {
            assert!(SCALARS.contains(&s.name()));
        }
        assert_eq!(SCALARS.len(), veld_ir::Scalar::ALL.len());
    }

    #[test]
    fn identifiers_reject_reserved_words() {
        assert!(is_valid_ident("get"));
        assert!(!is_valid_ident("model"));
        assert!(!is_valid_ident("1a"));
        assert!(!is_valid_ident("a-b"));
    }

    #[test]
    fn spec_serializes_and_lists_every_annotation() {
        let json = serde_json::to_value(spec()).map(|v| v["annotations"].as_array().map(Vec::len));
        assert_eq!(json.ok().flatten(), Some(18));
    }

    #[test]
    fn rule_words_are_contextual_never_reserved() {
        for w in CONTEXTUAL_RULE_WORDS {
            assert!(!is_reserved(w), "{w}");
            assert!(is_valid_ident(w), "{w}");
        }
        assert_eq!(spec().rule_words.len(), CONTEXTUAL_RULE_WORDS.len());
    }

    #[test]
    fn only_deprecated_and_rollout_apply_to_a_rule() {
        let on_rule: Vec<&str> = ANNOTATIONS
            .iter()
            .filter(|a| a.targets.contains(&Target::Rule))
            .map(|a| a.name)
            .collect();
        assert_eq!(on_rule, vec!["deprecated", "rollout"]);
    }

    #[test]
    fn annotation_lookup_finds_readonly() {
        assert!(annotation("readonly").is_some());
        assert!(annotation("nope").is_none());
    }
}
