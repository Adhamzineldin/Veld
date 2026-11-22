//! Veld project support: `veld.toml` loading and the safe, staged output writer.
//!
//! Generators depend on this crate only for [`FileSet`]; the CLI uses
//! [`config`] to resolve a project and [`output`] to write generated files
//! without ever destroying user data.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod config;
pub mod fileset;
pub mod output;
pub mod packages;

pub use fileset::{FileSet, FileSetError, PathError};
