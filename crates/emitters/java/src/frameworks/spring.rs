//! The Spring Boot 3 (Jakarta) adapter, the only generated code with a dependency.
//!
//! One `@RestController` per service maps every endpoint at its real path, so Spring's own
//! tooling (actuator, springdoc) sees the routes. Each method hands the servlet request to the
//! shared `SpringAdapter.respond`, which runs the endpoint through the same core pipeline as the
//! JDK adapter: there is no second implementation of the semantics.

use veld_codegen::{
    CodeWriter, FileSet, FrameworkAdapter, GenDiagnostic, LanguageVersion, Profile,
};

use super::{EndpointKind, EndpointModel, ServerModel, ServiceModel, class_file};
use crate::runtime;
use crate::tree::{Imports, java_path};
use crate::util::{add_file, java_str};

const SOURCE: &str = include_str!("../../adapters/SpringAdapter.java");
const WS_SOURCE: &str = include_str!("../../adapters/SpringWebSocketAdapter.java");

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
        if model.has_ws() {
            for (path, bytes) in class_file(model, "SpringWebSocketAdapter", WS_SOURCE)?.iter() {
                add_file(
                    &mut files,
                    path,
                    String::from_utf8_lossy(bytes).into_owned(),
                )?;
            }
        }
        for svc in &model.services {
            let class = controller_class(svc);
            add_file(
                &mut files,
                &java_path(&svc.adapter_package, &class),
                controller(model, svc, &class),
            )?;
            if svc.has_ws() {
                let config = ws_config_class(svc);
                add_file(
                    &mut files,
                    &java_path(&svc.adapter_package, &config),
                    ws_config(model, svc, &config),
                )?;
            }
        }
        Ok(files)
    }
}

/// The constructor that builds the shared handler from the service bean and the hooks bean.
fn write_ctor(
    w: &mut CodeWriter,
    imp: &mut Imports,
    svc: &ServiceModel,
    class: &str,
    (service, hooks, handler_factory): (&str, &str, &str),
    loaders: Option<&str>,
) {
    let (given, pass) = match loaders {
        Some(l) => (format!("{service} service, {l} loaders, "), "service, loaders"),
        None => (format!("{service} service, "), "service"),
    };
    if svc.needs_hooks {
        w.open(format!("public {class}({given}{hooks} hooks)"));
        w.line(format!(
            "this.handler = {handler_factory}.create({pass}, hooks);"
        ));
    } else {
        let provider = imp.name("org.springframework.beans.factory.ObjectProvider");
        w.open(format!("public {class}({given}{provider}<{hooks}> hooks)"));
        w.line(format!(
            "this.handler = {handler_factory}.create({pass}, hooks.getIfAvailable({hooks}::new));"
        ));
    }
    w.close();
}

fn controller_class(svc: &ServiceModel) -> String {
    format!("{}Controller", svc.name)
}

const MVC: &str = "org.springframework.web.bind.annotation";

/// The Spring mapping annotation of an endpoint, importing what it uses.
///
/// With `slash` the path is also mapped with a trailing slash, because the shared router ignores
/// it (`spec/targets/java.md` section 5) and Spring MVC 6 does not.
fn mapping(imp: &mut Imports, ep: &EndpointModel, slash: bool) -> String {
    let path = if slash && ep.path != "/" {
        format!(
            "{{{}, {}}}",
            java_str(&ep.path),
            java_str(&format!("{}/", ep.path))
        )
    } else {
        java_str(&ep.path)
    };
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
    let loaders = svc.loaders.as_ref().map(|l| imp.name(&format!("{server}.{l}")));
    let veld = imp.name(&format!("{}.VeldHandler", runtime::package(&model.base)));
    let rest_controller = imp.name(&format!("{MVC}.RestController"));
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
    write_ctor(
        &mut w,
        &mut imp,
        svc,
        class,
        (&service, &hooks, &handler_factory),
        loaders.as_deref(),
    );
    for ep in svc.endpoints.iter().filter(|e| e.kind != EndpointKind::Ws) {
        w.blank();
        w.line(mapping(&mut imp, ep, svc.rules));
        let bridge = imp.name(&format!(
            "{}.SpringAdapter",
            runtime::adapter_package(&model.base)
        ));
        let response = imp.name("org.springframework.http.ResponseEntity");
        let request = imp.name("jakarta.servlet.http.HttpServletRequest");
        let io_exception = imp.name("java.io.IOException");
        let (body, call) = if ep.kind == EndpointKind::Sse {
            (
                imp.name(
                    "org.springframework.web.servlet.mvc.method.annotation.StreamingResponseBody",
                ),
                "stream",
            )
        } else {
            ("byte[]".to_string(), "respond")
        };
        w.open(format!(
            "public {response}<{body}> {}({request} request) throws {io_exception}",
            ep.method_name
        ));
        w.line(format!(
            "return {bridge}.{call}(handler, {}, request);",
            java_str(&ep.name)
        ));
        w.close();
    }
    w.close();
    imp.finish(&w.finish())
}

fn ws_config_class(svc: &ServiceModel) -> String {
    format!("{}WebSocketConfig", svc.name)
}

