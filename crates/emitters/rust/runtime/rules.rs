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
use serde_json::Value;

use super::regex::Regex;
use super::schema::{ModelSchema, Schema, placeholder};

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
    // #if endpoint_rules
    /// The key of an optional fact reads a value that is absent: there is no such fact.
    Absent,
    // #endif
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
            // #if endpoint_rules
            Fail::Absent => "absent",
            // #endif
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

    // @if derived
    /// Like [`Self::check_rules`] for a value whose derived fields in `absent` (wire names) could
    /// not be computed: a rule that reads one of them is skipped, the cause is already reported
    /// (`spec/rules.md` section 17.2). A declaration with derived fields overrides it.
    fn check_rules_absent(&self, mode: Mode, path: &str, absent: &[&str], out: &mut Vec<Violation>) {
        let _ = absent;
        self.check_rules(mode, path, out);
    }
    // @end
}

/// What the schema calls for every object of a declaration with rules: reads the object from its
/// typed JSON form and evaluates its own rules. An object that cannot be read is skipped (the
/// shape error is reported by validation, never as a violation).
// @if !derived
pub fn run<T: Invariants + DeserializeOwned>(node: &Value, mode: Mode, path: &str, out: &mut Vec<Violation>) {
    if let Ok(value) = serde_json::from_value::<T>(node.clone()) {
        value.check_rules(mode, path, out);
    }
}
// @end
// @if derived
pub fn run<T: Invariants + DeserializeOwned + super::error::VeldDecl>(node: &Value, mode: Mode, path: &str, out: &mut Vec<Violation>) {
    let mut read = node.clone();
    let mut absent: Vec<&str> = Vec::new();
    if let (Some(model), Value::Object(map)) = (T::schema().model(), &mut read) {
        for f in model.fields.iter().filter(|f| f.derived) {
            // A derived field that could not be computed: a stand-in is read, and no rule that
            // reads it is evaluated.
            if !map.contains_key(f.wire) {
                absent.push(f.wire);
                map.insert(f.wire.to_string(), stand_in(f.schema));
            }
        }
    }
    if let Ok(value) = serde_json::from_value::<T>(read) {
        value.check_rules_absent(mode, path, &absent, out);
    }
}
// @end

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
    // @if !derived
    let typed = schema.from_wire(&filled, false);
    let mut out = Vec::new();
    // @end
    // @if derived
    // The derived fields are computed first (a value the caller sent for one is replaced), then the
    // invariants read them; the violations of the derived fields come first.
    let mut typed = schema.from_wire(&filled, false);
    let mut out = Vec::new();
    derive_walk(schema, &mut typed, path, mode, &mut out);
    // @end
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

// ---------------------------------------------------------------------------
// Helpers called by generated rule code
// ---------------------------------------------------------------------------

/// An optional value that the compiler proved present.
pub fn some<T>(value: &Option<T>) -> Res<&T> {
    value.as_ref().ok_or(Fail::Input)
}

// #if endpoint_rules
/// An optional value read by the key of an optional fact: absent means there is no fact.
pub fn absent<T>(value: &Option<T>) -> Res<&T> {
    value.as_ref().ok_or(Fail::Absent)
}

/// A value as typed JSON (the payload of an `else` clause).
pub fn json<T: serde::Serialize>(value: &T) -> Res<Value> {
    serde_json::to_value(value).map_err(|_| Fail::Input)
}

/// The violation of an endpoint rule, `None` when it holds. Like [`record`], for one rule.
#[must_use]
pub fn violated(path: &str, id: &str, name: &str, fields: &[&str], message: &str, verdict: Res<bool>) -> Option<Violation> {
    let mut out = Vec::new();
    record(&mut out, path, id, name, fields, message, verdict);
    out.pop()
}

