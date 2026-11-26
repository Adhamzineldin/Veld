//! The shared test vectors of `spec/rules.md` section 15: loading `tests/rules/**/*.json` and
//! running a case against an IR.
//!
//! The harness that compiles the `.veld` files and the per-language harnesses reuse this
//! module: [`load_cases`] reads a vector file, [`run_case`] checks one case.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Deserialize;
use serde_json::Value as Json;
use veld_ir::Ir;

use crate::check::{Options, check_value_audited, with_defaults};
use crate::error::InputError;

/// One expected violation. Messages and rule ids are covered by snapshots, not by vectors.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Expected {
    pub rule: String,
    pub path: String,
    pub fields: Vec<String>,
    /// `overflow`, `division_by_zero` or `non_finite`.
    #[serde(default)]
    pub error: Option<String>,
}

/// One case of a vector file.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Case {
    pub name: String,
    /// A model of the contract.
    #[serde(rename = "type")]
    pub ty: String,
    /// The value in wire form.
    pub value: Json,
    /// The value is a request body.
    #[serde(default)]
    pub request: bool,
    /// Run the partial check (TypeScript and the reference evaluator only).
    #[serde(default)]
    pub partial: bool,
    /// The violations of enforced rules, which reject.
    pub expect: Vec<Expected>,
    /// The violations of audited rules (`@rollout(audit)`, section 17.1): reported to the hook
    /// and never in `expect`. Absent means none. A client does not check audited rules at all.
    #[serde(default)]
    pub audited: Vec<Expected>,
    /// The output mode of the server for a case in response position: `"off"` means no output
    /// rule is evaluated, audited ones included, so a server reports nothing (section 5.3). Absent
    /// means `strict`. It does not apply to a case with `"request": true`: input rules are always
    /// evaluated.
    #[serde(default)]
    pub output: Option<String>,
    /// What the derived fields of the value are computed to (`spec/rules.md` sections 15 and
    /// 17.2): a path from `$` (`$.total`, `$.items[1].line`) to the value in wire form that the
    /// value has there once its defaults are applied and its derived fields computed. Only the
    /// paths named are compared. Absent means nothing is said. A field a failed expression left
    /// without a value is `null`.
    #[serde(default)]
    pub computed: BTreeMap<String, Json>,
    /// The flags that are on (`spec/platform.md` section 11). A flag a case does not name is off.
    #[serde(default)]
    pub flags: Vec<String>,
    /// `"failed"`: the flag provider failed, so a statement that restricts applies and one that
    /// grants does not. `flags` is then ignored.
    #[serde(default)]
    pub provider: Option<String>,
}

impl Case {
    /// The flag state the case runs under.
    #[must_use]
    pub fn flag_state(&self) -> crate::flags::FlagState {
        if self.partial {
            // A client has no provider and skips every flagged invariant (`spec/platform.md`
            // section 11).
            crate::flags::FlagState::default()
        } else if self.provider.as_deref() == Some("failed") {
            crate::flags::FlagState::ProviderFailed
        } else {
            crate::flags::FlagState::on(self.flags.iter().cloned())
        }
    }

    /// True when the case is a response checked by a server whose output mode is `off`.
    #[must_use]
    pub fn output_is_off(&self) -> bool {
        !self.request && self.output.as_deref() == Some("off")
    }
}

#[derive(Deserialize)]
struct CaseFile {
    cases: Vec<Case>,
}

/// Reads the cases of a vector file.
///
/// # Errors
/// The JSON is not a vector file.
pub fn load_cases(text: &str) -> Result<Vec<Case>, serde_json::Error> {
    serde_json::from_str::<CaseFile>(text).map(|f| f.cases)
}

/// What a violation is compared by: `(rule name, path, fields, error)`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    pub rule: String,
    pub path: String,
    pub fields: Vec<String>,
    pub error: Option<String>,
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at {} reading [{}]",
            self.rule,
            self.path,
            self.fields.join(", ")
        )?;
        match &self.error {
            Some(e) => write!(f, " error {e}"),
            None => Ok(()),
        }
    }
}

fn list(keys: &[Key]) -> String {
    if keys.is_empty() {
        return " none".to_owned();
    }
    keys.iter().map(|k| format!("\n    {k}")).collect()
}

/// Why a case failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Mismatch {
    #[error("case `{case}`: the IR has no declaration named `{ty}`")]
    UnknownType { case: String, ty: String },
    #[error("case `{case}`: the value was rejected: {error}")]
    Input { case: String, error: InputError },
    #[error(
        "case `{case}`: violations differ\n  expected but not reported:{}\n  reported but not expected:{}",
        list(.missing),
        list(.unexpected)
    )]
    Differs {
        case: String,
        missing: Vec<Key>,
        unexpected: Vec<Key>,
    },
    #[error("case `{case}`: the value at `{path}` is {found}, the case computes {expected}")]
    Computed {
        case: String,
        path: String,
        expected: String,
        found: String,
    },
    #[error(
        "case `{case}`: audited violations differ
  expected but not reported:{}
  reported but not expected:{}",
        list(.missing),
        list(.unexpected)
    )]
    AuditedDiffers {
        case: String,
        missing: Vec<Key>,
        unexpected: Vec<Key>,
    },
}

