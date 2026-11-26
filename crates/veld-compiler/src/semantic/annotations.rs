//! Annotation checking (`spec/language.md` section 7): E0601 unknown,
//! E0602 wrong target or arguments, E0603 pattern outside the regex subset.

use super::regex;
use crate::diagnostics::{Diagnostic, Span};
use crate::language::{self, ArgShape, Target};
use crate::syntax::ast::{Annotation, Arg, ArgValue, LitKind, Literal};
use veld_ir::retention::RetainThen;
use veld_ir::{Auth, AuthKind, Constraints, Rollout, Scalar, StringFormat, Type};

/// The meaning of the annotations written on one node.
#[derive(Debug, Default)]
pub struct Anns<'a> {
    pub deprecated: Option<String>,
    pub readonly: bool,
    pub wire: Option<String>,
    pub example: Option<&'a Literal>,
    pub constraints: Constraints,
    pub auth: Option<Auth>,
    pub middleware: Option<Vec<String>>,
    pub discriminator: Option<String>,
    pub query: Option<Span>,
    pub header: Option<(String, Span)>,
    pub topic: Option<String>,
    /// `@rollout(audit)` on a rule, a `require` or an `ensure`.
    pub rollout: Option<Rollout>,
    /// `@version` on a field.
    pub version: Option<Span>,
    /// `@pii` and `@secret` on a field (`spec/rules.md` section 19.3).
    pub pii: bool,
    pub secret: bool,
    /// `@idempotent` on an endpoint.
    pub idempotent: Option<Idempotent>,
    /// `@retain` on a model (`spec/platform.md` section 23).
    pub retain: Option<RetainAnn>,
}

/// What `@retain(days(30), since: f, then: anonymize)` says, before the field is looked up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainAnn {
    /// The duration, in milliseconds.
    pub after_ms: i64,
    /// The source name of the `since` field.
    pub since: String,
    pub since_span: Span,
    pub then: RetainThen,
    pub span: Span,
}

/// What `@idempotent(header: "Name", required: true)` says (`spec/rules.md` section 11.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Idempotent {
    /// The header, `None` for the default `Idempotency-Key`.
    pub header: Option<String>,
    pub required: bool,
    pub span: Span,
}

/// The header of `@idempotent` without a `header:` argument.
pub const DEFAULT_IDEMPOTENCY_HEADER: &str = "Idempotency-Key";

#[must_use]
pub fn target_name(t: Target) -> &'static str {
    match t {
        Target::Service => "service",
        Target::Endpoint => "endpoint",
        Target::Model => "model",
        Target::Error => "error",
        Target::Enum => "enum",
        Target::Union => "union",
        Target::Const => "const",
        Target::Field => "field",
        Target::Variant => "enum variant",
        Target::Rule => "rule",
        Target::Require => "require",
        Target::Ensure => "ensure",
        Target::Param => "parameter",
        Target::Event => "event",
        Target::Channel => "channel",
    }
}

/// Validates `anns` for `target`. `ty` is the resolved type of the annotated
/// field or parameter, used for applicability checks such as `@min`.
pub fn parse<'a>(
    anns: &'a [Annotation],
    target: Target,
    ty: Option<&Type>,
    diags: &mut Vec<Diagnostic>,
) -> Anns<'a> {
    let mut out = Anns::default();
    let mut seen: Vec<&str> = Vec::new();
    for a in anns {
        let name = a.name.name.as_str();
        let Some(spec) = language::annotation(name) else {
            diags.push(unknown(a, target));
            continue;
        };
        if !spec.targets.contains(&target) {
            diags.push(wrong_target(a, spec.targets, target));
            continue;
        }
        if seen.contains(&name) {
            diags.push(Diagnostic::error(
                "E0602",
                format!("`@{name}` is written more than once"),
                a.span,
            ));
            continue;
        }
        seen.push(name);
        if let Err(msg) = apply(&mut out, a, spec, ty, diags) {
            diags.push(
                Diagnostic::error("E0602", msg, a.span).with_help(format!("usage: {}", spec.usage)),
            );
        }
    }
    check_bounds(&out, anns, diags);
    out
}

