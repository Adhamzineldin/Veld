//! The `nest` adapter of the `node` server: one NestJS module and controller per service.
//!
//! The controller is a thin shell. Every route delegates to the SAME handler the other adapters
//! use (`create<Service>Handler`, served through `toNodeHttp`), so auth, middleware, validation,
//! error mapping and the problem+json bodies have one implementation. Nest and Swagger still see
//! the real routes, because each endpoint is a decorated method at its real path.
//!
//! This is the only adapter that emits files of its own (`server/nest/<service>.ts`). They need
//! `experimentalDecorators` and `emitDecoratorMetadata`; no other generated file does.

use veld_codegen::naming;
use veld_codegen::{FileSet, FrameworkAdapter, GenDiagnostic, LanguageVersion, Profile};
use veld_emit_typescript::util::{add_file, rel_import, ts_str};
use veld_ir::Method;

use super::{EndpointModel, ServerModel, ServiceModel};

/// The framework id, as written in `veld.toml`.
pub const ID: &str = "nest";

const TEMPLATE: &str = include_str!("../../runtime/nest/service.ts.tpl");

/// The `nest` framework (NestJS 10).
#[derive(Debug, Clone, Copy)]
pub struct Nest;

impl FrameworkAdapter<ServerModel> for Nest {
    fn id(&self) -> &'static str {
        ID
    }

    fn framework_versions(&self) -> &'static [&'static str] {
        &["10"]
    }

    fn min_language(&self) -> LanguageVersion {
        LanguageVersion::es(2020)
    }

    fn files(&self, model: &ServerModel, profile: &Profile) -> Result<FileSet, Vec<GenDiagnostic>> {
        let mut files = FileSet::new();
        if profile.framework() != Some(ID) {
            return Ok(files);
        }
        for svc in &model.services {
            let path = format!("server/nest/{}.ts", svc.file);
            add_file(&mut files, &path, source(svc, &path))?;
        }
        Ok(files)
    }
}

/// The Nest decorator of an HTTP method; `None` for WebSocket endpoints (not generated).
fn decorator(method: Method) -> Option<&'static str> {
    match method {
        Method::Get => Some("Get"),
        Method::Post => Some("Post"),
        Method::Put => Some("Put"),
        Method::Patch => Some("Patch"),
        Method::Delete => Some("Delete"),
        Method::Head => Some("Head"),
        Method::Options => Some("Options"),
        Method::Ws => None,
    }
}

fn route(e: &EndpointModel, decorator: &str) -> String {
    let path = if e.path.is_empty() {
        String::new()
    } else {
        ts_str(&e.path)
    };
    format!(
        "\n\n  @{decorator}({path})\n  @HttpCode({})\n  {}(@Req() req: NodeRequestLike, @Res() res: NodeResponseLike): void {{\n    this.#serve(req, res);\n  }}",
        e.status, e.name
    )
}

fn source(svc: &ServiceModel, path: &str) -> String {
    let mut decorators: Vec<&str> =
        vec!["Controller", "HttpCode", "Inject", "Module", "Req", "Res"];
    let mut routes = String::new();
    for e in &svc.endpoints {
        if let Some(d) = decorator(e.method) {
            decorators.push(d);
            routes.push_str(&route(e, d));
        }
    }
    decorators.sort_unstable();
    decorators.dedup();
    let prefix = if svc.prefix.is_empty() {
        String::new()
    } else {
        ts_str(&svc.prefix)
    };
    let optional = if svc.hooks_required { "" } else { "?" };
    TEMPLATE
        .replace("__DECORATORS__", &decorators.join(", "))
        .replace("__RUNTIME__", &rel_import(path, "runtime/veld"))
        .replace("__SERVICE__", &rel_import(path, &svc.module))
        .replace("__ADAPTERS__", &rel_import(path, "server/adapters"))
        .replace("__SNAKE__", &naming::screaming_snake(&svc.raw_name))
        .replace("__OPTIONAL__", optional)
        .replace("__PREFIX__", &prefix)
        .replace("__ROUTES__", &routes)
        .replace("__RAW__", &svc.raw_name)
        .replace("__NAME__", &svc.name)
}
