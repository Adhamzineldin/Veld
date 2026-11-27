//! `veld check --prove` and `veld diff --prove` (`spec/rules.md` section 18).
//!
//! The crate turns a question about the rules of a contract into SMT-LIB 2 text ([`Problem`]),
//! runs an external solver on it ([`solver`]), reads its answer, and shows a value only after the
//! reference evaluator of `veld-rules` has confirmed it. It links no solver and knows no syntax:
//! it reads the IR and nothing else.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::wildcard_enum_match_arm
    )
)]

pub mod confirm;
pub mod diff;
pub mod encode;
pub mod problem;
pub mod question;
pub mod report;
pub mod run;
pub mod sample;
pub mod sexp;
pub mod smt;
pub mod solver;
pub mod sorts;
pub mod types;
pub mod values;

#[cfg(test)]
mod tests;

pub use diff::Changed;
pub use problem::{Literal, Problem, ValueKey};
pub use report::{Counts, Item, Outcome, QuestionKind, Report};
pub use run::{ChangedRule, Options, ask, check, diff as prove_diff};
pub use solver::{Backend, Flavour, NotFound, Solver, SolverInfo, find, find_in_environment};
pub use types::{EncodeError, OpenPart, RootKey, RootSpec, Sort, VarInfo, VarKind};
