//! Invariants (`spec/rules.md`): violations, the helpers that generated rule code calls, the
//! schema walk that visits every value with rules, and the events a server reports.
//!
//! Generated code evaluates a rule as plain Rust. The value semantics live in the four helper
//! modules below (`decimal`, `instant`, `num`, `text`), the same files that the reference
//! evaluator of `veld-rules` runs. Everything here is the thin layer between them and the
//! generated types.

pub mod decimal;
pub mod instant;
pub mod num;
pub mod text;

use std::time::Instant;

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use super::regex::Regex;
use super::schema::{Field, ModelSchema, Scalar, Schema};

pub use decimal::Decimal;
pub use num::EvalError;

/// Why a rule could not be evaluated. The rule then counts as violated (it fails closed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fail {
    /// Overflow, division by zero or a non-finite float (`spec/rules.md` section 3.7).
    Eval(EvalError),
    /// A value the schema should have rejected: text that is not a decimal or an instant, or an
    /// optional value that the compiler proved present and is not. It never happens after
    /// validation; it is reported as `invalid_value`.
    Input,
}

impl From<EvalError> for Fail {
    fn from(e: EvalError) -> Self {
        Fail::Eval(e)
    }
}

impl Fail {
    /// The `error` member of the violation.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Fail::Eval(e) => e.as_str(),
            Fail::Input => "invalid_value",
        }
    }
}

/// The result of evaluating part of a rule.
pub type Res<T> = Result<T, Fail>;

/// How a value is checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Every rule.
    Full,
    /// A request: rules that read a `@readonly` field are skipped.
    Request,
}

/// One violated rule on one value (`spec/rules.md` section 7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The rule id, `veld:<package>/<Decl>.<rule>@<hex16>`.
    pub rule: String,
    /// The path of the value that carries the rule: `$`, `body`, `body.items[2]`.
    pub path: String,
    /// The wire names of the fields the rule reads.
    pub fields: Vec<String>,
    pub message: String,
    /// `overflow`, `division_by_zero`, `non_finite` (or `invalid_value`), for an evaluation error.
    pub error: Option<String>,
}

impl Violation {
    /// The JSON object of the wire, members in the documented order.
    #[must_use]
    pub fn to_text(&self) -> String {
        let text = |s: &str| serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
        let fields: Vec<String> = self.fields.iter().map(|f| text(f)).collect();
        let mut out = format!(
            "{{\"rule\":{},\"path\":{},\"fields\":[{}],\"message\":{}",
            text(&self.rule),
            text(&self.path),
            fields.join(","),
            text(&self.message)
        );
        if let Some(e) = &self.error {
            out.push_str(&format!(",\"error\":{}", text(e)));
        }
        out.push('}');
        out
    }

    /// The same object as a JSON value.
    #[must_use]
    pub fn to_json(&self) -> Value {
        serde_json::from_str(&self.to_text()).unwrap_or(Value::Null)
    }

    /// Reads one entry of the `violations` member; `None` when it is not one.
    #[must_use]
    pub fn from_json(v: &Value) -> Option<Violation> {
        let obj = v.as_object()?;
        let text = |k: &str| obj.get(k).and_then(Value::as_str).map(str::to_string);
        Some(Violation {
            rule: text("rule")?,
            path: text("path")?,
            fields: obj
                .get("fields")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|f| f.as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
            message: text("message").unwrap_or_default(),
            error: text("error"),
        })
    }

    /// The `issues` entry that a client without rules shows for this violation.
    #[must_use]
    pub fn issue(&self) -> super::error::Issue {
        super::error::Issue {
            path: self.path.clone(),
            code: if self.error.is_some() { "rule_error" } else { "rule" }.to_string(),
            message: self.message.clone(),
        }
    }
}

/// The rule part of a `ValidationError` problem: its violations and whether the client raised it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rulings {
    pub violations: Vec<Violation>,
    pub local: bool,
}

impl Rulings {
    /// `None` when there is nothing to carry.
    #[must_use]
    pub fn boxed(violations: Vec<Violation>, local: bool) -> Option<Box<Rulings>> {
        if violations.is_empty() && !local {
            None
        } else {
            Some(Box::new(Rulings { violations, local }))
        }
    }
}

/// The `violations` member of a problem document, as text.
#[must_use]
pub fn violations_text(violations: &[Violation]) -> String {
    let items: Vec<String> = violations.iter().map(Violation::to_text).collect();
    format!("[{}]", items.join(","))
}

/// The members of a `422 ValidationError` that has rule violations: `data` (one issue each) and
/// the list of violations.
#[must_use]
pub fn rejection_data(violations: &[Violation]) -> Value {
    let issues: Vec<Value> =
        violations.iter().map(|v| serde_json::to_value(v.issue()).unwrap_or(Value::Null)).collect();
    serde_json::json!({ "issues": issues })
}

