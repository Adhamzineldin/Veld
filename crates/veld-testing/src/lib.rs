//! Runs the `example` blocks of a contract (`spec/rules.md` section 12).
//!
//! * [`run_offline`] runs examples against the reference evaluator, no server needed.
//! * [`Live`] sends them to a running **test handler** and compares its answers. The request
//!   of an example is built by [`build_request`] and the answer is compared by [`judge`], both
//!   plain functions, so a request generator (fuzzing, boundaries) can reuse them.
//!
//! Both modes produce the same [`veld_rules::ExampleReport`].

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod b64;
mod error;
mod judge;
mod live;
mod offline;
mod request;

pub use error::LiveError;
pub use judge::{Answer, judge};
pub use live::Live;
pub use offline::{ExampleRun, run_examples, run_offline, select};
pub use request::{
    DRY_RUN, FACTS, HttpRequest, NOW, PASSED, PRINCIPAL, PROBE_PATH, RESULT, build_request,
};
