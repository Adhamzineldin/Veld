//! The reference evaluator for `spec/rules.md`.
//!
//! It evaluates the typed expressions of `veld_ir::Rule` with the exact value semantics of the
//! spec, and the shared test vectors in `tests/rules/` run against it ([`vectors`]). Generated
//! code never links to this crate; it is the oracle that the generated checks are compared
//! with.
//!
//! * [`check_value`] checks a wire value against its declaration, recursively (sections 5.2,
//!   5.4 and 7).
//! * [`eval`] evaluates one expression.
//! * [`helpers`] holds the dependency-free value helpers that the Rust generator embeds.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod builtins;
mod check;
mod error;
mod eval;
pub mod helpers;
mod ops;
mod value;
pub mod vectors;

#[cfg(test)]
mod tests;

pub use check::{Options, Violation, check_value};
pub use error::{Failure, InputError};
pub use eval::{Env, eval};
pub use helpers::num::EvalError;
pub use value::Value;

/// The IR version whose rules this crate evaluates.
pub const IR_VERSION: &str = veld_ir::IR_VERSION;