/// True when `spec` applies only to channels or events and `target` is neither, or only to
/// rules and `target` is not one: such an annotation is not suggested for a model, a field or
/// an endpoint.
fn hidden_from(spec: &language::AnnotationSpec, target: Target) -> bool {
    let events_only = |t: &Target| matches!(t, Target::Channel | Target::Event);
    let rules_only = |t: &Target| matches!(t, Target::Rule | Target::Require | Target::Ensure);
    // `@version` and `@idempotent` are suggested only where they apply, so that the help of an
    // unknown annotation on a model or a service reads as it did before they existed.
    let own_target_only = matches!(
        spec.name,
        "version" | "idempotent" | "pii" | "secret" | "retain"
    );
    (!events_only(&target) && spec.targets.iter().all(events_only))
        || (!rules_only(&target) && spec.targets.iter().all(rules_only))
        || (own_target_only && !spec.targets.contains(&target))
}

fn unknown(a: &Annotation, target: Target) -> Diagnostic {
    let visible: Vec<&language::AnnotationSpec> = language::ANNOTATIONS
        .iter()
        .filter(|s| !hidden_from(s, target))
        .collect();
    let known: Vec<String> = visible.iter().map(|s| format!("@{}", s.name)).collect();
    Diagnostic::error(
        "E0601",
        format!("unknown annotation `@{}`", a.name.name),
        a.name.span,
    )
    .did_you_mean(
        &a.name.name,
        visible.iter().map(|s| s.name),
        Some(format!("known annotations: {}", known.join(", "))),
    )
}

/// `an` before a vowel sound, `a` otherwise: `an enum`, `an ensure`, `a rule`.
fn article(word: &str) -> &'static str {
    if word.starts_with(['a', 'e', 'i', 'o']) {
        "an"
    } else {
        "a"
    }
}

fn wrong_target(a: &Annotation, allowed: &[Target], got: Target) -> Diagnostic {
    let list: Vec<&str> = allowed.iter().map(|t| target_name(*t)).collect();
    Diagnostic::error(
        "E0602",
        format!(
            "`@{}` cannot be applied to {} {}",
            a.name.name,
            article(target_name(got)),
            target_name(got)
        ),
        a.span,
    )
    .with_help(format!(
        "`@{}` applies to: {}",
        a.name.name,
        list.join(", ")
    ))
}

fn check_bounds(out: &Anns<'_>, anns: &[Annotation], diags: &mut Vec<Diagnostic>) {
    let c = &out.constraints;
    let span = anns.last().map_or_else(Span::none, |a| a.span);
    if let (Some(lo), Some(hi)) = (c.min, c.max)
        && lo > hi
    {
        diags.push(Diagnostic::error(
            "E0602",
            format!("`@min({lo})` is greater than `@max({hi})`"),
            span,
        ));
    }
}

fn apply<'a>(
    out: &mut Anns<'a>,
    a: &'a Annotation,
    spec: &language::AnnotationSpec,
    ty: Option<&Type>,
    diags: &mut Vec<Diagnostic>,
) -> Result<(), String> {
    let args = a.args.as_deref().unwrap_or(&[]);
    match (spec.name, spec.args) {
        (_, ArgShape::None) => no_args(args, spec.name)?,
        ("deprecated", _) => out.deprecated = Some(one_string(args)?),
        ("wire", _) => {
            let w = one_string(args)?;
            if w.is_empty() {
                return Err("the wire name must not be empty".to_string());
            }
            out.wire = Some(w);
        }
        ("pattern", _) => {
            require_type(ty, is_string, "`@pattern` applies to string fields")?;
            let p = one_string(args)?;
            if let Err(reason) = regex::check(&p) {
                diags.push(
                    Diagnostic::error("E0603", format!("pattern `{p}` is outside the portable regex subset: {reason}"), a.span)
                        .with_help("allowed: literals, `.`, `[...]`, `\\d \\w \\s`, `* + ? {n,m}`, `( )`, `|`, `^ $`"),
                );
            }
            out.constraints.pattern = Some(p);
        }
        ("format", _) => {
            require_type(ty, is_string, "`@format` applies to string fields")?;
            out.constraints.format = Some(format_arg(args)?);
        }
        ("min" | "max", _) => {
            require_type(ty, is_numeric, "`@min` and `@max` apply to numeric fields")?;
            let n = one_number(args)?;
            if spec.name == "min" {
                out.constraints.min = Some(n)
            } else {
                out.constraints.max = Some(n)
            }
        }
        ("length", _) => {
            require_type(ty, is_sized, "`@length` applies to strings, lists and maps")?;
            length_args(&mut out.constraints, args)?;
        }
        ("example", _) => out.example = Some(one_literal(args)?),
        ("discriminator", _) => out.discriminator = Some(one_string(args)?),
        ("header", _) => out.header = Some((one_string(args)?, a.span)),
        ("topic", _) => out.topic = Some(one_string(args)?),
        ("rollout", _) => out.rollout = Some(rollout_arg(args)?),
        ("idempotent", _) => out.idempotent = Some(idempotent_args(args, a.span)?),
        ("retain", _) => out.retain = retain_args(args, a.span, diags),
        ("auth", _) => out.auth = Some(auth_arg(args)?.declared()),
        ("middleware", _) => out.middleware = Some(idents(args)?),
        _ => {}
    }
    match spec.name {
        "readonly" => out.readonly = true,
        "pii" => out.pii = true,
        "secret" => out.secret = true,
        "version" => {
            require_type(
                ty,
                is_integer,
                "`@version` applies to `int` and `long` fields",
            )?;
            out.version = Some(a.span);
        }
        "query" => out.query = Some(a.span),
        _ => {}
    }
    Ok(())
}

