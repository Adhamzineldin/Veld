//! Timed moves (`spec/platform.md` section 10, `spec/targets/rust.md` section 17): the due loaders
//! of a service and the runner that runs the moves that are due through the pipeline.

use veld_codegen::CodeWriter;
use veld_ir::{Endpoint, Service};

use crate::endpoints::request_name;
use crate::util::{method, snake};

/// Whether the endpoint takes a timed edge.
#[must_use]
pub fn is_timed(ep: &Endpoint) -> bool {
    ep.rules.transition.as_ref().is_some_and(|t| t.is_timed())
}

/// Whether a service has an endpoint with a timed transition.
#[must_use]
pub fn svc_timed(svc: &Service) -> bool {
    svc.endpoints.iter().any(is_timed)
}

fn name_of(svc: &Service) -> String {
    crate::server::service_type(svc)
}

/// The edge's milliseconds of a timed endpoint, from the machine of its transition.
fn after_ms(ir: &veld_ir::Ir, ep: &Endpoint) -> Option<i64> {
    let t = ep.rules.transition.as_ref()?;
    let [source] = t.from.as_slice() else {
        return None;
    };
    match &ir.decl(t.decl).kind {
        veld_ir::DeclKind::Enum(e) => e
            .flows
            .iter()
            .find(|f| &f.from == source && f.to == t.to)?
            .after_ms
            .as_deref()?
            .parse()
            .ok(),
        veld_ir::DeclKind::Model(_)
        | veld_ir::DeclKind::Error(_)
        | veld_ir::DeclKind::Union(_)
        | veld_ir::DeclKind::Const(_)
        | veld_ir::DeclKind::Event(_) => None,
    }
}

/// `<Service>DueLoaders`, `<Service>DueOptions` and `run_<service>_due_moves`.
pub fn emit(w: &mut CodeWriter, ir: &veld_ir::Ir, svc: &Service) {
    let name = name_of(svc);
    let timed: Vec<&Endpoint> = svc.endpoints.iter().filter(|e| is_timed(e)).collect();
    if timed.is_empty() {
        return;
    }
    loaders(w, &name, &timed);
    options(w, &name);
    runner(w, ir, svc, &name, &timed);
}

fn loaders(w: &mut CodeWriter, name: &str, timed: &[&Endpoint]) {
    w.line("/// The due moves of the service (`spec/platform.md` section 10.1): one method per endpoint with a");
    w.line("/// timed transition. It answers the parameters of up to `limit` calls of that endpoint whose fact");
    w.line("/// entered the source state at or before `before`, from your own store (for example");
    w.line("/// `where status = 'pending' and pending_since <= :before`).");
    w.open(format!("pub trait {name}DueLoaders: Send + Sync"));
    for ep in timed {
        w.line(format!(
            "fn {}(&self, before: std::time::SystemTime, limit: usize, ctx: &veld::server::RequestContext) -> impl std::future::Future<Output = Result<Vec<{}>, veld::Error>> + Send;",
            method(&ep.name),
            request_name(ep)
        ));
    }
    w.close().blank();
}

fn options(w: &mut CodeWriter, name: &str) {
    w.line("/// What a run needs: the loaders, the clock reading (default: the clock of the hooks) and the most");
    w.line("/// calls per endpoint (default 100).");
    w.line("#[derive(Debug, Clone)]");
    w.open(format!("pub struct {name}DueOptions<D>"));
    w.line("pub loaders: D,");
    w.line("pub now: Option<std::time::SystemTime>,");
    w.line("pub limit: usize,");
    w.close().blank();
    w.open(format!("impl<D> {name}DueOptions<D>"));
    w.line("/// Options with the default clock and a limit of 100.");
    w.open("pub fn new(loaders: D) -> Self");
    w.line("Self { loaders, now: None, limit: 100 }");
    w.close().close().blank();
}

fn runner(w: &mut CodeWriter, ir: &veld_ir::Ir, svc: &Service, name: &str, timed: &[&Endpoint]) {
    let fn_name = format!("run_{}_due_moves", snake(&svc.name));
    w.line("/// Runs the moves that are due through the pipeline, in process (`spec/platform.md` section 10.1).");
    w.line("/// For each endpoint with a timed transition it asks the loader for the calls whose fact entered the");
    w.line("/// source state at or before `now - after`, and runs each as a call of that endpoint (steps 4 to 10,");
    w.line("/// no middleware) as the caller `hooks.timer_principal()` answers. A failure does not stop the others.");
    w.line("/// Running it twice, or on two machines at once, is safe exactly as far as the handler's conditional");
    w.line("/// write is: the guard is not an atomic guarantee.");
    w.line("///");
    w.line("/// # Errors");
    w.line("/// An endpoint with authentication and no `timer_principal` in the hooks: nothing is run.");
    w.open(format!(
        "pub async fn {fn_name}<E: veld::server::Endpoints, D: {name}DueLoaders>(handler: &veld::server::Handler<E>, options: &{name}DueOptions<D>) -> Result<veld::server::DueReport, veld::Error>"
    ));
    w.line("let e = handler.endpoints();");
    let auth = timed
        .iter()
        .any(|ep| ep.auth.kind != veld_ir::AuthKind::None);
    if auth {
        w.line("let principal = e.timer_principal().await;");
        for ep in timed
            .iter()
            .filter(|ep| ep.auth.kind != veld_ir::AuthKind::None)
        {
            w.open("if principal.is_none()");
            w.line(format!(
                "return Err(veld::Error::other(\"{}: hooks.timer_principal is required\"));",
                ep.name
            ));
            w.close();
        }
    }
    w.line("let mut report = veld::server::DueReport::new();");
    for ep in timed {
        let ms = after_ms(ir, ep).unwrap_or(0);
        let who = if ep.auth.kind == veld_ir::AuthKind::None {
            "None"
        } else {
            "principal.clone()"
        };
        w.open("'ep:");
        w.line(format!("let name = {};", crate::util::lit(&ep.name)));
        w.line("let ctx = handler.timer_context(name).ok_or_else(|| veld::Error::other(\"unknown endpoint\"))?;");
        w.line("let now = options.now.unwrap_or_else(|| e.timer_clock(&ctx));");
        w.line(format!(
            "let before = now.checked_sub(std::time::Duration::from_millis({ms}_u64)).unwrap_or(std::time::UNIX_EPOCH);"
        ));
        w.line("let counts = report.entry(name.to_string()).or_default();");
        w.line(format!(
            "let calls = match options.loaders.{}(before, options.limit, &ctx).await {{",
            method(&ep.name)
        ));
        w.line("    Ok(calls) => calls,");
        w.line("    Err(err) => {");
        w.line("        e.on_error(&err, &ctx);");
        w.line("        counts.failed += 1;");
        w.line("        break 'ep;");
        w.line("    }");
        w.line("};");
        w.open("for call in calls.into_iter().take(options.limit)");
        w.line("counts.ran += 1;");
        w.line("let args = match serde_json::to_value(&call) {");
        w.line("    Ok(serde_json::Value::Object(args)) => args,");
        w.line("    _ => { counts.failed += 1; continue; }");
        w.line("};");
        w.line(format!(
            "match handler.due_move(name, &args, veld::server::TimerCall {{ principal: {who}, now }}).await {{"
        ));
        w.line("    veld::server::DueOutcome::Moved => counts.moved += 1,");
        w.line("    veld::server::DueOutcome::Refused => counts.refused += 1,");
        w.line("    veld::server::DueOutcome::Failed => counts.failed += 1,");
        w.line("}");
        w.close();
        w.close();
    }
    w.line("Ok(report)");
    w.close().blank();
}