/// The clock reading of a request: milliseconds since the epoch.
#[must_use]
pub fn clock_ms(now: std::time::SystemTime) -> i64 {
    match now.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_millis()).unwrap_or(i64::MAX),
        Err(e) => i64::try_from(e.duration().as_millis()).map_or(i64::MIN, |n| -n),
    }
}

/// The error a failed `require` without `else` answers with: `422 ValidationError` with one issue
/// and the one violation (`spec/rules.md` sections 7 and 9.3).
#[must_use]
pub fn request_rejection(validation_sig: &str, op: &str, violation: Violation) -> super::error::Error {
    let violations = vec![violation];
    super::error::Error::Problem(super::error::Problem {
        sig: validation_sig.to_string(),
        title: "ValidationError".to_string(),
        status: 422,
        op: Some(op.to_string()),
        data: rejection_data(&violations),
        schema: None,
        rulings: Rulings::boxed(violations, false),
    })
}

/// The error a failed `require ... else E { .. }` answers with: the declared error `sig` of `ep`
/// built from the payload (typed JSON by wire name; defaults are applied), carrying the one
/// violation. The error it builds is checked like any other value of its type: when it breaks one
/// of its own invariants the server could not build what it promised, and the violations are the
/// `Err` (empty when the endpoint does not declare the error, which the compiler rules out).
///
/// # Errors
/// The violations of the error's own invariants.
pub fn else_rejection(
    ep: &'static super::spec::EndpointSpec,
    sig: &str,
    payload: serde_json::Map<String, Value>,
    violation: Violation,
) -> Result<super::error::Error, Vec<Violation>> {
    let Some(e) = ep.errors.iter().find(|e| e.sig == sig) else {
        return Err(Vec::new());
    };
    let data = e.schema.to_wire(&Value::Object(payload), false, false);
    // @if !derived
    let broken = check_wire(e.schema, &data, "error", Mode::Full);
    // @end
    // @if derived
    // The error is an output: its derived fields are computed before its own rules are checked.
    let (data, mut broken) = complete_wire(e.schema, &data, "error");
    broken.extend(check_computed(e.schema, &data, "error", Mode::Full));
    // @end
    if !broken.is_empty() {
        return Err(broken);
    }
    Ok(super::error::Error::Problem(super::error::Problem {
        sig: e.sig.to_string(),
        title: e.name.to_string(),
        status: e.status,
        op: Some(ep.sig.to_string()),
        data,
        schema: Some(e.schema),
        rulings: Rulings::boxed(vec![violation], false),
    }))
}

// #endif
// #if guards

/// A state change that the endpoint's guard checked (`spec/rules.md` section 10.2): the wire path
/// of the field inside the fact, the state it had when it was checked and the target. The states
/// are values of the state machine's generated enum.
///
/// **This is a guard, not an atomic guarantee.** A second request can change the row after the
/// check. Write the change conditionally (compare-and-swap, or a row lock) in your own
/// transaction, and answer `InvalidTransition` yourself when that write finds another value.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Transition<S> {
    /// The wire path of the state field inside the fact: `status`.
    pub field: String,
    /// The state the field had when the guard checked it.
    pub from: S,
    /// The state the endpoint moves the field to.
    pub to: S,
}

/// The violation of a guard (a policy or a transition): `path` is `request`, and there is no
/// evaluation error to report.
#[must_use]
pub fn guard_violation(rule: &str, fields: &[&str], message: &str) -> Violation {
    Violation {
        rule: rule.to_string(),
        path: "request".to_string(),
        fields: fields.iter().map(|f| (*f).to_string()).collect(),
        message: message.to_string(),
        error: None,
    }
}

/// What step 6 answers when a guard refuses: the declared error `sig` of `ep`, built from the
/// string members `data`, carrying the one violation. An error that the endpoint does not declare,
/// or that breaks its own invariants, is `500 Internal`.
#[must_use]
pub fn refusal(
    ep: &'static super::spec::EndpointSpec,
    sig: &str,
    data: &[(&str, &str)],
    violation: Violation,
) -> super::server::Halt {
    let payload: serde_json::Map<String, Value> = data
        .iter()
        .map(|(k, v)| ((*k).to_string(), Value::String((*v).to_string())))
        .collect();
    match else_rejection(ep, sig, payload, violation) {
        Ok(err) => super::server::Halt::Error(err),
        Err(broken) => super::server::Halt::Broken(broken),
    }
}

