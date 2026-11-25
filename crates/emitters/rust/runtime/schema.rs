//! Schemas: validation, defaults, the wire mapping and canonical bytes.
//!
//! Every generated declaration has a static [`Schema`]. Two JSON forms exist:
//! the *wire* form (what travels: `long` is a decimal string, `bytes` is base64) and the *typed*
//! form (what `serde` derives for the generated Rust types: `long` is a number, `bytes` an array).
//! Wire names are the same in both, so the mapping only converts those two scalars, applies
//! defaults and, for canonical bytes, normalises datetimes and uuids.

use std::fmt;

use serde_json::{Map, Value};

use super::canonical::canonical_bytes;
use super::error::Issue;
use super::regex::Regex;
use super::util::{base64_decode, base64_encode, is_base64};

/// The scalar types of the contract language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scalar {
    String,
    Int,
    Long,
    Float,
    Decimal,
    Bool,
    Date,
    Datetime,
    Time,
    Uuid,
    Bytes,
    Json,
}

impl Scalar {
    fn name(self) -> &'static str {
        match self {
            Scalar::String => "string",
            Scalar::Int => "int",
            Scalar::Long => "long",
            Scalar::Float => "float",
            Scalar::Decimal => "decimal",
            Scalar::Bool => "bool",
            Scalar::Date => "date",
            Scalar::Datetime => "datetime",
            Scalar::Time => "time",
            Scalar::Uuid => "uuid",
            Scalar::Bytes => "bytes",
            Scalar::Json => "json",
        }
    }
}

/// String formats (`@format`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Email,
    Uri,
    Hostname,
    Ipv4,
    Ipv6,
}

impl Format {
    fn name(self) -> &'static str {
        match self {
            Format::Email => "email",
            Format::Uri => "uri",
            Format::Hostname => "hostname",
            Format::Ipv4 => "ipv4",
            Format::Ipv6 => "ipv6",
        }
    }
}

/// Constraints of a field or parameter: `@min`, `@max`, `@length`, `@pattern`, `@format`.
#[derive(Debug, Clone, Copy)]
pub struct Rules {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub min_len: Option<u64>,
    pub max_len: Option<u64>,
    pub pattern: Option<&'static str>,
    pub format: Option<Format>,
}

impl Rules {
    /// No constraints.
    pub const NONE: Rules = Rules {
        min: None,
        max: None,
        min_len: None,
        max_len: None,
        pattern: None,
        format: None,
    };
}

/// One field of a model or error.
#[derive(Debug, Clone, Copy)]
pub struct Field {
    pub wire: &'static str,
    pub schema: &'static Schema,
    pub optional: bool,
    /// The default as JSON text, applied when the field is absent.
    pub default: Option<&'static str>,
    pub readonly: bool,
}

/// A model or an error.
#[derive(Debug, Clone, Copy)]
pub struct ModelSchema {
    pub name: &'static str,
    pub sig: &'static str,
    pub fields: &'static [Field],
    // #if rules
    /// Evaluates the invariants of one object of this declaration (typed JSON form), if it has any.
    pub invariants: Option<fn(&Value, super::rules::Mode, &str, &mut Vec<super::rules::Violation>)>,
    // #endif
}

/// A discriminated union.
#[derive(Debug, Clone, Copy)]
pub struct UnionSchema {
    pub name: &'static str,
    pub sig: &'static str,
    pub discriminator: &'static str,
    pub arms: &'static [(&'static str, &'static Schema)],
}

