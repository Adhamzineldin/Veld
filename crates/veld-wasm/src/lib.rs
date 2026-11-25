//! The real Veld engine as WebAssembly.
//!
//! [`api`] is the pure Rust core: it compiles contracts, runs the very same generators the CLI
//! runs, formats and exports OpenAPI, taking and returning plain data. [`wasm`] exposes it to
//! JavaScript with JSON strings in and out. Nothing here re-implements the language or an
//! emitter; the only glue is the target resolution that `apps/veld/src/pipeline.rs` also does,
//! and `tests/parity.rs` proves the two agree byte for byte.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod api;
#[cfg(test)]
mod event_tests;
pub mod registry;
mod wasm;