fn no_args(args: &[Arg], name: &str) -> Result<(), String> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(format!("`@{name}` takes no arguments"))
    }
}

fn require_type(ty: Option<&Type>, pred: fn(&Type) -> bool, msg: &str) -> Result<(), String> {
    match ty {
        Some(t) if !pred(t) => Err(msg.to_string()),
        _ => Ok(()),
    }
}

fn is_string(t: &Type) -> bool {
    matches!(t, Type::Scalar(Scalar::String))
}

fn is_numeric(t: &Type) -> bool {
    matches!(t, Type::Scalar(s) if s.is_numeric())
}

fn is_integer(t: &Type) -> bool {
    matches!(t, Type::Scalar(Scalar::Int | Scalar::Long))
}

fn is_sized(t: &Type) -> bool {
    matches!(
        t,
        Type::Scalar(Scalar::String) | Type::List(_) | Type::Map(_)
    )
}

fn positional(args: &[Arg]) -> Result<Vec<&Arg>, String> {
    if args.iter().any(|a| a.name.is_some()) {
        return Err("named arguments are not accepted here".to_string());
    }
    Ok(args.iter().collect())
}

fn one(args: &[Arg]) -> Result<&Arg, String> {
    match positional(args)?.as_slice() {
        [a] => Ok(a),
        _ => Err("expected exactly one argument".to_string()),
    }
}

fn literal_of(a: &Arg) -> Option<&Literal> {
    match &a.value {
        ArgValue::Literal(l) => Some(l),
        ArgValue::Ident(_) | ArgValue::Call { .. } => None,
    }
}

fn one_string(args: &[Arg]) -> Result<String, String> {
    match literal_of(one(args)?).map(|l| &l.kind) {
        Some(LitKind::Str(s)) => Ok(s.clone()),
        _ => Err("expected a string argument".to_string()),
    }
}

fn one_number(args: &[Arg]) -> Result<f64, String> {
    match literal_of(one(args)?).map(|l| &l.kind) {
        #[allow(clippy::cast_precision_loss)]
        Some(LitKind::Int(n)) => Ok(*n as f64),
        Some(LitKind::Float(f)) => Ok(*f),
        _ => Err("expected a number argument".to_string()),
    }
}

fn one_literal(args: &[Arg]) -> Result<&Literal, String> {
    literal_of(one(args)?).ok_or_else(|| "expected a literal argument".to_string())
}

fn format_arg(args: &[Arg]) -> Result<StringFormat, String> {
    match &one(args)?.value {
        ArgValue::Ident(i) => StringFormat::from_name(&i.name).ok_or_else(|| {
            format!(
                "unknown format `{}`; use email, uri, hostname, ipv4 or ipv6",
                i.name
            )
        }),
        ArgValue::Literal(_) | ArgValue::Call { .. } => {
            Err("expected one of email, uri, hostname, ipv4, ipv6".to_string())
        }
    }
}