/// The text of a denial: `None` when the policy permits. A policy whose arguments could not be
/// evaluated denies (it fails closed) with `not permitted`.
#[must_use]
pub fn denial(verdict: Res<Result<(), String>>) -> Option<String> {
    match verdict {
        Ok(Ok(())) => None,
        Ok(Err(text)) => Some(text),
        Err(_) => Some("not permitted".to_string()),
    }
}
// #endif
// #if policies

/// The system clock as milliseconds since the epoch: what `now` is for a policy called outside a
/// request.
#[must_use]
pub fn system_now_ms() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_millis()).unwrap_or(i64::MAX),
        Err(e) => i64::try_from(e.duration().as_millis()).map_or(i64::MIN, |n| -n),
    }
}
// #endif
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
// #if rollout

// ---------------------------------------------------------------------------
// Audited rules (`@rollout(audit)`, `spec/rules.md` section 17.1)
// ---------------------------------------------------------------------------

thread_local! {
    /// Where audited rules report while a server checks a value. Rule checks are synchronous, so
    /// the sink is opened right before a walk and closed right after it.
    static SINK: std::cell::RefCell<Option<Vec<Violation>>> = const { std::cell::RefCell::new(None) };
}

/// One audited rule of a declaration: evaluated when a server listens, skipped when nobody does
/// (a client never opens a sink, so it skips audited rules in the local check).
pub fn audit(path: &str, id: &str, name: &str, fields: &[&str], message: &str, verdict: impl FnOnce() -> Res<bool>) {
    SINK.with(|sink| {
        if let Some(out) = sink.borrow_mut().as_mut() {
            record(out, path, id, name, fields, message, verdict());
        }
    });
}

/// The violations of the audited rules of a wire value and of everything inside it (the rules that
/// reject are not looked at here). The value must have passed validation.
#[must_use]
pub fn audit_wire(schema: &Schema, wire: &Value, path: &str, mode: Mode) -> Vec<Violation> {
    SINK.with(|sink| *sink.borrow_mut() = Some(Vec::new()));
    let _ = check_wire(schema, wire, path, mode);
    SINK.with(|sink| sink.borrow_mut().take()).unwrap_or_default()
}
// #endif
// @if derived

// ---------------------------------------------------------------------------
// Derived fields (`spec/rules.md` section 17.2)
// ---------------------------------------------------------------------------

/// The derived fields of one object of a generated declaration.
pub trait Derive {
    /// Computes the derived fields of this value, in dependency order. Each value is written to
    /// `self` (so the next one can read it) and to `node`, the typed JSON of the object; a field
    /// that could not be computed is left out of `node`. `path` is the path of this value.
    fn derive_fields(&mut self, node: &mut serde_json::Map<String, Value>, mode: Mode, path: &str, out: &mut Vec<Violation>);
}

/// What the schema calls for every object of a declaration with derived fields: reads the object
/// (a value that was there for a derived field is not believed: a stand-in is read), computes
/// the derived fields and writes them back.
pub fn derive_object<T: Derive + DeserializeOwned + super::error::VeldDecl>(node: &mut Value, mode: Mode, path: &str, out: &mut Vec<Violation>) {
    let (Value::Object(map), Some(model)) = (node, T::schema().model()) else {
        return;
    };
    for f in model.fields.iter().filter(|f| f.derived) {
        map.remove(f.wire);
    }
    let mut read = map.clone();
    for f in model.fields.iter().filter(|f| f.derived) {
        read.insert(f.wire.to_string(), stand_in(f.schema));
    }
    if let Ok(mut value) = serde_json::from_value::<T>(Value::Object(read)) {
        value.derive_fields(map, mode, path, out);
    }
}

