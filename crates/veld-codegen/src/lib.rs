//! Veld generator framework: the pure (no I/O) layer between the IR and the
//! file writer. Dependency direction: `veld-project` (I/O) -> `veld-codegen`
//! (pure) -> `veld-ir`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod canonical;
pub mod expr;
pub mod fileset;
pub mod gate;
pub mod generator;
pub mod lint;
pub mod naming;
pub mod profile;
pub mod runtime_config;
pub mod writer;

pub use expr::{ExprPrinter, Printed, ValueKind, element_type, print_expr};
pub use fileset::{FileSet, FileSetError, PathError};
pub use gate::{GateError, gate_regions};
pub use generator::resolve_profile;
pub use generator::{
    Capabilities, DynGenerator, GenDiagnostic, GenRequest, Generator, Role, check_capabilities,
    check_framework, checked_features,
};
pub use lint::LintLevel;
pub use profile::{
    FRAMEWORK_VERSION_KEY, FrameworkAdapter, FrameworkInfo, LanguageInfo, LanguageVersion, Offer,
    Profile, TARGET_NAME_KEY, framework_info, framework_infos, strip_profile_keys,
    with_target_name,
};
pub use writer::CodeWriter;
