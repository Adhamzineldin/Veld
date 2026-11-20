//! Veld generator framework: the pure (no I/O) layer between the IR and the
//! file writer. Dependency direction: `veld-project` (I/O) -> `veld-codegen`
//! (pure) -> `veld-ir`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod canonical;
pub mod fileset;
pub mod generator;
pub mod naming;
pub mod writer;

pub use fileset::{FileSet, FileSetError, PathError};
pub use generator::{
    Capabilities, DynGenerator, GenDiagnostic, GenRequest, Generator, Role, check_capabilities,
};
pub use writer::CodeWriter;