/// The shape of a value.
#[derive(Clone, Copy)]
pub enum Schema {
    Scalar(Scalar, Rules),
    Enum(&'static [&'static str]),
    List(&'static Schema, Rules),
    Map(&'static Schema, Rules),
    Page(&'static Schema),
    Model(&'static ModelSchema),
    Union(&'static UnionSchema),
}

impl fmt::Debug for Schema {
    // Schemas are cyclic graphs: print the name, never the structure.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Schema::Scalar(k, _) => write!(f, "Schema::{}", k.name()),
            Schema::Enum(_) => write!(f, "Schema::Enum"),
            Schema::List(..) => write!(f, "Schema::List"),
            Schema::Map(..) => write!(f, "Schema::Map"),
            Schema::Page(_) => write!(f, "Schema::Page"),
            Schema::Model(m) => write!(f, "Schema::Model({})", m.name),
            Schema::Union(u) => write!(f, "Schema::Union({})", u.name),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dir {
    /// Wire form to typed form.
    From,
    /// Typed form to wire form.
    To,
}

#[derive(Clone, Copy)]
struct Opts {
    dir: Dir,
    /// Request mode: read-only fields are left out.
    request: bool,
    /// Canonical mode: datetimes are UTC and uuids lower case.
    normalize: bool,
}

const LONG: Schema = Schema::Scalar(Scalar::Long, Rules::NONE);
const INT: Schema = Schema::Scalar(Scalar::Int, Rules::NONE);

impl Schema {
    /// The model behind this schema, when it is one.
    #[must_use]
    pub fn model(&self) -> Option<&'static ModelSchema> {
        match self {
            Schema::Model(m) => Some(m),
            Schema::Scalar(..)
            | Schema::Enum(_)
            | Schema::List(..)
            | Schema::Map(..)
            | Schema::Page(_)
            | Schema::Union(_) => None,
        }
    }

    /// Whether this is a list of something.
    #[must_use]
    pub fn list_item(&self) -> Option<&'static Schema> {
        match self {
            Schema::List(of, _) => Some(of),
            Schema::Scalar(..)
            | Schema::Enum(_)
            | Schema::Map(..)
            | Schema::Page(_)
            | Schema::Model(_)
            | Schema::Union(_) => None,
        }
    }

    /// Wire form to typed form: applies defaults, leaves read-only fields out in request mode,
    /// reads `long` and `bytes`. The value must be valid ([`Schema::validate`]).
    #[must_use]
    pub fn from_wire(&self, wire: &Value, request: bool) -> Value {
        self.map(wire, Opts { dir: Dir::From, request, normalize: false })
    }

    /// Typed form to wire form: applies defaults, writes `long` as a string and `bytes` as base64.
    /// `request` leaves read-only fields out; `normalize` is the canonical form.
    #[must_use]
    pub fn to_wire(&self, typed: &Value, request: bool, normalize: bool) -> Value {
        self.map(typed, Opts { dir: Dir::To, request, normalize })
    }

    /// RFC 8785 bytes of a typed value: defaults applied, UTC datetimes, lower case uuids.
    #[must_use]
    pub fn canonical_typed(&self, typed: &Value) -> Vec<u8> {
        canonical_bytes(&self.to_wire(typed, false, true))
    }

    /// RFC 8785 bytes of a wire value (the same bytes as for the decoded typed value).
    #[must_use]
    pub fn canonical_wire(&self, wire: &Value) -> Vec<u8> {
        self.canonical_typed(&self.from_wire(wire, false))
    }

    fn map(&self, v: &Value, o: Opts) -> Value {
        match self {
            Schema::Scalar(k, _) => map_scalar(*k, v, o),
            Schema::Enum(_) => v.clone(),
            Schema::List(of, _) => match v.as_array() {
                Some(a) => Value::Array(a.iter().map(|x| of.map(x, o)).collect()),
                None => v.clone(),
            },
            Schema::Map(of, _) => match v.as_object() {
                Some(m) => Value::Object(
                    m.iter().filter(|(_, x)| !x.is_null()).map(|(k, x)| (k.clone(), of.map(x, o))).collect(),
                ),
                None => v.clone(),
            },
            Schema::Page(of) => map_page(of, v, o),
            Schema::Model(m) => map_model(m, v, o),
            Schema::Union(u) => map_union(u, v, o),
        }
    }

    /// The issues of a wire value (empty when it is valid). `path` names the root (`body`,
    /// `query.limit`, `$`); `request` lets read-only fields be absent.
    #[must_use]
    pub fn validate(&self, wire: &Value, path: &str, request: bool) -> Vec<Issue> {
        let mut out = Vec::new();
        check(self, wire, path, request, &mut out);
        out
    }
}

fn map_scalar(k: Scalar, v: &Value, o: Opts) -> Value {
    match (k, o.dir, v) {
        (Scalar::Long, Dir::From, Value::String(s)) => s.parse::<i64>().map_or_else(|_| v.clone(), Value::from),
        (Scalar::Long, Dir::To, Value::Number(n)) => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => Value::String(i.to_string()),
            (None, Some(u)) => Value::String(u.to_string()),
            (None, None) => v.clone(),
        },
        (Scalar::Int, Dir::From, Value::Number(n)) if !n.is_i64() => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 9.0e15 => Value::from(f as i64),
            Some(_) | None => v.clone(),
        },
        (Scalar::Bytes, Dir::From, Value::String(s)) => match base64_decode(s) {
            Some(b) => Value::Array(b.into_iter().map(Value::from).collect()),
            None => v.clone(),
        },
        (Scalar::Bytes, Dir::To, Value::Array(a)) => {
            let bytes: Option<Vec<u8>> = a.iter().map(|x| x.as_u64().and_then(|b| u8::try_from(b).ok())).collect();
            bytes.map_or_else(|| v.clone(), |b| Value::String(base64_encode(&b)))
        }
        (Scalar::Uuid, Dir::To, Value::String(s)) if o.normalize => Value::String(s.to_lowercase()),
        (Scalar::Datetime, Dir::To, Value::String(s)) if o.normalize => {
            Value::String(normalize_datetime(s).unwrap_or_else(|| s.clone()))
        }
        _ => v.clone(),
    }
}

fn map_page(of: &'static Schema, v: &Value, o: Opts) -> Value {
    let Some(obj) = v.as_object() else {
        return v.clone();
    };
    let get = |k: &str| obj.get(k).cloned().unwrap_or(Value::Null);
    let mut out = Map::new();
    out.insert("items".into(), Schema::List(of, Rules::NONE).map(&get("items"), o));
    out.insert("total".into(), LONG.map(&get("total"), o));
    out.insert("page".into(), INT.map(&get("page"), o));
    out.insert("size".into(), INT.map(&get("size"), o));
    Value::Object(out)
}

fn map_model(m: &ModelSchema, v: &Value, o: Opts) -> Value {
    let Some(obj) = v.as_object() else {
        return v.clone();
    };
    let mut out = Map::new();
    for f in m.fields {
        if o.request && f.readonly {
            continue;
        }
        let mut x = obj.get(f.wire).filter(|x| !x.is_null()).cloned();
        if x.is_none() {
            x = f.default.and_then(|d| serde_json::from_str(d).ok());
        }
        if let Some(x) = x {
            out.insert(f.wire.to_string(), f.schema.map(&x, o));
        }
    }
    Value::Object(out)
}

fn map_union(u: &UnionSchema, v: &Value, o: Opts) -> Value {
    let label = v.get(u.discriminator).and_then(Value::as_str);
    let Some((label, arm)) = label.and_then(|l| u.arms.iter().find(|(name, _)| *name == l)) else {
        return v.clone();
    };
    match arm.map(v, o) {
        Value::Object(mut obj) => {
            obj.insert(u.discriminator.to_string(), Value::String((*label).to_string()));
            Value::Object(obj)
        }
        other => other,
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn push(out: &mut Vec<Issue>, path: &str, code: &str, message: String) {
    out.push(Issue { path: path.to_string(), code: code.to_string(), message });
}

fn num(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1.0e15 {
        format!("{}", f as i64)
    } else {
        format!("{f}")
    }
}

fn check(s: &Schema, v: &Value, path: &str, request: bool, out: &mut Vec<Issue>) {
    match s {
        Schema::Enum(values) => {
            if !v.as_str().is_some_and(|x| values.contains(&x)) {
                push(out, path, "enum", format!("must be one of {}", values.join(", ")));
            }
        }
        Schema::List(of, rules) => match v.as_array() {
            None => push(out, path, "type", "must be an array".into()),
            Some(a) => {
                check_len(rules, a.len(), path, out);
                for (i, x) in a.iter().enumerate() {
                    check(of, x, &format!("{path}[{i}]"), request, out);
                }
            }
        },
        Schema::Map(of, rules) => match v.as_object() {
            None => push(out, path, "type", "must be an object".into()),
            Some(m) => {
                check_len(rules, m.len(), path, out);
                for (k, x) in m {
                    check(of, x, &format!("{path}.{k}"), request, out);
                }
            }
        },
        Schema::Page(of) => check_page(of, v, path, request, out),
        Schema::Model(m) => check_model(m, v, path, request, out),
        Schema::Union(u) => check_union(u, v, path, request, out),
        Schema::Scalar(k, rules) => {
            if scalar_ok(*k, v) {
                check_bounds(rules, v, path, out);
            } else {
                push(out, path, "type", format!("must be a valid {}", k.name()));
            }
        }
    }
}

fn check_page(of: &'static Schema, v: &Value, path: &str, request: bool, out: &mut Vec<Issue>) {
    let Some(obj) = v.as_object() else {
        return push(out, path, "type", "must be an object".into());
    };
    let get = |k: &str| obj.get(k).unwrap_or(&Value::Null).clone();
    check(&Schema::List(of, Rules::NONE), &get("items"), &format!("{path}.items"), request, out);
    check(&LONG, &get("total"), &format!("{path}.total"), request, out);
    check(&INT, &get("page"), &format!("{path}.page"), request, out);
    check(&INT, &get("size"), &format!("{path}.size"), request, out);
}

fn check_model(m: &ModelSchema, v: &Value, path: &str, request: bool, out: &mut Vec<Issue>) {
    let Some(obj) = v.as_object() else {
        return push(out, path, "type", "must be an object".into());
    };
    for f in m.fields {
        let at = format!("{path}.{}", f.wire);
        match obj.get(f.wire) {
            None => {
                if !f.optional && f.default.is_none() && !(request && f.readonly) {
                    push(out, &at, "required", "is required".into());
                }
            }
            Some(x) => check(f.schema, x, &at, request, out),
        }
    }
}

fn check_union(u: &UnionSchema, v: &Value, path: &str, request: bool, out: &mut Vec<Issue>) {
    let label = v.get(u.discriminator).and_then(Value::as_str);
    match label.and_then(|l| u.arms.iter().find(|(name, _)| *name == l)) {
        Some((_, arm)) => check(arm, v, path, request, out),
        None => {
            let names: Vec<&str> = u.arms.iter().map(|(n, _)| *n).collect();
            push(
                out,
                &format!("{path}.{}", u.discriminator),
                "discriminator",
                format!("must be one of {}", names.join(", ")),
            );
        }
    }
}

fn check_len(rules: &Rules, len: usize, path: &str, out: &mut Vec<Issue>) {
    let len = len as u64;
    if let Some(min) = rules.min_len {
        if len < min {
            push(out, path, "min_length", format!("length must be at least {min}"));
        }
    }
    if let Some(max) = rules.max_len {
        if len > max {
            push(out, path, "max_length", format!("length must be at most {max}"));
        }
    }
}

fn check_bounds(rules: &Rules, v: &Value, path: &str, out: &mut Vec<Issue>) {
    match v {
        Value::Number(n) => {
            let x = n.as_f64().unwrap_or(0.0);
            if let Some(min) = rules.min {
                if x < min {
                    push(out, path, "min", format!("must be at least {}", num(min)));
                }
            }
            if let Some(max) = rules.max {
                if x > max {
                    push(out, path, "max", format!("must be at most {}", num(max)));
                }
            }
        }
        Value::String(s) => {
            check_len(rules, s.chars().count(), path, out);
            if let Some(p) = rules.pattern {
                if !Regex::new(p).is_some_and(|r| r.is_match(s)) {
                    push(out, path, "pattern", format!("must match {p}"));
                }
            }
            if let Some(f) = rules.format {
                if !format_ok(f, s) {
                    push(out, path, "format", format!("must be a valid {}", f.name()));
                }
            }
        }
        Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => {}
    }
}

fn scalar_ok(k: Scalar, v: &Value) -> bool {
    match k {
        Scalar::String => v.is_string(),
        Scalar::Int => as_int(v),
        Scalar::Long => v.as_str().is_some_and(is_long),
        Scalar::Float => v.as_f64().is_some_and(f64::is_finite) && v.is_number(),
        Scalar::Decimal => v.as_str().is_some_and(is_decimal),
        Scalar::Bool => v.is_boolean(),
        Scalar::Date => v.as_str().is_some_and(valid_date),
        Scalar::Datetime => v.as_str().is_some_and(|s| parse_datetime(s).is_some()),
        Scalar::Time => v.as_str().is_some_and(valid_time),
        Scalar::Uuid => v.as_str().is_some_and(is_uuid),
        Scalar::Bytes => v.as_str().is_some_and(is_base64),
        Scalar::Json => true,
    }
}

fn as_int(v: &Value) -> bool {
    let Value::Number(n) = v else {
        return false;
    };
    match n.as_i64() {
        Some(i) => i32::try_from(i).is_ok(),
        None => n.as_f64().is_some_and(|f| f.fract() == 0.0 && (-2_147_483_648.0..=2_147_483_647.0).contains(&f)),
    }
}

fn is_long(s: &str) -> bool {
    let digits = s.strip_prefix('-').unwrap_or(s);
    !digits.is_empty() && digits.len() <= 19 && digits.bytes().all(|b| b.is_ascii_digit()) && s.parse::<i64>().is_ok()
}

fn is_decimal(s: &str) -> bool {
    let body = s.strip_prefix('-').unwrap_or(s);
    let (int, frac) = match body.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (body, None),
    };
    let digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    digits(int) && frac.map_or(true, digits)
}

fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    let lens = [8, 4, 4, 4, 12];
    parts.len() == 5
        && parts.iter().zip(lens).all(|(p, n)| p.len() == n && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn digits(s: &str, n: usize) -> Option<u32> {
    (s.len() == n && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok()).flatten()
}

fn leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if leap(y) => 29,
        _ => 28,
    }
}

fn valid_date(s: &str) -> bool {
    let p: Vec<&str> = s.split('-').collect();
    if p.len() != 3 {
        return false;
    }
    match (digits(p[0], 4), digits(p[1], 2), digits(p[2], 2)) {
        (Some(y), Some(m), Some(d)) => (1..=12).contains(&m) && d >= 1 && d <= days_in_month(i64::from(y), m),
        _ => false,
    }
}

fn valid_time(s: &str) -> bool {
    let (main, frac) = match s.split_once('.') {
        Some((m, f)) => (m, Some(f)),
        None => (s, None),
    };
    let p: Vec<&str> = main.split(':').collect();
    if p.len() != 3 || frac.is_some_and(|f| f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit())) {
        return false;
    }
    matches!((digits(p[0], 2), digits(p[1], 2), digits(p[2], 2)), (Some(h), Some(m), Some(sec)) if h < 24 && m < 60 && sec <= 60)
}