/// A stand-in of the shape of `schema`, in typed form: what is read where a derived value is not
/// (yet) there. It is never evaluated by a rule.
fn stand_in(schema: &Schema) -> Value {
    schema.from_wire(&placeholder(schema, 0), false)
}

/// The value of a derived field that was computed, `None` when it could not be: an evaluation
/// error is one violation (its rule is the id of the field, `error` is set); a field that reads
/// another one that has no value stays without one and without a second violation.
pub fn derived<T>(out: &mut Vec<Violation>, path: &str, id: &str, name: &str, fields: &[&str], verdict: Res<T>) -> Option<T> {
    match verdict {
        Ok(v) => Some(v),
        Err(Fail::Eval(e)) => {
            out.push(Violation {
                rule: id.to_string(),
                path: path.to_string(),
                fields: fields.iter().map(|f| (*f).to_string()).collect(),
                message: format!("field {name} could not be computed"),
                error: Some(e.as_str().to_string()),
            });
            None
        }
        Err(_) => None,
    }
}

/// Writes a computed value into the typed JSON of its object.
pub fn put<T: serde::Serialize>(node: &mut serde_json::Map<String, Value>, wire: &str, value: &T) {
    node.insert(wire.to_string(), serde_json::to_value(value).unwrap_or(Value::Null));
}

/// A `date` as `YYYY-MM-DD` (days since the epoch).
#[must_use]
pub fn format_date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// A `time` as `HH:MM:SS`, with `.mmm` when the milliseconds are not zero.
#[must_use]
pub fn format_time(ms_of_day: i64) -> String {
    let ms = ms_of_day.rem_euclid(instant::MS_PER_DAY);
    let (h, m, s, f) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    if f == 0 { format!("{h:02}:{m:02}:{s:02}") } else { format!("{h:02}:{m:02}:{s:02}.{f:03}") }
}

/// A `datetime` as `YYYY-MM-DDTHH:MM:SS[.mmm]Z` (milliseconds since the epoch).
#[must_use]
pub fn format_datetime(ms: i64) -> String {
    format!("{}T{}Z", format_date(ms.div_euclid(instant::MS_PER_DAY)), format_time(ms))
}

/// Computes the derived fields of every value inside `v` (typed form) and then of `v`, so that
/// an expression reads values that are complete: list items, map values, page items and union
/// arms first, the fields of an object in declaration order, then the object itself.
pub fn derive_walk(schema: &Schema, v: &mut Value, path: &str, mode: Mode, out: &mut Vec<Violation>) {
    match schema {
        Schema::Scalar(..) | Schema::Enum(_) => {}
        Schema::List(of, _) => {
            for (i, x) in v.as_array_mut().into_iter().flatten().enumerate() {
                derive_walk(of, x, &format!("{path}[{i}]"), mode, out);
            }
        }
        Schema::Map(of, _) => {
            for (k, x) in v.as_object_mut().into_iter().flatten() {
                derive_walk(of, x, &format!("{path}.{k}"), mode, out);
            }
        }
        Schema::Page(of) => {
            if let Some(items) = v.get_mut("items") {
                derive_walk(&Schema::List(of, super::schema::Rules::NONE), items, &format!("{path}.items"), mode, out);
            }
        }
        Schema::Model(m) => {
            for f in m.fields.iter().filter(|f| !f.derived) {
                if let Some(x) = v.get_mut(f.wire) {
                    derive_walk(f.schema, x, &format!("{path}.{}", f.wire), mode, out);
                }
            }
            if let Some(derive) = m.derive {
                derive(v, mode, path, out);
            }
        }
        Schema::Union(u) => {
            let label = v.get(u.discriminator).and_then(Value::as_str).map(str::to_string);
            if let Some((_, arm)) = label.and_then(|l| u.arms.iter().find(|(name, _)| *name == l)) {
                derive_walk(arm, v, path, mode, out);
            }
        }
    }
}