fn length_args(c: &mut Constraints, args: &[Arg]) -> Result<(), String> {
    if args.is_empty() {
        return Err("expected `min:` and/or `max:`".to_string());
    }
    for a in args {
        let key = a.name.as_ref().map(|n| n.name.as_str());
        let value = match literal_of(a).map(|l| &l.kind) {
            Some(LitKind::Int(n)) if *n >= 0 => n.unsigned_abs(),
            _ => return Err("length bounds must be non-negative integers".to_string()),
        };
        match key {
            Some("min") => c.min_len = Some(value),
            Some("max") => c.max_len = Some(value),
            _ => return Err("expected `min:` and/or `max:`".to_string()),
        }
    }
    if let (Some(lo), Some(hi)) = (c.min_len, c.max_len)
        && lo > hi
    {
        return Err(format!("`min: {lo}` is greater than `max: {hi}`"));
    }
    Ok(())
}

/// `@rollout(audit)`: the one word `audit` (`spec/rules.md` section 17.1).
fn rollout_arg(args: &[Arg]) -> Result<Rollout, String> {
    match &one(args)?.value {
        ArgValue::Ident(i) if i.name == "audit" => Ok(Rollout::Audit),
        ArgValue::Ident(i) => Err(format!(
            "unknown rollout `{}`; the only one is `audit`",
            i.name
        )),
        ArgValue::Literal(_) | ArgValue::Call { .. } => {
            Err("expected the word `audit`".to_string())
        }
    }
}

const RETAIN_USAGE: &str = "@retain(days(30), since: checkedOutAt, then: delete | anonymize)";

/// One day in milliseconds.
const DAY_MS: i64 = 86_400_000;

fn retain_error(span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error("E1310", message, span).with_help(format!("usage: {RETAIN_USAGE}"))
}

/// `@retain(days(n), since: field, then: delete | anonymize)` (`spec/platform.md` section 23):
/// every mistake is `E1310`, and the annotation has no meaning when there is one. Whether the field
/// exists and is a date, and whether `anonymize` has a `@pii` field to work on, are checked once
/// the fields of the model are known.
fn retain_args(args: &[Arg], span: Span, diags: &mut Vec<Diagnostic>) -> Option<RetainAnn> {
    let before = diags.len();
    let (mut after, mut since, mut then) = (None, None, None);
    for a in args {
        match a.name.as_ref().map(|n| n.name.as_str()) {
            None => take_once(
                &mut after,
                retain_days(a, diags),
                a.span,
                "the duration",
                diags,
            ),
            Some("since") => match &a.value {
                ArgValue::Ident(i) => {
                    take_once(
                        &mut since,
                        Some((i.name.clone(), i.span)),
                        a.span,
                        "`since:`",
                        diags,
                    );
                }
                ArgValue::Literal(_) | ArgValue::Call { .. } => {
                    diags.push(retain_error(a.span, "`since:` names a field of the model"));
                }
            },
            Some("then") => take_once(&mut then, retain_then(a, diags), a.span, "`then:`", diags),
            Some(other) => diags.push(retain_error(
                a.span,
                format!("`{other}:` is not an argument of `@retain`"),
            )),
        }
    }
    let missing: Vec<&str> = [
        (after.is_none(), "the duration `days(n)`"),
        (since.is_none(), "`since:`"),
        (then.is_none(), "`then:`"),
    ]
    .into_iter()
    .filter_map(|(m, what)| m.then_some(what))
    .collect();
    if diags.len() == before && !missing.is_empty() {
        diags.push(retain_error(
            span,
            format!("`@retain` needs {}", missing.join(", ")),
        ));
    }
    let ((after_ms, _), (since, since_span), (then, _)) = (after?, since?, then?);
    (diags.len() == before).then_some(RetainAnn {
        after_ms,
        since,
        since_span,
        then,
        span,
    })
}

/// Keeps the first value and reports a second one.
fn take_once<T>(
    slot: &mut Option<T>,
    value: Option<T>,
    span: Span,
    what: &str,
    diags: &mut Vec<Diagnostic>,
) {
    if slot.is_some() {
        diags.push(retain_error(
            span,
            format!("{what} is given more than once"),
        ));
    } else if value.is_some() {
        *slot = value;
    }
}