/// The invariants of a generated declaration.
pub trait Invariants {
    /// Evaluates this declaration's own rules, in list order, and appends the violations. `path`
    /// is the path of this value. Nested values are visited by the schema walk, not here.
    fn check_rules(&self, mode: Mode, path: &str, out: &mut Vec<Violation>);
}

/// What the schema calls for every object of a declaration with rules: reads the object from its
/// typed JSON form and evaluates its own rules. An object that cannot be read is skipped (the
/// shape error is reported by validation, never as a violation).
pub fn run<T: Invariants + DeserializeOwned>(node: &Value, mode: Mode, path: &str, out: &mut Vec<Violation>) {
    if let Ok(value) = serde_json::from_value::<T>(node.clone()) {
        value.check_rules(mode, path, out);
    }
}

/// Records the outcome of one rule: nothing when it holds, a violation when it does not or could
/// not be evaluated.
pub fn record(out: &mut Vec<Violation>, path: &str, id: &str, name: &str, fields: &[&str], message: &str, verdict: Res<bool>) {
    let (message, error) = match verdict {
        Ok(true) => return,
        Ok(false) => (message.to_string(), None),
        Err(f) => (format!("rule {name} could not be evaluated"), Some(f.as_str().to_string())),
    };
    out.push(Violation {
        rule: id.to_string(),
        path: path.to_string(),
        fields: fields.iter().map(|f| (*f).to_string()).collect(),
        message,
        error,
    });
}

// ---------------------------------------------------------------------------
// The schema walk
// ---------------------------------------------------------------------------

/// Checks the invariants of a wire value and of everything inside it. Defaults are applied
/// first. The value must have passed validation. `path` names the root (`$`, `body`).
#[must_use]
pub fn check_wire(schema: &Schema, wire: &Value, path: &str, mode: Mode) -> Vec<Violation> {
    let filled = if mode == Mode::Request { fill_readonly(schema, wire, 0) } else { wire.clone() };
    let typed = schema.from_wire(&filled, false);
    let mut out = Vec::new();
    walk(schema, &typed, path, mode, &mut out);
    out
}

/// The violations of a typed value, through its schema (`Booking::check()` calls this).
#[must_use]
pub fn check_value<T: super::error::VeldDecl>(value: &T, mode: Mode) -> Vec<Violation> {
    let typed = serde_json::to_value(value).unwrap_or(Value::Null);
    let wire = T::schema().to_wire(&typed, mode == Mode::Request, false);
    check_wire(T::schema(), &wire, "$", mode)
}

/// The walk: own rules of an object before its fields, list items by index, in the order and
/// with the path syntax of validation issues.
fn walk(schema: &Schema, v: &Value, path: &str, mode: Mode, out: &mut Vec<Violation>) {
    match schema {
        Schema::Scalar(..) | Schema::Enum(_) => {}
        Schema::List(of, _) => walk_items(of, v, path, mode, out),
        Schema::Map(of, _) => {
            for (k, x) in v.as_object().into_iter().flatten() {
                walk(of, x, &format!("{path}.{k}"), mode, out);
            }
        }
        Schema::Page(of) => {
            if let Some(items) = v.get("items") {
                walk_items(of, items, &format!("{path}.items"), mode, out);
            }
        }
        Schema::Model(m) => walk_model(m, v, path, mode, out),
        Schema::Union(u) => {
            let label = v.get(u.discriminator).and_then(Value::as_str);
            if let Some((_, arm)) = label.and_then(|l| u.arms.iter().find(|(name, _)| *name == l)) {
                walk(arm, v, path, mode, out);
            }
        }
    }
}

fn walk_items(of: &Schema, v: &Value, path: &str, mode: Mode, out: &mut Vec<Violation>) {
    for (i, x) in v.as_array().into_iter().flatten().enumerate() {
        walk(of, x, &format!("{path}[{i}]"), mode, out);
    }
}

fn walk_model(m: &ModelSchema, v: &Value, path: &str, mode: Mode, out: &mut Vec<Violation>) {
    if let Some(check) = m.invariants {
        check(v, mode, path, out);
    }
    for f in m.fields {
        if let Some(x) = v.get(f.wire) {
            walk(f.schema, x, &format!("{path}.{}", f.wire), mode, out);
        }
    }
}

// ---------------------------------------------------------------------------
// Request position: `@readonly` fields are absent
// ---------------------------------------------------------------------------

/// How deep a placeholder for a required read-only field may nest.
const PLACEHOLDER_DEPTH: usize = 6;

