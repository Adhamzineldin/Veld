//! Endpoint rules and examples (`spec/rules.md` sections 9, 12 and 13.1).
//!
//! These types are plumbing: the IR carries what the compiler worked out, and nothing here
//! evaluates anything. Everything is omitted from the JSON of an IR that has no rules or examples
//! (see the `skip_serializing_if` attributes on `Endpoint`, `Service` and `Ir`), so the IR of an
//! existing contract is byte-identical.

use serde::{Deserialize, Serialize};

use crate::rules::{Expr, Step};
use crate::{DeclId, Origin, PackageId, ServiceId, Type};

/// The statements of an endpoint block.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EndpointRules {
    /// In load order.
    pub facts: Vec<EndpointFact>,
    /// In source order.
    pub requires: Vec<EndpointRequire>,
    /// In source order.
    pub ensures: Vec<EndpointEnsure>,
    /// The state transition guard (`spec/rules.md` section 10.2), last as in section 10.4. Omitted
    /// from the JSON when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition: Option<EndpointTransition>,
}

impl EndpointRules {
    /// True when the endpoint has no statement at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
            && self.transition.is_none()
            && self.requires.is_empty()
            && self.ensures.is_empty()
    }

    /// The origin of the first statement: facts, then requires, then ensures.
    #[must_use]
    pub fn first_origin(&self) -> Option<&Origin> {
        self.facts
            .first()
            .map(|f| &f.origin)
            .or_else(|| self.transition.as_ref().map(|t| &t.origin))
            .or_else(|| self.requires.first().map(|r| &r.origin))
            .or_else(|| self.ensures.first().map(|e| &e.origin))
    }
}

/// `transition fact.path: a, b -> c` (`spec/rules.md` section 10.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndpointTransition {
    /// Stable identity (`spec/rules.md` section 10.2).
    pub id: String,
    /// The fact the path starts at.
    pub fact: String,
    /// The path below the fact; the last step is the state field.
    pub steps: Vec<Step>,
    /// The state machine.
    pub decl: DeclId,
    /// Variant names of the sources, in source order.
    pub from: Vec<String>,
    /// Variant name of the target.
    pub to: String,
    pub origin: Origin,
}

/// `fact name: T = key`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndpointFact {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    pub optional: bool,
    pub key: Option<Expr>,
    pub origin: Origin,
}

/// `require [name:] expr [else Error { .. }] [message ".."]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndpointRequire {
    pub name: Option<String>,
    /// Stable rule identity (`spec/rules.md` section 9.6).
    pub id: String,
    pub expr: Expr,
    pub message: Option<String>,
    /// Names of the parameters the expression reads, first use first.
    pub reads: Vec<String>,
    #[serde(rename = "else")]
    pub else_error: Option<ElseError>,
    pub docs: Option<String>,
    pub origin: Origin,
}

/// The error a failed `require` answers with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElseError {
    pub decl: DeclId,
    pub status: u16,
    pub fields: Vec<ElseField>,
}

/// One field of the payload of an `else` error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElseField {
    pub name: String,
    pub wire: String,
    pub value: Expr,
}

/// `ensure [name:] expr [message ".."]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndpointEnsure {
    pub name: Option<String>,
    pub id: String,
    pub expr: Expr,
    pub message: Option<String>,
    pub reads: Vec<String>,
    pub docs: Option<String>,
    pub origin: Origin,
}

/// One loader method of a service: the union of the facts of its endpoints, by name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceFact {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    pub key: Option<Type>,
    pub optional: bool,
}

/// `example "name" { .. }`: what one call does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Example {
    pub name: String,
    pub package: PackageId,
    pub service: ServiceId,
    pub endpoint: String,
    pub givens: Vec<ExampleGiven>,
    pub principal: Option<ExampleValue>,
    /// The clock, as written.
    pub now: Option<String>,
    pub args: Vec<ExampleArg>,
    pub returns: Option<ExampleValue>,
    pub outcome: ExampleOutcome,
    pub docs: Option<String>,
    pub origin: Origin,
}

/// `given fact = value`; `None` is `absent`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExampleGiven {
    pub fact: String,
    pub value: Option<ExampleValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExampleArg {
    pub param: String,
    pub value: ExampleValue,
}

/// `ok [value]` or an error name with an optional pattern.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExampleOutcome {
    Ok(Option<ExampleValue>),
    Error(ExampleErrorOutcome),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExampleErrorOutcome {
    pub decl: DeclId,
    pub pattern: Option<ExampleValue>,
}

/// A value written in an example.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExampleValue {
    /// A literal, already typed: a wire value.
    Json(serde_json::Value),
    /// A field read from a given, by wire names.
    Ref(ExampleRef),
    Object(Vec<ExampleField>),
    List(Vec<ExampleValue>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExampleRef {
    pub given: String,
    pub path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExampleField {
    pub name: String,
    pub wire: String,
    pub value: ExampleValue,
}

#[cfg(test)]
pub(crate) mod tests;