/// A parsed RFC 3339 datetime: fields as written, and the offset in minutes.
struct DateTime<'a> {
    y: i64,
    mo: u32,
    d: u32,
    h: u32,
    mi: u32,
    s: u32,
    frac: &'a str,
    zulu: bool,
    offset_min: i64,
}

fn parse_datetime(s: &str) -> Option<DateTime<'_>> {
    let (date, rest) = s.split_once('T')?;
    let p: Vec<&str> = date.split('-').collect();
    if p.len() != 3 || !valid_date(date) {
        return None;
    }
    let (time, zone) = if let Some(t) = rest.strip_suffix('Z') {
        (t, None)
    } else {
        let at = rest.rfind(['+', '-'])?;
        (&rest[..at], Some(&rest[at..]))
    };
    let (main, frac) = match time.split_once('.') {
        Some((m, f)) if !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()) => (m, f),
        Some(_) => return None,
        None => (time, ""),
    };
    let t: Vec<&str> = main.split(':').collect();
    if t.len() != 3 {
        return None;
    }
    let (h, mi, sec) = (digits(t[0], 2)?, digits(t[1], 2)?, digits(t[2], 2)?);
    if h > 23 || mi > 59 || sec > 59 {
        return None;
    }
    let offset_min = match zone {
        None => 0,
        Some(z) => {
            let sign = if z.starts_with('-') { -1 } else { 1 };
            let (oh, om) = z[1..].split_once(':')?;
            let (oh, om) = (digits(oh, 2)?, digits(om, 2)?);
            if oh > 23 || om > 59 {
                return None;
            }
            sign * i64::from(oh * 60 + om)
        }
    };
    Some(DateTime {
        y: i64::from(digits(p[0], 4)?),
        mo: digits(p[1], 2)?,
        d: digits(p[2], 2)?,
        h,
        mi,
        s: sec,
        frac,
        zulu: zone.is_none(),
        offset_min,
    })
}

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = i64::from((m + 9) % 12);
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let m = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// The canonical form of a datetime: UTC, `Z`, no trailing zeros in the fraction.
fn normalize_datetime(s: &str) -> Option<String> {
    let dt = parse_datetime(s)?;
    let local = days_from_civil(dt.y, dt.mo, dt.d) * 86_400
        + i64::from(dt.h) * 3600
        + i64::from(dt.mi) * 60
        + i64::from(dt.s);
    let utc = local - dt.offset_min * 60;
    let (y, mo, d) = civil_from_days(utc.div_euclid(86_400));
    let rem = utc.rem_euclid(86_400);
    let frac: String = if dt.zulu { dt.frac.to_string() } else { dt.frac.chars().take(3).collect() };
    let frac = frac.trim_end_matches('0');
    let frac = if frac.is_empty() { String::new() } else { format!(".{frac}") };
    Some(format!("{y:04}-{mo:02}-{d:02}T{:02}:{:02}:{:02}{frac}Z", rem / 3600, rem % 3600 / 60, rem % 60))
}

