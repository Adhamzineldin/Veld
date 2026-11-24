//! Runtime helpers with the value semantics of `spec/rules.md` section 3.4.
//!
//! Every file here depends on `std` only and never mentions the rest of the crate, because the
//! Rust code generator embeds these same files verbatim into generated crates.

pub mod decimal;
pub mod instant;
pub mod num;
pub mod text;

/// The dependency-free matcher of the generated Rust runtime, shared as it is.
#[rustfmt::skip]
#[path = "../../../emitters/rust/runtime/regex.rs"]
pub mod regex;