/// `days(30)`: a whole, positive number of days that fits in milliseconds.
fn retain_days(a: &Arg, diags: &mut Vec<Diagnostic>) -> Option<(i64, Span)> {
    let fail = |diags: &mut Vec<Diagnostic>, why: &str| {
        diags.push(retain_error(a.span, why.to_string()));
        None
    };
    let ArgValue::Call { name, args } = &a.value else {
        return fail(
            diags,
            "the first argument is a duration of whole days: `days(30)`",
        );
    };
    if name.name != "days" {
        return fail(
            diags,
            &format!(
                "`{}(..)` is not a whole number of days: write `days(n)`",
                name.name
            ),
        );
    }
    let n = match args.as_slice() {
        [one] if one.name.is_none() => literal_of(one).map(|l| &l.kind),
        _ => None,
    };
    match n {
        Some(LitKind::Int(n)) if *n >= 1 => match n.checked_mul(DAY_MS) {
            Some(ms) => Some((ms, a.span)),
            None => fail(diags, "the duration is too long"),
        },
        _ => fail(diags, "`days(n)` takes one whole number of days, 1 or more"),
    }
}

fn retain_then(a: &Arg, diags: &mut Vec<Diagnostic>) -> Option<(RetainThen, Span)> {
    let found = match &a.value {
        ArgValue::Ident(i) if i.name == "delete" => Some(RetainThen::Delete),
        ArgValue::Ident(i) if i.name == "anonymize" => Some(RetainThen::Anonymize),
        ArgValue::Ident(_) | ArgValue::Literal(_) | ArgValue::Call { .. } => None,
    };
    if found.is_none() {
        diags.push(retain_error(a.span, "`then:` is `delete` or `anonymize`"));
    }
    found.map(|t| (t, a.span))
}

/// `@idempotent`, `@idempotent(header: "Name")`, `@idempotent(required: true)` or both, in either
/// order (`spec/rules.md` section 11.8). The header must be an HTTP token that no part of Veld owns.
fn idempotent_args(args: &[Arg], span: Span) -> Result<Idempotent, String> {
    let mut out = Idempotent {
        header: None,
        required: false,
        span,
    };
    let (mut saw_header, mut saw_required) = (false, false);
    for a in args {
        let key = a.name.as_ref().map(|n| n.name.as_str());
        let kind = literal_of(a).map(|l| &l.kind);
        match (key, kind) {
            (Some("header"), Some(LitKind::Str(h))) => {
                once_only(&mut saw_header, "header")?;
                out.header = Some(idempotency_header(h)?);
            }
            (Some("required"), Some(LitKind::Bool(b))) => {
                once_only(&mut saw_required, "required")?;
                out.required = *b;
            }
            _ => {
                return Err(
                    "expected `header: \"Name\"` and/or `required: true` or `required: false`"
                        .to_string(),
                );
            }
        }
    }
    Ok(out)
}

fn once_only(seen: &mut bool, name: &str) -> Result<(), String> {
    if std::mem::replace(seen, true) {
        return Err(format!("`{name}:` is given more than once"));
    }
    Ok(())
}

/// Headers Veld or HTTP already uses: a key cannot take their place.
const RESERVED_HEADERS: [&str; 7] = [
    "authorization",
    "content-type",
    "content-length",
    "accept",
    "host",
    "if-match",
    "etag",
];

fn idempotency_header(h: &str) -> Result<String, String> {
    let token = |c: char| c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(c);
    if h.is_empty() || !h.chars().all(token) {
        return Err(format!("`{h}` is not a header name"));
    }
    let lower = h.to_ascii_lowercase();
    if RESERVED_HEADERS.contains(&lower.as_str()) || lower.starts_with("veld-") {
        return Err(format!("`{h}` is a header that Veld or HTTP already uses"));
    }
    Ok(h.to_string())
}

fn auth_arg(args: &[Arg]) -> Result<Auth, String> {
    let arg = one(args)?;
    match &arg.value {
        ArgValue::Ident(i) => match i.name.as_str() {
            "bearer" => Ok(Auth::new(AuthKind::Bearer, None)),
            "basic" => Ok(Auth::new(AuthKind::Basic, None)),
            "none" => Ok(Auth::none()),
            "apiKey" => Err("`apiKey` needs a header: `apiKey(header: \"X-API-Key\")`".to_string()),
            other => Err(format!("unknown auth scheme `{other}`")),
        },
        ArgValue::Literal(Literal {
            kind: LitKind::Null,
            ..
        }) => Ok(Auth::none()),
        ArgValue::Call { name, args } if name.name == "apiKey" => {
            let header = match args.as_slice() {
                [h] if h.name.as_ref().is_some_and(|n| n.name == "header") => {
                    match literal_of(h).map(|l| &l.kind) {
                        Some(LitKind::Str(s)) if !s.is_empty() => s.clone(),
                        _ => return Err("`header:` must be a non-empty string".to_string()),
                    }
                }
                _ => return Err("`apiKey` takes exactly `header: \"Name\"`".to_string()),
            };
            Ok(Auth::new(AuthKind::ApiKey, Some(header)))
        }
        ArgValue::Literal(_) | ArgValue::Call { .. } => {
            Err("expected bearer, basic, none or apiKey(header: \"X\")".to_string())
        }
    }
}

