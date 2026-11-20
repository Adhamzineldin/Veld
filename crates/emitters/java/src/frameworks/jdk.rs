//! The JDK `com.sun.net.httpserver` adapter: the zero-dependency baseline.

use veld_codegen::{FileSet, FrameworkAdapter, GenDiagnostic, LanguageVersion, Profile};

use super::{ServerModel, class_file};

pub const ID: &str = "jdk";
const SOURCE: &str = include_str!("../../adapters/JdkHttpServerAdapter.java");

/// The `jdk` framework.
#[derive(Debug, Clone, Copy)]
pub struct Jdk;

impl FrameworkAdapter<ServerModel> for Jdk {
    fn id(&self) -> &'static str {
        ID
    }

    fn framework_versions(&self) -> &'static [&'static str] {
        &[]
    }

    fn min_language(&self) -> LanguageVersion {
        LanguageVersion::new(17, 0)
    }

    fn files(
        &self,
        model: &ServerModel,
        _profile: &Profile,
    ) -> Result<FileSet, Vec<GenDiagnostic>> {
        class_file(model, "JdkHttpServerAdapter", SOURCE)
    }
}