fn format_ok(f: Format, s: &str) -> bool {
    match f {
        Format::Email => is_email(s),
        Format::Uri => is_uri(s),
        Format::Hostname => is_hostname(s),
        Format::Ipv4 => s.parse::<std::net::Ipv4Addr>().is_ok() && is_dotted_quad(s),
        Format::Ipv6 => s.parse::<std::net::Ipv6Addr>().is_ok(),
    }
}

fn is_dotted_quad(s: &str) -> bool {
    let p: Vec<&str> = s.split('.').collect();
    p.len() == 4 && p.iter().all(|x| !x.is_empty() && x.len() <= 3 && (*x == "0" || !x.starts_with('0')))
}

fn is_email(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    let bad = |c: char| c.is_whitespace() || c == '@';
    let labels: Vec<&str> = domain.split('.').collect();
    !local.is_empty() && !local.contains(bad) && labels.len() >= 2 && labels.iter().all(|l| !l.is_empty() && !l.contains(bad))
}

fn is_uri(s: &str) -> bool {
    let Some((scheme, rest)) = s.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
        && !rest.contains(char::is_whitespace)
}

fn is_hostname(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && s.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                && !l.starts_with('-')
                && !l.ends_with('-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datetimes_normalise_to_utc() {
        assert_eq!(normalize_datetime("2024-05-01T12:00:00+02:00").as_deref(), Some("2024-05-01T10:00:00Z"));
        assert_eq!(normalize_datetime("2024-05-01T10:00:00.500Z").as_deref(), Some("2024-05-01T10:00:00.5Z"));
        assert_eq!(normalize_datetime("2024-12-31T23:30:00-01:00").as_deref(), Some("2025-01-01T00:30:00Z"));
        assert!(normalize_datetime("2024-13-01T00:00:00Z").is_none());
    }

    #[test]
    fn scalars_validate() {
        assert!(is_long("-9223372036854775808"));
        assert!(!is_long("9223372036854775808"));
        assert!(is_decimal("1.50") && !is_decimal("1."));
        assert!(valid_date("2024-02-29") && !valid_date("2023-02-29"));
        assert!(is_email("a@b.co") && !is_email("a@b"));
        assert!(format_ok(Format::Ipv4, "10.0.0.1") && !format_ok(Format::Ipv4, "10.0.0.01"));
    }
}
