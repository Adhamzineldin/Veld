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
pub mod example;
pub mod helpers;
pub mod machine;
mod old;
mod ops;
mod pipeline;
pub mod policy;
mod value;
pub mod vectors;
mod wire;

#[cfg(test)]
mod tests;

pub use check::{Options, Violation, check_value};
pub use error::{Failure, InputError};
pub use eval::{Env, SavedOld, eval};
pub use example::{
    DEFAULT_CLOCK, Delivery, Difference, ExampleError, ExampleReport, Expected, FilledEvent,
    FilledExample, Mismatch, RejectReason, Verdict, deliver, describe_actual, describe_delivery,
    describe_expectation, describe_expected, diff_pattern, fill_event_example, fill_example,
    run_event_example, run_example,
};
pub use helpers::num::EvalError;
pub use machine::{is_final, next_states};
pub use old::save_old;
pub use pipeline::{Before, ErrorAnswer, Outcome, Pipeline, Prepared, Request, RunError};
pub use policy::{NOT_PERMITTED, PolicyVerdict, eval_policy};
pub use value::Value;
pub use wire::{format_date, format_datetime, format_time};

/// The IR version whose rules this crate evaluates.
pub const IR_VERSION: &str = veld_ir::IR_VERSION;
