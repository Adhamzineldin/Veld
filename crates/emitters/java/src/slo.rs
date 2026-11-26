//! Service levels (`spec/platform.md` section 13, `spec/targets/java.md` section 16.4): the
//! `metrics` hook of `<Service>Hooks`. The measuring is the router's (`// #if slo` regions of
//! `Router` and `HandlerHooks`, `runtime/Metric.java`).

use veld_codegen::CodeWriter;
use veld_ir::{Ir, Service, ServiceId};

use crate::tree::FileCtx;

/// Whether the service has an endpoint with an `@slo` (its own or the service's default).
#[must_use]
pub fn service_has_slo(svc: &Service) -> bool {
    svc.endpoints.iter().any(|e| e.slo.is_some())
}

/// Whether any of the services has one.
#[must_use]
pub fn services_have_slo(ir: &Ir, services: &[ServiceId]) -> bool {
    services.iter().any(|s| service_has_slo(ir.service(*s)))
}

/// `metrics(hook)`: present in the type only for a service with an `@slo`.
pub fn write_hook(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, class: &str) {
    let hooks = fc.rt("HandlerHooks");
    w.line("/**");
    w.line(" * Observes every request that matched an endpoint of this server, once, when its answer is ready to");
    w.line(" * be written (`spec/platform.md` section 13): the duration in seconds and the attributes of the");
    w.line(" * OpenTelemetry HTTP conventions. A failing hook is ignored. The test handler does not call it.");
    w.line(" */");
    w.open(format!("public {class} metrics({hooks}.MetricsHook hook)"));
    w.line("builder.metrics(hook);");
    w.line("return this;");
    w.close();
    w.blank();
}