/// A stand-in of its shape for every derived member that is absent, at every depth: a value that
/// had a derived field fail, or that reads a field a request does not have, is still read by serde
/// as the struct that has the field.
pub fn fill_derived(schema: &Schema, v: &mut Value) {
    match schema {
        Schema::Scalar(..) | Schema::Enum(_) => {}
        Schema::List(of, _) => v.as_array_mut().into_iter().flatten().for_each(|x| fill_derived(of, x)),
        Schema::Map(of, _) => v.as_object_mut().into_iter().flat_map(serde_json::Map::values_mut).for_each(|x| fill_derived(of, x)),
        Schema::Page(of) => {
            if let Some(items) = v.get_mut("items") {
                fill_derived(&Schema::List(of, super::schema::Rules::NONE), items);
            }
        }
        Schema::Model(m) => {
            if let Value::Object(map) = v {
                for f in m.fields {
                    if !f.derived {
                        if let Some(x) = map.get_mut(f.wire) {
                            fill_derived(f.schema, x);
                        }
                    } else if !map.contains_key(f.wire) {
                        map.insert(f.wire.to_string(), stand_in(f.schema));
                    }
                }
            }
        }
        Schema::Union(u) => {
            let label = v.get(u.discriminator).and_then(Value::as_str).map(str::to_string);
            if let Some((_, arm)) = label.and_then(|l| u.arms.iter().find(|(name, _)| *name == l)) {
                fill_derived(arm, v);
            }
        }
    }
}

/// `Order::derive`: computes the derived fields of a value and of everything inside it, and
/// returns the violations of the ones that could not be computed (they hold a stand-in).
pub fn derive_value<T: serde::Serialize + DeserializeOwned + super::error::VeldDecl>(value: &mut T, mode: Mode) -> Vec<Violation> {
    let mut typed = serde_json::to_value(&*value).unwrap_or(Value::Null);
    let mut out = Vec::new();
    derive_walk(T::schema(), &mut typed, "$", mode, &mut out);
    fill_derived(T::schema(), &mut typed);
    if let Ok(computed) = serde_json::from_value::<T>(typed) {
        *value = computed;
    }
    out
}

/// A value the application handed over (a fact, the principal) with its derived fields computed:
/// the violations, when one could not be, are `Err`.
///
/// # Errors
/// The violations of the derived fields that could not be computed.
pub fn complete_with<T: serde::Serialize + DeserializeOwned>(schema: &Schema, value: &mut T) -> Result<(), Vec<Violation>> {
    let mut typed = serde_json::to_value(&*value).unwrap_or(Value::Null);
    let mut out = Vec::new();
    derive_walk(schema, &mut typed, "$", Mode::Full, &mut out);
    if !out.is_empty() {
        return Err(out);
    }
    if let Ok(computed) = serde_json::from_value::<T>(typed) {
        *value = computed;
    }
    Ok(())
}

/// A wire value of an output with its derived fields computed, and the violations of the ones
/// that could not be.
#[must_use]
pub fn complete_wire(schema: &Schema, wire: &Value, path: &str) -> (Value, Vec<Violation>) {
    let mut typed = schema.from_wire(wire, false);
    let mut out = Vec::new();
    derive_walk(schema, &mut typed, path, Mode::Full, &mut out);
    (schema.to_wire(&typed, false, false), out)
}

/// Like [`check_wire`] for a value whose derived fields are computed already (an output): the
/// invariants only.
#[must_use]
pub fn check_computed(schema: &Schema, wire: &Value, path: &str, mode: Mode) -> Vec<Violation> {
    let typed = schema.from_wire(wire, false);
    let mut out = Vec::new();
    walk(schema, &typed, path, mode, &mut out);
    out
}
// @end
