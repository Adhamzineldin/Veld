//! The Veld language server: one synchronous engine over stdio.
//!
//! The whole project is re-analysed on every change; contracts are small. Open buffers
//! override disk contents.

// `lsp_types::Uri` caches a parsed part lazily, which clippy sees as interior mutability; its
// ordering and hashing never change, and lsp-types itself uses it as a map key.
#![allow(clippy::mutable_key_type)]

pub mod complete;
mod complete_events;
mod complete_rules;
pub mod context;
pub mod diag;
pub mod hover;
mod hover_events;
mod hover_rules;
pub mod nav;
pub mod outline;
pub mod query;
pub mod rfind;
#[cfg(test)]
mod rules_tests;
mod scan;
mod scope;
pub mod server;
pub mod text;
pub mod uri;
pub mod words;
pub mod workspace;

pub use server::{ServerError, capabilities, run_stdio, serve};

#[cfg(test)]
mod event_tests;
#[cfg(test)]
mod testutil;