fn idents(args: &[Arg]) -> Result<Vec<String>, String> {
    if args.is_empty() {
        return Err("expected at least one name".to_string());
    }
    positional(args)?
        .into_iter()
        .map(|a| match &a.value {
            ArgValue::Ident(i) => Ok(i.name.clone()),
            ArgValue::Literal(_) | ArgValue::Call { .. } => Err("expected plain names".to_string()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::FileId;
    use crate::syntax::ast::DeclKind;
    use crate::syntax::parser::parse as parse_src;

    /// Annotations of the first field of a model parsed from `model M { <field> }`.
    fn run(field: &str, target: Target, ty: Option<Type>) -> (Vec<String>, Vec<&'static str>) {
        let src = format!("model M {{ {field} }}");
        let parsed = parse_src(&src, FileId(0));
        let DeclKind::Model(m) = &parsed.file.decls[0].kind else {
            panic!("model expected")
        };
        let mut diags = Vec::new();
        let a = parse(&m.fields[0].annotations, target, ty.as_ref(), &mut diags);
        let mut facts = Vec::new();
        if a.readonly {
            facts.push("readonly".to_string());
        }
        if let Some(w) = &a.wire {
            facts.push(format!("wire={w}"));
        }
        if let Some(n) = a.constraints.min_len {
            facts.push(format!("min_len={n}"));
        }
        if let Some(n) = a.constraints.max {
            facts.push(format!("max={n}"));
        }
        (facts, diags.iter().map(|d| d.code).collect())
    }

    const S: Option<Type> = Some(Type::Scalar(Scalar::String));
    const I: Option<Type> = Some(Type::Scalar(Scalar::Int));

    #[test]
    fn valid_annotations_are_applied() {
        let (f, d) = run(
            "a: string @readonly @wire(\"b\") @length(min: 2)",
            Target::Field,
            S,
        );
        assert!(d.is_empty());
        assert_eq!(f, vec!["readonly", "wire=b", "min_len=2"]);
        assert_eq!(run("a: int @max(5)", Target::Field, I).0, vec!["max=5"]);
    }

    #[test]
    fn unknown_annotation_is_e0601() {
        assert_eq!(run("a: int @nope", Target::Field, I).1, vec!["E0601"]);
    }

    #[test]
    fn wrong_target_wrong_args_and_duplicates_are_e0602() {
        assert_eq!(
            run("a: int @auth(bearer)", Target::Field, I).1,
            vec!["E0602"]
        );
        assert_eq!(run("a: int @wire", Target::Field, I).1, vec!["E0602"]);
        assert_eq!(
            run("a: int @readonly(1)", Target::Field, I).1,
            vec!["E0602"]
        );
        assert_eq!(
            run("a: int @readonly @readonly", Target::Field, I).1,
            vec!["E0602"]
        );
        assert_eq!(run("a: string @min(1)", Target::Field, S).1, vec!["E0602"]);
        assert_eq!(
            run("a: int @length(min: 1)", Target::Field, I).1,
            vec!["E0602"]
        );
        assert_eq!(
            run("a: int @length(min: 3, max: 1)", Target::Field, I).1,
            vec!["E0602"]
        );
        assert_eq!(
            run("a: int @min(5) @max(1)", Target::Field, I).1,
            vec!["E0602"]
        );
        assert_eq!(
            run("a: string @format(nope)", Target::Field, S).1,
            vec!["E0602"]
        );
    }

    #[test]
    fn pattern_outside_subset_is_e0603() {
        assert_eq!(
            run("a: string @pattern(\"(?=x)\")", Target::Field, S).1,
            vec!["E0603"]
        );
        assert!(
            run("a: string @pattern(\"^[a-z]+$\")", Target::Field, S)
                .1
                .is_empty()
        );
    }
}