/// The path of a `ws` endpoint as an Ant pattern: `{param}` segments become `*`.
fn ant_pattern(path: &str) -> String {
    path.split('/')
        .map(|seg| {
            if seg.starts_with('{') && seg.ends_with('}') {
                "*"
            } else {
                seg
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// A `WebSocketConfigurer` that registers the service's `ws` endpoints with Spring WebSocket.
fn ws_config(model: &ServerModel, svc: &ServiceModel, class: &str) -> String {
    let server = &svc.server_package;
    let name = &svc.name;
    let mut imp = Imports::new(&svc.adapter_package, class);
    let service = imp.name(&format!("{server}.{name}Service"));
    let hooks = imp.name(&format!("{server}.{name}Hooks"));
    let handler_factory = imp.name(&format!("{server}.{name}Handler"));
    let loaders = svc.loaders.as_ref().map(|l| imp.name(&format!("{server}.{l}")));
    let veld = imp.name(&format!("{}.VeldHandler", runtime::package(&model.base)));
    let bridge = imp.name(&format!(
        "{}.SpringWebSocketAdapter",
        runtime::adapter_package(&model.base)
    ));
    let ws = "org.springframework.web.socket";
    let configurer = imp.name(&format!("{ws}.config.annotation.WebSocketConfigurer"));
    let registry = imp.name(&format!("{ws}.config.annotation.WebSocketHandlerRegistry"));
    let enable = imp.name(&format!("{ws}.config.annotation.EnableWebSocket"));
    let configuration = imp.name("org.springframework.context.annotation.Configuration");
    let paths: Vec<String> = svc
        .endpoints
        .iter()
        .filter(|e| e.kind == EndpointKind::Ws)
        .map(|e| java_str(&ant_pattern(&e.path)))
        .collect();
    let mut w = CodeWriter::new();
    w.line(format!(
        "/** Registers the `ws` endpoints of the `{name}` service with Spring WebSocket. */"
    ));
    w.line(format!("@{configuration}"));
    w.line(format!("@{enable}"));
    w.open(format!("public class {class} implements {configurer}"));
    w.line(format!("private final {veld} handler;"));
    w.blank();
    w.line(format!(
        "/** Creates the configuration from the {name}Service bean and its hooks. */"
    ));
    write_ctor(
        &mut w,
        &mut imp,
        svc,
        class,
        (&service, &hooks, &handler_factory),
        loaders.as_deref(),
    );
    w.blank();
    w.line("@Override");
    w.open(format!(
        "public void registerWebSocketHandlers({registry} registry)"
    ));
    w.line(format!(
        "registry.addHandler({bridge}.handler(handler), {});",
        paths.join(", ")
    ));
    w.close();
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
                rules: false,
                loaders: None,
                endpoints: vec![
                    EndpointModel {
                        name: "getOrder".into(),
                        method_name: "getOrder".into(),
                        http_method: "GET",
                        path: "/orders/{id}".into(),
                        kind: EndpointKind::Http,
                    },
                    EndpointModel {
                        name: "ping".into(),
                        method_name: "ping".into(),
                        http_method: "HEAD",
                        path: "/ping".into(),
                        kind: EndpointKind::Http,
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
    fn streaming_model() -> ServerModel {
        let endpoint = |name: &str, path: &str, kind| EndpointModel {
            name: name.into(),
            method_name: name.into(),
            http_method: if kind == EndpointKind::Ws {
                "WS"
            } else {
                "GET"
            },
            path: path.into(),
            kind,
        };
        ServerModel {
            base: "a.b".into(),
            services: vec![ServiceModel {
                name: "Live".into(),
                server_package: "a.b.live.server".into(),
                adapter_package: "a.b.adapters.live".into(),
                needs_hooks: false,
                rules: false,
                loaders: None,
                endpoints: vec![
                    endpoint("ticks", "/live/ticks", EndpointKind::Sse),
                    endpoint("chat", "/live/chat/{room}", EndpointKind::Ws),
                ],
            }],
        }
    }

    #[test]
    fn websocket_endpoints_get_a_config_and_no_controller_method() {
        let model = streaming_model();
        let svc = &model.services[0];
        let controller = controller(&model, svc, "LiveController");
        assert!(controller.contains("ResponseEntity<StreamingResponseBody> ticks("));
        assert!(controller.contains("SpringAdapter.stream(handler, \"ticks\", request)"));
        assert!(!controller.contains("chat"), "{controller}");
        let config = ws_config(&model, svc, "LiveWebSocketConfig");
        assert!(config.contains(
            "registry.addHandler(SpringWebSocketAdapter.handler(handler), \"/live/chat/*\");"
        ));
        assert!(config.contains("@EnableWebSocket"));
        assert!(!config.contains("ticks"));
    }

    #[test]
    fn the_websocket_adapter_is_emitted_only_for_ws_contracts() {
        let files = |model: &ServerModel| {
            Spring
                .files(model, &Profile::default())
                .unwrap()
                .paths()
                .map(str::to_string)
                .collect::<Vec<_>>()
        };
        let with_ws = files(&streaming_model());
        assert!(with_ws.contains(&"a/b/adapters/SpringWebSocketAdapter.java".to_string()));
        assert!(with_ws.contains(&"a/b/adapters/live/LiveWebSocketConfig.java".to_string()));
        let mut sse_only = streaming_model();
        sse_only.services[0].endpoints.pop();
        assert!(files(&sse_only).iter().all(|p| !p.contains("WebSocket")));
    }
}
