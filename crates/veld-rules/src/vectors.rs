//! The shared test vectors of `spec/rules.md` section 15: loading `tests/rules/**/*.json` and
//! running a case against an IR.
//!
//! The harness that compiles the `.veld` files and the per-language harnesses reuse this
//! module: [`load_cases`] reads a vector file, [`run_case`] checks one case.

use std::collections::BTreeSet;
use std::fmt;

use serde::Deserialize;
use serde_json::Value as Json;
use veld_ir::Ir;

use crate::check::{Options, check_value};
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
    pub expect: Vec<Expected>,
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
}

/// Runs one case: finds the model by name, checks the value with the case's flags and compares
/// the violations with `expect` as a set.
///
/// # Errors
/// A [`Mismatch`] that says what differs.
pub fn run_case(ir: &Ir, case: &Case) -> Result<(), Mismatch> {
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
    let found = check_value(ir, decl, &case.value, options).map_err(|error| Mismatch::Input {
        case: case.name.clone(),
        error,
    })?;
    let got: BTreeSet<Key> = found
        .into_iter()
        .map(|v| Key {
            rule: v.rule_name,
            path: v.path,
            fields: v.fields,
            error: v.error.map(|e| e.as_str().to_owned()),
        })
        .collect();
    diff(case, got)
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
    let got: BTreeSet<Key> = produced
        .iter()
        .map(|p| Key {
            rule: rule_name_of(&p.rule).unwrap_or(&p.rule).to_owned(),
            path: p.path.clone(),
            fields: p.fields.clone(),
            error: p.error.clone(),
        })
        .collect();
    diff(case, got)
}

fn diff(case: &Case, got: BTreeSet<Key>) -> Result<(), Mismatch> {
    let want: BTreeSet<Key> = case
        .expect
        .iter()
        .map(|e| Key {
            rule: e.rule.clone(),
            path: e.path.clone(),
            fields: e.fields.clone(),
            error: e.error.clone(),
        })
        .collect();
    if got == want {
        return Ok(());
    }
    Err(Mismatch::Differs {
        case: case.name.clone(),
        missing: want.difference(&got).cloned().collect(),
        unexpected: got.difference(&want).cloned().collect(),
    })
}
