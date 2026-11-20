//! The `fastify` adapter of the `node` server.

use veld_codegen::{FileSet, FrameworkAdapter, GenDiagnostic, LanguageVersion, Profile};

use super::{ServerModel, part};

/// Where this adapter's section sits in `server/adapters.ts`.
const POSITION: u8 = 3;
const SOURCE: &str = include_str!("../../runtime/adapters/fastify.ts");

/// The `fastify` framework.
#[derive(Debug, Clone, Copy)]
pub struct Fastify;

impl FrameworkAdapter<ServerModel> for Fastify {
    fn id(&self) -> &'static str {
        "fastify"
    }

    fn framework_versions(&self) -> &'static [&'static str] {
        &[]
    }

    fn min_language(&self) -> LanguageVersion {
        LanguageVersion::es(2020)
    }

    fn files(
        &self,
        model: &ServerModel,
        _profile: &Profile,
    ) -> Result<FileSet, Vec<GenDiagnostic>> {
        part(model, POSITION, SOURCE)
    }
}