/// A request body has no `@readonly` fields, but a generated struct has them. The rules that read
/// them are skipped in request mode, so a placeholder of the right shape stands in for each absent
/// one while the value is read; no evaluated rule can see it.
fn fill_readonly(schema: &Schema, v: &Value, depth: usize) -> Value {
    match (schema, v) {
        (Schema::List(of, _), Value::Array(a)) => Value::Array(a.iter().map(|x| fill_readonly(of, x, depth)).collect()),
        (Schema::Map(of, _), Value::Object(m)) => {
            Value::Object(m.iter().map(|(k, x)| (k.clone(), fill_readonly(of, x, depth))).collect())
        }
        (Schema::Page(of), Value::Object(m)) => {
            let mut out = m.clone();
            if let Some(items) = m.get("items") {
                out.insert("items".into(), fill_readonly(&Schema::List(of, super::schema::Rules::NONE), items, depth));
            }
            Value::Object(out)
        }
        (Schema::Model(m), Value::Object(obj)) => {
            let mut out = obj.clone();
            for f in m.fields {
                match obj.get(f.wire) {
                    Some(x) => {
                        out.insert(f.wire.to_string(), fill_readonly(f.schema, x, depth));
                    }
                    None if f.readonly && !f.optional && f.default.is_none() => {
                        out.insert(f.wire.to_string(), placeholder(f.schema, depth));
                    }
                    None => {}
                }
            }
            Value::Object(out)
        }
        (Schema::Union(u), Value::Object(_)) => {
            let label = v.get(u.discriminator).and_then(Value::as_str);
            match label.and_then(|l| u.arms.iter().find(|(name, _)| *name == l)) {
                Some((_, arm)) => fill_readonly(arm, v, depth),
                None => v.clone(),
            }
        }
        _ => v.clone(),
    }
}

/// A value of the shape of `schema` that stands for an absent required field.
fn placeholder(schema: &Schema, depth: usize) -> Value {
    match schema {
        Schema::Scalar(k, _) => match k {
            Scalar::String | Scalar::Bytes => Value::String(String::new()),
            Scalar::Int | Scalar::Float => Value::from(0),
            Scalar::Long | Scalar::Decimal => Value::String("0".into()),
            Scalar::Bool => Value::Bool(false),
            Scalar::Date => Value::String("1970-01-01".into()),
            Scalar::Datetime => Value::String("1970-01-01T00:00:00Z".into()),
            Scalar::Time => Value::String("00:00:00".into()),
            Scalar::Uuid => Value::String("00000000-0000-0000-0000-000000000000".into()),
            Scalar::Json => Value::Null,
        },
        Schema::Enum(variants) => variants.first().map_or(Value::Null, |v| Value::String((*v).to_string())),
        Schema::List(..) => Value::Array(Vec::new()),
        Schema::Map(..) => Value::Object(Map::new()),
        Schema::Page(_) => serde_json::json!({ "items": [], "total": "0", "page": 0, "size": 0 }),
        Schema::Model(m) if depth < PLACEHOLDER_DEPTH => Value::Object(required_fields(m.fields, depth + 1)),
        Schema::Union(u) if depth < PLACEHOLDER_DEPTH => match u.arms.first() {
            Some((label, arm)) => match placeholder(arm, depth + 1) {
                Value::Object(mut o) => {
                    o.insert(u.discriminator.to_string(), Value::String((*label).to_string()));
                    Value::Object(o)
                }
                other => other,
            },
            None => Value::Null,
        },
        Schema::Model(_) | Schema::Union(_) => Value::Null,
    }
}

fn required_fields(fields: &[Field], depth: usize) -> Map<String, Value> {
    fields
        .iter()
        .filter(|f| !f.optional && f.default.is_none())
        .map(|f| (f.wire.to_string(), placeholder(f.schema, depth)))
        .collect()
}

// ---------------------------------------------------------------------------
// Helpers called by generated rule code
// ---------------------------------------------------------------------------

/// An optional value that the compiler proved present.
pub fn some<T>(value: &Option<T>) -> Res<&T> {
    value.as_ref().ok_or(Fail::Input)
}

/// A `decimal` field or literal.
pub fn dec(text: &str) -> Res<Decimal> {
    Decimal::parse(text).ok_or(Fail::Input)
}

/// A `datetime` field as milliseconds since the epoch (UTC, truncated to the millisecond).
pub fn datetime(text: &str) -> Res<i64> {
    instant::parse_datetime(text).ok_or(Fail::Input)
}

/// A `date` field as days since the epoch.
pub fn date(text: &str) -> Res<i64> {
    instant::parse_date(text).ok_or(Fail::Input)
}

