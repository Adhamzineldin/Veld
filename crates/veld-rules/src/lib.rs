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
pub mod coverage;
pub mod envelope;
mod error;
mod eval;
pub mod example;
pub mod flags;
pub mod helpers;
pub mod idempotency;
pub mod machine;
pub mod messages;
mod old;
mod ops;
mod pipeline;
pub mod policy;
pub mod redact;
pub mod saga;
mod value;
pub mod vectors;
pub mod version;
mod wire;

#[cfg(test)]
mod tests;

pub use check::{Checked, Options, Violation, check_value, check_value_audited, with_defaults};
pub use error::{Failure, InputError};
pub use eval::{Env, SavedOld, eval};
pub use example::{
    Completed, Completer, DEFAULT_CLOCK, Delivery, Difference, ExampleError, ExampleReport,
    Expected, FilledEvent, FilledExample, Mismatch, RejectReason, Verdict, deliver,
    describe_actual, describe_delivery, describe_expectation, describe_expected, diff_pattern,
    fill_event_example, fill_example, run_event_example, run_example,
};
pub use flags::{FlagState, flag_names};
pub use helpers::num::EvalError;
pub use idempotency::{
    IdempotencyKey, Scope, StoreEffect, StoreState, StoredAnswer, fingerprint, is_valid_key,
};
pub use machine::{TimedEndpoint, is_final, next_states, timed_endpoints, timed_moves};
pub use old::save_old;
pub use pipeline::{
    After, Before, ErrorAnswer, Outcome, OutputMode, Pipeline, Prepared, Reported, Request,
    RunError,
};
pub use policy::{NOT_PERMITTED, PolicyVerdict, eval_policy};
pub use redact::{Redacted, redact_decl, redact_decl_at, redact_type, redact_type_at};
pub use value::Value;
pub use version::{IfMatch, etag, parse_if_match};
pub use wire::{format_date, format_datetime, format_time};

/// The IR version whose rules this crate evaluates.
pub const IR_VERSION: &str = veld_ir::IR_VERSION;
