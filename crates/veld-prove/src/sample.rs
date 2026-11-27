//! Plain values of a type, and a check of the constraints (`@min`, `@max`, `@length`,
//! `@pattern`) that the evaluator of rules does not look at.
//!
//! The solver models only the parts of a value that a rule reads. The rest of a value is filled
//! in here with the simplest value that respects the constraints.

use serde_json::{Map, Value as Json, json};
use veld_ir::{Constraints, Decl, DeclKind, Ir, Scalar, StringFormat, Type};
use veld_rules::helpers::regex::Regex;
use veld_rules::helpers::text;

const MAX_DEPTH: usize = 6;
/// The largest list or map the confirmation builds; a longer one is not confirmed.
pub const MAX_ITEMS: usize = 256;

/// Base64 of `n` zero bytes.
#[must_use]
pub fn zero_bytes(n: usize) -> String {
    let mut out = "AAAA".repeat(n / 3);
    match n % 3 {
        1 => out.push_str("AA=="),
        2 => out.push_str("AAA="),
        _ => {}
    }
    out
}

/// The simplest value of `ty` that satisfies `c`.
///
/// # Errors
/// A text that says why no such value could be made.
pub fn sample(ir: &Ir, ty: &Type, c: &Constraints, depth: usize) -> Result<Json, String> {
    if depth > MAX_DEPTH {
        return Err("a type nested too deeply".to_owned());
    }
    match ty {
        Type::Scalar(s) => sample_scalar(*s, c),
        Type::List(t) => {
            let n = usize::try_from(c.min_len.unwrap_or(0)).unwrap_or(usize::MAX);
            if n > MAX_ITEMS {
                return Err("a list that must be longer than the check builds".to_owned());
            }
            let item = sample(ir, t, &Constraints::default(), depth + 1)?;
            Ok(Json::Array(vec![item; n]))
        }
        Type::Map(t) => {
            let n = usize::try_from(c.min_len.unwrap_or(0)).unwrap_or(usize::MAX);
            if n > MAX_ITEMS {
                return Err("a map that must be longer than the check builds".to_owned());
            }
            let item = sample(ir, t, &Constraints::default(), depth + 1)?;
            Ok(Json::Object(
                (0..n).map(|i| (format!("k{i}"), item.clone())).collect(),
            ))
        }
        Type::Page(_) => Ok(json!({ "items": [], "total": "0", "page": 1, "size": 0 })),
        Type::Decl(id) => sample_decl(ir, ir.decl(*id), depth),
    }
}

/// The simplest value of a declaration: its required fields only.
///
/// # Errors
/// A text that says why no such value could be made.
pub fn sample_decl(ir: &Ir, decl: &Decl, depth: usize) -> Result<Json, String> {
    if depth > MAX_DEPTH {
        return Err("a type nested too deeply".to_owned());
    }
    match &decl.kind {
        DeclKind::Enum(e) => e
            .variants
            .first()
            .map(|v| Json::String(v.wire.clone()))
            .ok_or_else(|| "an enum with no variant".to_owned()),
        DeclKind::Model(_) | DeclKind::Error(_) | DeclKind::Event(_) => {
            let mut obj = Map::new();
            for f in decl.fields() {
                if f.optional && f.default.is_none() {
                    continue;
                }
                obj.insert(
                    f.wire.clone(),
                    sample(ir, &f.ty, &f.constraints, depth + 1)?,
                );
            }
            Ok(Json::Object(obj))
        }
        DeclKind::Union(u) => {
            let arm = u
                .arms
                .first()
                .ok_or_else(|| "a union with no arm".to_owned())?;
            let mut v = sample_decl(ir, ir.decl(arm.decl), depth + 1)?;
            if let Some(obj) = v.as_object_mut() {
                obj.insert(u.discriminator.clone(), Json::String(arm.label.clone()));
            }
            Ok(v)
        }
        DeclKind::Const(_) => Err("a constant used as a type".to_owned()),
    }
}

fn clamp_zero(c: &Constraints) -> i64 {
    let lo = c.min.map(f64::ceil);
    let hi = c.max.map(f64::floor);
    let v = match (lo, hi) {
        (Some(l), _) if l > 0.0 => l,
        (_, Some(h)) if h < 0.0 => h,
        _ => 0.0,
    };
    v as i64
}

