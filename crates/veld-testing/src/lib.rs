//! Runs the `example` blocks of a contract (`spec/rules.md` section 12).
//!
//! * [`run_offline`] runs examples against the reference evaluator, no server needed.
//! * [`Live`] sends them to a running **test handler** and compares its answers. The request
//!   of an example is built by [`build_request`] and the answer is compared by [`judge`], both
//!   plain functions, so a request generator (fuzzing, boundaries) can reuse them.
//!
//! Both modes produce the same [`veld_rules::ExampleReport`].

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod audited;
pub mod b64;
pub mod conditional;
pub mod coverage;
pub mod emitted;
mod error;
mod events;
mod fuzz;
mod judge;
mod live;
mod offline;
mod outcome;
mod request;
mod violation;

pub use error::LiveError;
pub use events::{DELIVERIES, EVENT_PATH, EventAnswer, KEY, TOPIC};
pub use fuzz::{
    EndpointReport, FuzzError, FuzzMismatch, FuzzOptions, FuzzReport, GenerateError, Generated,
    fuzz, generate, render_example,
};
pub use judge::{Answer, judge};
pub use live::Live;
pub use offline::{ExampleRun, run_examples, run_offline, select};
pub use outcome::{Called, describe_answer, judge_outcome};
pub use request::{
    DRY_RUN, FACTS, HttpRequest, NOW, PASSED, PRINCIPAL, PROBE_PATH, RESULT, build_request,
};
pub use violation::{FromViolation, ViolationError, from_violation};
