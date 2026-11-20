//! The `node-http` adapter of the `node` server.

use veld_codegen::{FileSet, FrameworkAdapter, GenDiagnostic, LanguageVersion, Profile};

use super::{ServerModel, part};

/// Where this adapter's section sits in `server/adapters.ts`.
const POSITION: u8 = 1;
const SOURCE: &str = include_str!("../../runtime/adapters/node_http.ts");

/// The `node-http` framework.
#[derive(Debug, Clone, Copy)]
pub struct NodeHttp;

impl FrameworkAdapter<ServerModel> for NodeHttp {
    fn id(&self) -> &'static str {
        "node-http"
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
