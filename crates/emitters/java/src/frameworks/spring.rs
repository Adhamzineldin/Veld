//! The Spring Boot 3 (Jakarta) adapter, the only generated file with a dependency.

use veld_codegen::{FileSet, FrameworkAdapter, GenDiagnostic, LanguageVersion, Profile};

use super::{ServerModel, class_file};

const SOURCE: &str = include_str!("../../adapters/SpringAdapter.java");

/// The `spring` framework.
#[derive(Debug, Clone, Copy)]
pub struct Spring;

impl FrameworkAdapter<ServerModel> for Spring {
    fn id(&self) -> &'static str {
        "spring"
    }

    /// Spring Boot 3 is the Jakarta line and the only one the adapter targets.
    fn framework_versions(&self) -> &'static [&'static str] {
        &["3"]
    }

    /// Spring Boot 3 needs Java 17.
    fn min_language(&self) -> LanguageVersion {
        LanguageVersion::new(17, 0)
    }

    fn files(
        &self,
        model: &ServerModel,
        _profile: &Profile,
    ) -> Result<FileSet, Vec<GenDiagnostic>> {
        class_file(model, "SpringAdapter", SOURCE)
    }
}