/// Runs one case: finds the model by name, checks the value with the case's flags and compares
/// the violations with `expect` as a set.
///
/// # Errors
/// A [`Mismatch`] that says what differs.
pub fn run_case(ir: &Ir, case: &Case) -> Result<(), Mismatch> {
    let seen;
    let ir = if ir.flag_usage().is_empty() {
        ir
    } else {
        seen = case.flag_state().resolve(ir);
        &seen
    };
    let decl = ir
        .decls
        .iter()
        .find(|d| d.name == case.ty || d.qualified == case.ty)
        .ok_or_else(|| Mismatch::UnknownType {
            case: case.name.clone(),
            ty: case.ty.clone(),
        })?;
    let options = Options {
        request: case.request,
        partial: case.partial,
    };
    let mut found =
        check_value_audited(ir, decl, &case.value, options).map_err(|error| Mismatch::Input {
            case: case.name.clone(),
            error,
        })?;
    if case.output_is_off() {
        // Output mode `off` evaluates no output rule, so nothing is found and nothing reported.
        found.violations.clear();
        found.audited.clear();
    }
    let keys = |vs: Vec<crate::check::Violation>| -> BTreeSet<Key> {
        vs.into_iter()
            .map(|v| Key {
                rule: v.rule_name,
                path: v.path,
                fields: v.fields,
                error: v.error.map(|e| e.as_str().to_owned()),
            })
            .collect()
    };
    diff(case, keys(found.violations))?;
    diff_audited(case, keys(found.audited))?;
    check_computed(ir, decl, case, options)
}

/// Compares the values at the paths of `computed` with the value completed by the evaluator.
fn check_computed(
    ir: &Ir,
    decl: &veld_ir::Decl,
    case: &Case,
    options: Options,
) -> Result<(), Mismatch> {
    if case.computed.is_empty() {
        return Ok(());
    }
    let full = with_defaults(ir, decl, &case.value, options).map_err(|error| Mismatch::Input {
        case: case.name.clone(),
        error,
    })?;
    for (path, want) in &case.computed {
        let got = value_at(&full, path).unwrap_or(&Json::Null);
        if got != want {
            return Err(Mismatch::Computed {
                case: case.name.clone(),
                path: path.clone(),
                expected: want.to_string(),
                found: got.to_string(),
            });
        }
    }
    Ok(())
}

/// The value at `$`, `$.name`, `$.items[1].line`: the path syntax of a violation. A name with a
/// dot in it is not supported.
#[must_use]
pub fn value_at<'a>(root: &'a Json, path: &str) -> Option<&'a Json> {
    let mut cur = root;
    let mut rest = path.strip_prefix('$')?;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('[') {
            let (index, tail) = after.split_once(']')?;
            cur = cur.get(index.parse::<usize>().ok()?)?;
            rest = tail;
        } else {
            let after = rest.strip_prefix('.')?;
            let end = after.find(['.', '[']).unwrap_or(after.len());
            cur = cur.get(&after[..end])?;
            rest = &after[end..];
        }
    }
    Some(cur)
}

/// A violation as generated code reported it: the JSON object of `spec/rules.md` section 7.
/// The per-language vector harnesses deserialise their driver's output into this.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Produced {
    /// The rule id, `veld:<package>/<Decl>.<rule>@<hex16>`.
    pub rule: String,
    pub path: String,
    pub fields: Vec<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// The rule name inside a rule id: `veld:t/AddInt.small@91c0…` gives `small`.
#[must_use]
pub fn rule_name_of(id: &str) -> Option<&str> {
    let (head, _hash) = id.rsplit_once('@')?;
    let (_decl, name) = head.rsplit_once('.')?;
    Some(name)
}

/// Compares what generated code produced for `case` with the case's `expect`, as a set. Rule
/// ids are reduced to rule names first, because vectors name rules, not ids.
///
/// # Errors
/// A [`Mismatch`] that says what differs. A rule id that has no name in it is reported as an
/// unexpected violation under the whole id.
pub fn compare_produced(case: &Case, produced: &[Produced]) -> Result<(), Mismatch> {
    diff(case, produced_keys(produced))
}

/// Compares the audited violations generated code reported for `case` with the case's `audited`,
/// as a set (`spec/rules.md` section 17.1).
///
/// # Errors
/// A [`Mismatch::AuditedDiffers`] that says what differs.
pub fn compare_produced_audited(case: &Case, produced: &[Produced]) -> Result<(), Mismatch> {
    diff_audited(case, produced_keys(produced))
}

fn produced_keys(produced: &[Produced]) -> BTreeSet<Key> {
    produced
        .iter()
        .map(|p| Key {
            rule: rule_name_of(&p.rule).unwrap_or(&p.rule).to_owned(),
            path: p.path.clone(),
            fields: p.fields.clone(),
            error: p.error.clone(),
        })
        .collect()
}

fn expected_keys(list: &[Expected]) -> BTreeSet<Key> {
    list.iter()
        .map(|e| Key {
            rule: e.rule.clone(),
            path: e.path.clone(),
            fields: e.fields.clone(),
            error: e.error.clone(),
        })
        .collect()
}

fn diff(case: &Case, got: BTreeSet<Key>) -> Result<(), Mismatch> {
    let want = expected_keys(&case.expect);
    if got == want {
        return Ok(());
    }
    Err(Mismatch::Differs {
        case: case.name.clone(),
        missing: want.difference(&got).cloned().collect(),
        unexpected: got.difference(&want).cloned().collect(),
    })
}

fn diff_audited(case: &Case, got: BTreeSet<Key>) -> Result<(), Mismatch> {
    let want = expected_keys(&case.audited);
    if got == want {
        return Ok(());
    }
    Err(Mismatch::AuditedDiffers {
        case: case.name.clone(),
        missing: want.difference(&got).cloned().collect(),
        unexpected: got.difference(&want).cloned().collect(),
    })
}