fn sample_scalar(s: Scalar, c: &Constraints) -> Result<Json, String> {
    Ok(match s {
        Scalar::Int | Scalar::Float => json!(clamp_zero(c)),
        Scalar::Long | Scalar::Decimal => Json::String(clamp_zero(c).to_string()),
        Scalar::Bool => Json::Bool(false),
        Scalar::Date => json!("1970-01-01"),
        Scalar::Datetime => json!("1970-01-01T00:00:00Z"),
        Scalar::Time => json!("00:00:00"),
        Scalar::Uuid => json!("00000000-0000-4000-8000-000000000000"),
        Scalar::Json => json!({}),
        Scalar::Bytes => {
            let n = usize::try_from(c.min_len.unwrap_or(0)).unwrap_or(usize::MAX);
            if n > MAX_ITEMS {
                return Err("bytes that must be longer than the check builds".to_owned());
            }
            Json::String(zero_bytes(n))
        }
        Scalar::String => string_sample(c)?,
    })
}

fn format_sample(f: StringFormat) -> &'static str {
    match f {
        StringFormat::Email => "a@b.co",
        StringFormat::Uri => "http://a.b",
        StringFormat::Hostname => "a.b",
        StringFormat::Ipv4 => "1.2.3.4",
        StringFormat::Ipv6 => "::1",
    }
}

fn string_sample(c: &Constraints) -> Result<Json, String> {
    let n = usize::try_from(c.min_len.unwrap_or(0)).unwrap_or(usize::MAX);
    if n > MAX_ITEMS {
        return Err("a string that must be longer than the check builds".to_owned());
    }
    let mut candidates: Vec<String> = Vec::new();
    if let Some(f) = c.format {
        candidates.push(format_sample(f).to_owned());
    }
    candidates.push("a".repeat(n));
    candidates.push("0".repeat(n.max(1)));
    for cand in candidates {
        let v = Json::String(cand);
        if check_scalar(&Type::Scalar(Scalar::String), c, &v).is_ok() {
            return Ok(v);
        }
    }
    Err("a string that satisfies its pattern or format".to_owned())
}

/// The number a wire value stands for, when it is one.
fn number_of(v: &Json) -> Option<f64> {
    match v {
        Json::Number(n) => n.as_f64(),
        Json::String(s) => s.parse().ok(),
        Json::Null | Json::Bool(_) | Json::Array(_) | Json::Object(_) => None,
    }
}

/// The length that `@length` counts: code points, items, entries or bytes.
fn length_of(ty: &Type, v: &Json) -> Option<u64> {
    let n = match v {
        Json::String(s) if matches!(ty, Type::Scalar(Scalar::Bytes)) => text::base64_byte_len(s)?,
        Json::String(s) => text::len_code_points(s),
        Json::Array(a) => a.len(),
        Json::Object(o) => o.len(),
        Json::Null | Json::Bool(_) | Json::Number(_) => return None,
    };
    u64::try_from(n).ok()
}

/// Checks `@min`, `@max`, `@length` and `@pattern` of one value. A `format` cannot be checked
/// here and is accepted.
///
/// # Errors
/// A text that names the constraint the value breaks.
pub fn check_scalar(ty: &Type, c: &Constraints, v: &Json) -> Result<(), String> {
    if let (Some(min), Some(x)) = (c.min, number_of(v))
        && x < min
    {
        return Err(format!("{x} is below @min {min}"));
    }
    if let (Some(max), Some(x)) = (c.max, number_of(v))
        && x > max
    {
        return Err(format!("{x} is above @max {max}"));
    }
    if c.min_len.is_some() || c.max_len.is_some() {
        let n = length_of(ty, v).ok_or_else(|| "a length of a value without one".to_owned())?;
        if c.min_len.is_some_and(|m| n < m) || c.max_len.is_some_and(|m| n > m) {
            return Err(format!("the length {n} breaks @length"));
        }
    }
    if let (Some(p), Json::String(s)) = (&c.pattern, v) {
        let re = Regex::new(p).ok_or_else(|| format!("the pattern `{p}` is not supported"))?;
        if !re.is_match(s) {
            return Err(format!("`{s}` does not match the pattern `{p}`"));
        }
    }
    Ok(())
}

/// Checks the constraints of every field of a model value and of the models nested in it. The
/// items of lists and maps are not looked at.
///
/// # Errors
/// A text that names the field and the constraint it breaks.
pub fn check_decl(ir: &Ir, decl: &Decl, v: &Json, depth: usize) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Ok(());
    }
    for f in decl.fields() {
        let Some(child) = v.get(&f.wire) else {
            continue;
        };
        check_scalar(&f.ty, &f.constraints, child).map_err(|e| format!("`{}`: {e}", f.name))?;
        if let Type::Decl(id) = &f.ty {
            let inner = ir.decl(*id);
            if matches!(inner.kind, DeclKind::Model(_) | DeclKind::Error(_)) {
                check_decl(ir, inner, child, depth + 1)?;
            }
        }
    }
    Ok(())
}
