//! Sagas (`spec/platform.md` sections 17 and 18): an endpoint whose implementation is a list of
//! calls to other endpoints, each with an optional call that undoes it.
//!
//! A step is read in an expression as a root of its own kind of name: `Root::Fact(<step name>)`,
//! typed as the success type of the step's endpoint. The parameters of the saga are
//! `Root::Param`. Every key here is omitted from the JSON when absent, so the IR of a contract
//! without a saga is unchanged.

use serde::{Deserialize, Serialize};

use crate::{Expr, Origin, ServiceId};

/// The saga of an endpoint (`Endpoint::saga`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Saga {
    /// The steps, in the order they run.
    pub steps: Vec<SagaStep>,
    /// The time from the start within which the last step must have answered, in milliseconds,
    /// as a decimal string (a `long`).
    #[serde(with = "ms_text")]
    pub timeout_ms: i64,
}

/// `step name = call [compensate call]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SagaStep {
    pub name: String,
    pub call: SagaCall,
    /// The call that undoes the step. Omitted from the JSON when the step has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compensate: Option<SagaCall>,
    pub origin: Origin,
}

/// `Service.endpoint(name: value, ..)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SagaCall {
    pub service: ServiceId,
    pub endpoint: String,
    /// The arguments, in the order of the parameters of the endpoint called. A parameter that is
    /// optional and not given is not in the list.
    pub args: Vec<SagaArg>,
}

/// One argument: a parameter of the endpoint called, and its value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SagaArg {
    /// The parameter, by its source name.
    pub name: String,
    pub value: SagaValue,
}

/// The value of an argument: an expression (`{ "t": .., "e": .. }`), or a model built from
/// expressions (`{ "object": [..] }`, `spec/platform.md` section 18).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SagaValue {
    Expr(Expr),
    Object {
        /// The fields that are given, in source order. A field that is optional and not given is
        /// not in the list.
        object: Vec<SagaField>,
    },
}

/// One field of an object value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SagaField {
    pub name: String,
    pub wire: String,
    pub value: SagaValue,
}

impl SagaValue {
    /// Every expression of the value, in source order.
    #[must_use]
    pub fn exprs(&self) -> Vec<&Expr> {
        match self {
            SagaValue::Expr(e) => vec![e],
            SagaValue::Object { object } => object.iter().flat_map(|f| f.value.exprs()).collect(),
        }
    }
}

impl Saga {
    /// Every call of the saga: the steps' calls and their compensations, in step order, a step
    /// before its own compensation.
    pub fn calls(&self) -> impl Iterator<Item = &SagaCall> {
        self.steps
            .iter()
            .flat_map(|s| std::iter::once(&s.call).chain(s.compensate.as_ref()))
    }
}

/// What the example of a saga states (`spec/platform.md` section 18): what each call answers and
/// what must have happened. Omitted from the JSON of an example that is not a saga example.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ExampleSaga {
    /// `given step s = ..`, in source order. A step that is not named answers a filled-in success.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<SagaGiven>,
    /// `given undo s = ..`, in source order. An undo that is not named answers `ok`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub undos: Vec<SagaGiven>,
    /// `expect compensated a, b` or `expect stuck`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect: Option<SagaExpect>,
}

/// What a call answers in a saga example.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SagaGiven {
    /// The step.
    pub step: String,
    pub answer: SagaAnswer,
}

/// `<value | Error { .. } | unavailable>` for a step, `<ok | Error { .. } | unavailable>` for an
/// undo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SagaAnswer {
    /// The call never answers.
    Unavailable,
    /// The call answers: a success (with a value, for a step), or a declared error of the
    /// endpoint called.
    Answer(crate::ExampleOutcome),
}

/// `expect compensated a, b` or `expect stuck`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SagaExpect {
    /// The steps undone, in the order their undo was called.
    Compensated(Vec<String>),
    /// The saga ended stuck.
    Stuck,
}

mod ms_text {
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &i64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
        let text = String::deserialize(d)?;
        text.parse().map_err(D::Error::custom)
    }
}

#[cfg(test)]
pub(crate) mod tests;
