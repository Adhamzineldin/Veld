//! Rule IR: the typed expression tree that a model or error declaration carries.
//!
//! The JSON forms are normative and live in `spec/rules.md` section 13. Every enum here uses
//! serde's default (externally tagged) form unless noted, so a unit variant is a string and a
//! variant with a payload is a one-key object. Nothing here is `#[non_exhaustive]`: consumers
//! match every variant, so adding one is a compile error until each consumer decides.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{DeclId, Origin, Type};

/// A boolean rule declared on a model or error (`rule name: expr`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub name: String,
    /// Stable rule identity (`spec/rules.md` section 6).
    pub id: String,
    pub expr: Expr,
    pub message: Option<String>,
    /// Source names of the fields of this declaration that the rule reads, first use first.
    pub reads: Vec<String>,
    /// True when one of the fields read is `@readonly`.
    pub reads_readonly: bool,
    pub inherited_from: Option<DeclId>,
    pub docs: Option<String>,
    pub deprecated: Option<String>,
    pub origin: Origin,
}

/// A typed expression node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expr {
    pub t: ExprType,
    pub e: ExprKind,
}

/// The type of an expression: an ordinary IR type, or the expression-only `duration`.
#[derive(Debug, Clone, PartialEq)]
pub enum ExprType {
    Type(Type),
    Duration,
}

#[derive(Deserialize)]
enum DurationName {
    #[serde(rename = "duration")]
    Duration,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ExprTypeRepr {
    Type(Type),
    Duration(DurationName),
}

impl Serialize for ExprType {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            ExprType::Type(t) => t.serialize(s),
            ExprType::Duration => s.serialize_str("duration"),
        }
    }
}

impl<'de> Deserialize<'de> for ExprType {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match ExprTypeRepr::deserialize(d)? {
            ExprTypeRepr::Type(t) => ExprType::Type(t),
            ExprTypeRepr::Duration(DurationName::Duration) => ExprType::Duration,
        })
    }
}

/// The JSON `true` of `{ "now": true }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Always;

impl Serialize for Always {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bool(true)
    }
}

impl<'de> Deserialize<'de> for Always {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        if bool::deserialize(d)? {
            Ok(Always)
        } else {
            Err(D::Error::custom("expected `true`"))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExprKind {
    Lit(ExprLit),
    Path(PathRef),
    Now(Always),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Bin(Box<BinExpr>),
    Has(PathRef),
    Call(CallExpr),
    Quant(Box<QuantExpr>),
    List(Vec<Expr>),
    /// Exact conversion of the operand to this node's type (`spec/rules.md` section 3.3).
    Widen(Box<Expr>),
    /// The value before the operation (Phase 2).
    Old(Box<Expr>),
}

/// A literal. A `long` is a decimal string in JSON so it stays exact; a `decimal` keeps its
/// source text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExprLit {
    Bool(bool),
    Int(i64),
    Long(#[serde(with = "long_text")] i64),
    Float(f64),
    Decimal(String),
    String(String),
    Enum(EnumLit),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumLit {
    pub decl: DeclId,
    pub name: String,
    pub wire: String,
}

mod long_text {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathRef {
    pub root: Root,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    pub wire: String,
    pub optional: bool,
}

/// What a path starts from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Root {
    #[serde(rename = "self")]
    SelfValue,
    #[serde(rename = "principal")]
    Principal,
    #[serde(rename = "result")]
    Result,
    #[serde(rename = "var")]
    Var(String),
    #[serde(rename = "param")]
    Param(String),
    #[serde(rename = "fact")]
    Fact(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BinExpr {
    pub op: Op,
    pub l: Expr,
    pub r: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    And,
    Or,
    Implies,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    In,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallExpr {
    pub f: Builtin,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Builtin {
    Len,
    Matches,
    StartsWith,
    EndsWith,
    Contains,
    Days,
    Hours,
    Minutes,
    Seconds,
    Ms,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuantExpr {
    pub q: Quantifier,
    pub list: Expr,
    pub var: String,
    pub body: Expr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quantifier {
    All,
    Any,
    Count,
    Sum,
}

#[cfg(test)]
mod tests;
