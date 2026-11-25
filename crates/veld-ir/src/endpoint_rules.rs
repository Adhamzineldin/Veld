//! Endpoint rules and examples (`spec/rules.md` sections 9, 12 and 13.1).
//!
//! These types are plumbing: the IR carries what the compiler worked out, and nothing here
//! evaluates anything. Everything is omitted from the JSON of an IR that has no rules or examples
//! (see the `skip_serializing_if` attributes on `Endpoint`, `Service` and `Ir`), so the IR of an
//! existing contract is byte-identical.

use serde::{Deserialize, Serialize};

use crate::rules::{Expr, Step};
use crate::{ChannelId, DeclId, Origin, PackageId, PolicyId, ServiceId, Type};

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
    /// The `allow` lines in source order (`spec/rules.md` section 11.3). Omitted from the JSON
    /// when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allows: Vec<EndpointAllow>,
}

impl EndpointRules {
    /// True when the endpoint has no statement at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
            && self.transition.is_none()
            && self.allows.is_empty()
            && self.requires.is_empty()
            && self.ensures.is_empty()
    }

    /// The origin of the first statement: facts, then requires, then ensures.
    #[must_use]
    pub fn first_origin(&self) -> Option<&Origin> {
        self.facts
            .first()
            .map(|f| &f.origin)
            .or_else(|| self.allows.first().map(|a| &a.origin))
            .or_else(|| self.transition.as_ref().map(|t| &t.origin))
            .or_else(|| self.requires.first().map(|r| &r.origin))
            .or_else(|| self.ensures.first().map(|e| &e.origin))
    }
    /// The origin of the first statement that needs `rules_policies`: the first `allow`, else
    /// the first fact key, `require`, payload or `ensure` that reads the principal (a policy is
    /// not needed to read the caller, but a generator that cannot represent the principal must
    /// refuse it all the same).
    #[must_use]
    pub fn first_policy_origin(&self) -> Option<&Origin> {
        let key = |f: &&EndpointFact| f.key.as_ref().is_some_and(Expr::reads_principal);
        let payload = |r: &&EndpointRequire| {
            r.expr.reads_principal()
                || r.else_error
                    .iter()
                    .flat_map(|e| &e.fields)
                    .any(|f| f.value.reads_principal())
        };
        self.allows
            .first()
            .map(|a| &a.origin)
            .or_else(|| self.facts.iter().find(key).map(|f| &f.origin))
            .or_else(|| self.requires.iter().find(payload).map(|r| &r.origin))
            .or_else(|| {
                self.ensures
                    .iter()
                    .find(|e| e.expr.reads_principal())
                    .map(|e| &e.origin)
            })
    }
}

/// `allow Policy(args)` (`spec/rules.md` section 11.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndpointAllow {
    /// Index into `Ir.policies`.
    pub policy: PolicyId,
    /// One expression per parameter of the policy, in the endpoint's scope.
    pub args: Vec<Expr>,
    /// Names of the parameters the arguments read, first use first.
    pub reads: Vec<String>,
    pub origin: Origin,
}

/// A top-level `policy` (`spec/rules.md` section 11.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    pub name: String,
    pub package: PackageId,
    pub module: String,
    pub qualified: String,
    /// Stable identity (`spec/rules.md` section 11.2).
    pub id: String,
    pub params: Vec<PolicyParam>,
    pub clauses: Vec<PolicyClause>,
    pub docs: Option<String>,
    pub origin: Origin,
}

/// One typed parameter of a policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyParam {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
}

/// `permit when expr` or `forbid when expr`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyClause {
    pub effect: PolicyEffect,
    pub expr: Expr,
    pub message: Option<String>,
    pub origin: Origin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyEffect {
    Permit,
    Forbid,
}

/// The package's principal, on each of its services (`spec/rules.md` section 11.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServicePrincipal {
    /// The model.
    pub decl: DeclId,
    /// The name it has in endpoint blocks.
    pub name: String,
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
///
/// An example is either a call (`service`, `endpoint`, `outcome` and the rest) or, when `event`
/// is set, one delivery of an event (`spec/events.md` section 12): then `service` and `outcome`
/// are `None`, `endpoint` is empty and `expect` is set. Those three keys are omitted from the
/// JSON of an event example, and `event` and `expect` from the JSON of a call example.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Example {
    pub name: String,
    pub package: PackageId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<ServiceId>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub endpoint: String,
    pub givens: Vec<ExampleGiven>,
    pub principal: Option<ExampleValue>,
    /// The clock, as written.
    pub now: Option<String>,
    pub args: Vec<ExampleArg>,
    pub returns: Option<ExampleValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<ExampleOutcome>,
    pub docs: Option<String>,
    /// The event form: `given event E { .. } [on C]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<ExampleEvent>,
    /// The `expect` lines of an event example.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect: Option<EventExpect>,
    pub origin: Origin,
}

/// `given event E { .. } on C`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExampleEvent {
    pub decl: DeclId,
    /// Index into `Ir.channels`.
    pub channel: ChannelId,
    pub value: ExampleValue,
}

/// What an event example expects: `"rejected"` or a list of `Service.handler` lines.
#[derive(Debug, Clone, PartialEq)]
pub enum EventExpect {
    Rejected,
    Calls(Vec<ExpectedCall>),
}

/// `expect S.handler [not] called`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedCall {
    pub service: ServiceId,
    pub handler: String,
    pub called: bool,
}

impl Serialize for EventExpect {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            EventExpect::Rejected => s.serialize_str("rejected"),
            EventExpect::Calls(c) => c.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for EventExpect {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Word(String),
            Calls(Vec<ExpectedCall>),
        }
        match Raw::deserialize(d)? {
            Raw::Calls(c) => Ok(EventExpect::Calls(c)),
            Raw::Word(w) if w == "rejected" => Ok(EventExpect::Rejected),
            Raw::Word(w) => Err(serde::de::Error::custom(format!(
                "expected \"rejected\" or a list, found \"{w}\""
            ))),
        }
    }
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