/// A `time` field as milliseconds since midnight.
pub fn time(text: &str) -> Res<i64> {
    instant::parse_time(text).ok_or(Fail::Input)
}

/// `date - date`: a whole number of days as a duration in milliseconds.
pub fn date_diff(a: i64, b: i64) -> Res<i64> {
    Ok(num::mul_i64(num::sub_i64(a, b)?, instant::MS_PER_DAY)?)
}

/// A list converted element by element (the elements of a comprehension or of `in`).
pub fn list<'a, T, U>(items: &'a [T], f: impl Fn(&'a T) -> Res<U>) -> Res<Vec<U>> {
    items.iter().map(f).collect()
}

/// A length or count as an `int`; more than `i32::MAX` is an overflow.
pub fn len_n(n: usize) -> Res<i32> {
    i32::try_from(n).map_err(|_| Fail::Eval(EvalError::Overflow))
}

/// The length of a string in code points.
pub fn len_str(s: &str) -> Res<i32> {
    len_n(text::len_code_points(s))
}

/// `matches(s, "re")`: the regex subset of the language, searching anywhere unless anchored.
pub fn matches(s: &str, pattern: &str) -> Res<bool> {
    Regex::new(pattern).map(|re| re.is_match(s)).ok_or(Fail::Input)
}

/// `needle in list`: `eq` compares the needle with one element.
pub fn contains<T>(items: &[T], eq: impl Fn(&T) -> bool) -> bool {
    items.iter().any(eq)
}

/// `all(list, x => body)`: true for an empty list; stops at the first false.
pub fn all<T>(items: &[T], f: impl Fn(&T) -> Res<bool>) -> Res<bool> {
    for item in items {
        if !f(item)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `any(list, x => body)`: false for an empty list; stops at the first true.
pub fn any<T>(items: &[T], f: impl Fn(&T) -> Res<bool>) -> Res<bool> {
    for item in items {
        if f(item)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `count(list, x => body)`, checked.
pub fn count<T>(items: &[T], f: impl Fn(&T) -> Res<bool>) -> Res<i32> {
    let mut hits = 0usize;
    for item in items {
        if f(item)? {
            hits += 1;
        }
    }
    len_n(hits)
}

/// `sum` of `int`, checked.
pub fn sum_i32<T>(items: &[T], f: impl Fn(&T) -> Res<i32>) -> Res<i32> {
    items.iter().try_fold(0i32, |acc, x| Ok(num::add_i32(acc, f(x)?)?))
}

/// `sum` of `long`, checked.
pub fn sum_i64<T>(items: &[T], f: impl Fn(&T) -> Res<i64>) -> Res<i64> {
    items.iter().try_fold(0i64, |acc, x| Ok(num::add_i64(acc, f(x)?)?))
}

/// `sum` of `float`; a result that is not finite is an error.
pub fn sum_f64<T>(items: &[T], f: impl Fn(&T) -> Res<f64>) -> Res<f64> {
    items.iter().try_fold(0.0f64, |acc, x| Ok(num::add_f64(acc, f(x)?)?))
}

/// `sum` of `decimal`, exact.
pub fn sum_dec<T>(items: &[T], f: impl Fn(&T) -> Res<Decimal>) -> Res<Decimal> {
    items.iter().try_fold(Decimal::from_i64(0), |acc, x| Ok(acc.add(&f(x)?)))
}

// ---------------------------------------------------------------------------
// The server's view
// ---------------------------------------------------------------------------

/// What a server does with a violated rule on its own output (`spec/rules.md` section 5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuleOutput {
    /// Answer `500 Internal` (no rule detail); the violation goes to the hook. The default.
    #[default]
    Strict,
    /// Send the response unchanged; the violation goes to the hook.
    Audit,
    /// Output rules are not evaluated.
    Off,
}

/// Where the checked value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Input,
    Output,
}

/// What happened to the request or response because of the violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Rejected,
    Audited,
}

/// What `on_rule_violation` receives.
#[derive(Debug, Clone)]
pub struct RuleEvent {
    /// The rule id.
    pub rule: String,
    /// The operation signature.
    pub operation: String,
    pub position: Position,
    pub outcome: Outcome,
    pub violation: Violation,
    /// How long the check took, in microseconds.
    pub elapsed_micros: u64,
}

/// A stopwatch for the `elapsed_micros` of an event.
#[derive(Debug, Clone, Copy)]
pub struct Stopwatch(Instant);

impl Stopwatch {
    /// Starts now.
    #[must_use]
    pub fn start() -> Self {
        Stopwatch(Instant::now())
    }

    /// Microseconds since `start`.
    #[must_use]
    pub fn micros(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_micros()).unwrap_or(u64::MAX)
    }
}
