//! The Spring Boot 3 (Jakarta) adapter, the only generated code with a dependency.
//!
//! One `@RestController` per service maps every endpoint at its real path, so Spring's own
//! tooling (actuator, springdoc) sees the routes. Each method hands the servlet request to the
//! shared `SpringAdapter.respond`, which runs the endpoint through the same core pipeline as the
//! JDK adapter: there is no second implementation of the semantics.

use veld_codegen::{
    CodeWriter, FileSet, FrameworkAdapter, GenDiagnostic, LanguageVersion, Profile,
};

use super::{EndpointModel, ServerModel, ServiceModel, class_file};
use crate::runtime;
use crate::tree::{Imports, java_path};
use crate::util::{add_file, java_str};

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
        let mut files = class_file(model, "SpringAdapter", SOURCE)?;
        for svc in &model.services {
            let class = controller_class(svc);
            add_file(
                &mut files,
                &java_path(&svc.adapter_package, &class),
                controller(model, svc, &class),
            )?;
        }
        Ok(files)
    }
}

fn controller_class(svc: &ServiceModel) -> String {
    format!("{}Controller", svc.name)
}

const MVC: &str = "org.springframework.web.bind.annotation";

/// The Spring mapping annotation of an endpoint, importing what it uses.
fn mapping(imp: &mut Imports, ep: &EndpointModel) -> String {
    let path = java_str(&ep.path);
    let verb = match ep.http_method {
        "GET" => "Get",
        "POST" => "Post",
        "PUT" => "Put",
        "PATCH" => "Patch",
        "DELETE" => "Delete",
        other => {
            let request_mapping = imp.name(&format!("{MVC}.RequestMapping"));
            let request_method = imp.name(&format!("{MVC}.RequestMethod"));
            return format!(
                "@{request_mapping}(path = {path}, method = {request_method}.{})",
                other.to_ascii_uppercase()
            );
        }
    };
    format!("@{}({path})", imp.name(&format!("{MVC}.{verb}Mapping")))
}

fn controller(model: &ServerModel, svc: &ServiceModel, class: &str) -> String {
    let server = &svc.server_package;
    let name = &svc.name;
    let mut imp = Imports::new(&svc.adapter_package, class);
    let service = imp.name(&format!("{server}.{name}Service"));
    let hooks = imp.name(&format!("{server}.{name}Hooks"));
    let handler_factory = imp.name(&format!("{server}.{name}Handler"));
    let veld = imp.name(&format!("{}.VeldHandler", runtime::package(&model.base)));
    let bridge = imp.name(&format!(
        "{}.SpringAdapter",
        runtime::adapter_package(&model.base)
    ));
    let rest_controller = imp.name(&format!("{MVC}.RestController"));
    let response = imp.name("org.springframework.http.ResponseEntity");
    let request = imp.name("jakarta.servlet.http.HttpServletRequest");
    let io_exception = imp.name("java.io.IOException");
    let mut w = CodeWriter::new();
    w.line(format!(
        "/** The Spring MVC routes of the `{name}` service. Every method runs the shared Veld pipeline. */"
    ));
    w.line(format!("@{rest_controller}"));
    w.open(format!("public class {class}"));
    w.line(format!("private final {veld} handler;"));
    w.blank();
    w.line("/**");
    w.line(format!(
        " * Creates the controller from the {name}Service bean{}.",
        if svc.needs_hooks {
            " and the hooks bean"
        } else {
            " and an optional hooks bean"
        }
    ));
    w.line(" */");
    if svc.needs_hooks {
        w.open(format!("public {class}({service} service, {hooks} hooks)"));
        w.line(format!(
            "this.handler = {handler_factory}.create(service, hooks);"
        ));
    } else {
        let provider = imp.name("org.springframework.beans.factory.ObjectProvider");
        w.open(format!(
            "public {class}({service} service, {provider}<{hooks}> hooks)"
        ));
        w.line(format!(
            "this.handler = {handler_factory}.create(service, hooks.getIfAvailable({hooks}::new));"
        ));
    }
    w.close();
    for ep in &svc.endpoints {
        w.blank();
        w.line(mapping(&mut imp, ep));
        w.open(format!(
            "public {response}<byte[]> {}({request} request) throws {io_exception}",
            ep.method_name
        ));
        w.line(format!(
            "return {bridge}.respond(handler, {}, request);",
            java_str(&ep.name)
        ));
        w.close();
    }
    w.close();
    imp.finish(&w.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_controller_maps_each_endpoint_at_its_real_path() {
        let model = ServerModel {
            base: "a.b".into(),
            services: vec![ServiceModel {
                name: "Orders".into(),
                server_package: "a.b.orders.server".into(),
                adapter_package: "a.b.adapters.orders".into(),
                needs_hooks: true,
                endpoints: vec![
                    EndpointModel {
                        name: "getOrder".into(),
                        method_name: "getOrder".into(),
                        http_method: "GET",
                        path: "/orders/{id}".into(),
                    },
                    EndpointModel {
                        name: "ping".into(),
                        method_name: "ping".into(),
                        http_method: "HEAD",
                        path: "/ping".into(),
                    },
                ],
            }],
        };
        let text = controller(&model, &model.services[0], "OrdersController");
        assert!(text.contains("@GetMapping(\"/orders/{id}\")"));
        assert!(text.contains("@RequestMapping(path = \"/ping\", method = RequestMethod.HEAD)"));
        assert!(text.contains("SpringAdapter.respond(handler, \"getOrder\", request)"));
        assert!(text.contains("OrdersHandler.create(service, hooks)"));
    }
}
