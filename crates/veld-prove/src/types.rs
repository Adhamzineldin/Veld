//! The vocabulary shared by the encoder, the runner and the confirmation step.

use veld_ir::{Constraints, Root, Type};

/// The SMT sort a Veld value is modelled with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Bool,
    Int,
    Real,
    /// Strings and `uuid`s: an uninterpreted sort, so only equality is known.
    Str,
    Uuid,
}

impl Sort {
    /// The sort name in SMT-LIB.
    #[must_use]
    pub fn smt(self) -> &'static str {
        match self {
            Sort::Bool => "Bool",
            Sort::Int => "Int",
            Sort::Real => "Real",
            Sort::Str => "Str",
            Sort::Uuid => "Uuid",
        }
    }
}

/// What a path of a rule starts from.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RootKey {
    SelfValue,
    Param(String),
    Fact(String),
    Principal,
    Result,
}

impl RootKey {
    /// The root of an IR path; a lambda variable has none.
    #[must_use]
    pub fn of(root: &Root) -> Option<RootKey> {
        match root {
            Root::SelfValue => Some(RootKey::SelfValue),
            Root::Param(n) => Some(RootKey::Param(n.clone())),
            Root::Fact(n) => Some(RootKey::Fact(n.clone())),
            Root::Principal => Some(RootKey::Principal),
            Root::Result => Some(RootKey::Result),
            Root::Var(_) => None,
        }
    }

    /// The prefix of the SMT names of the values under this root.
    #[must_use]
    pub fn prefix(&self) -> String {
        match self {
            RootKey::SelfValue => "self".to_owned(),
            RootKey::Param(n) => format!("param.{n}"),
            RootKey::Fact(n) => format!("fact.{n}"),
            RootKey::Principal => "principal".to_owned(),
            RootKey::Result => "result".to_owned(),
        }
    }
}

/// What the encoder knows about one root.
#[derive(Debug, Clone)]
pub struct RootSpec {
    /// The type of the value.
    pub ty: Type,
    /// The root itself may be absent (an optional parameter or fact).
    pub optional: bool,
    /// The constraints of the root itself (of a scalar parameter).
    pub constraints: Constraints,
    /// The value is validated before the rules run, so its constraints are assumed. Facts and
    /// the principal are trusted values of the application and are not (`spec/rules.md` 9.2).
    pub validated: bool,
}

impl RootSpec {
    /// A validated root of type `ty`.
    #[must_use]
    pub fn validated(ty: Type) -> RootSpec {
        RootSpec {
            ty,
            optional: false,
            constraints: Constraints::default(),
            validated: true,
        }
    }

    /// A trusted root of type `ty`: only the domain of its type is assumed.
    #[must_use]
    pub fn trusted(ty: Type) -> RootSpec {
        RootSpec {
            validated: false,
            ..RootSpec::validated(ty)
        }
    }
}

/// A part of the fragment that is left open (`spec/rules.md` section 18.3): the encoding uses an
/// unconstrained value of the right sort instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OpenPart {
    Float,
    StringContent,
    Matches,
    Comprehension,
    Sum,
    Now,
    Membership,
    PatternFormat,
    /// A derived field (`spec/rules.md` section 17.2): the server computes it from the other
    /// fields, and the encoding lets the solver pick any value of its type.
    Derived,
}

impl OpenPart {
    /// The name used in the report.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            OpenPart::Float => "float",
            OpenPart::StringContent => "string content",
            OpenPart::Matches => "matches",
            OpenPart::Comprehension => "comprehension",
            OpenPart::Sum => "sum",
            OpenPart::Now => "now",
            OpenPart::Membership => "membership in a list or map",
            OpenPart::PatternFormat => "pattern and format of a string",
            OpenPart::Derived => "derived field",
        }
    }
}

/// What a solver variable stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarKind {
    /// The value itself.
    Value,
    /// The Boolean beside an optional value: it is present.
    Has,
    /// The length of a list, map or `bytes`.
    Len,
}

/// One solver variable that stands for a part of an input value.
#[derive(Debug, Clone)]
pub struct VarInfo {
    /// The SMT name, without bars.
    pub name: String,
    pub sort: Sort,
    pub root: RootKey,
    /// The wire names from the root down to the part.
    pub path: Vec<String>,
    pub kind: VarKind,
    /// The type of the part (of the list, for a length).
    pub ty: Option<Type>,
    /// The wire names of the variants when the part is an enum, in the order of its integer
    /// model; empty otherwise.
    pub variants: Vec<String>,
}

/// Why a question cannot be encoded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EncodeError {
    #[error("{0}")]
    Unsupported(String),
}

pub(crate) fn unsupported<T>(what: impl Into<String>) -> Result<T, EncodeError> {
    Err(EncodeError::Unsupported(what.into()))
}
