//! The Veld plugin host: runs emitter plugins as child processes over JSON lines
//! (`spec/plugin-protocol.md`) and presents them as ordinary [`veld_codegen::DynGenerator`]s,
//! so plugin targets go through the same capability check and staged writer as built-ins.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod command;
pub mod conformance;
pub mod error;
pub mod generator;
pub mod protocol;
pub mod session;

pub use command::CommandLine;
pub use error::PluginError;
pub use generator::{HostConfig, PluginGenerator, Runner};
