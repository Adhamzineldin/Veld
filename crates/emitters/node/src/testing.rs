//! The test handler (`spec/rules.md` section 12.4): with the target option `testing = true`, a
//! second entry point, in a file of its own, for each service with endpoint rules.
//!
//! It is built from the endpoint descriptions of the production file (`<service>Endpoints`) and
//! the runtime's `createHandler`, so it runs the same pipeline. Nothing in the production entry
//! point or in `runtime/veld.ts` reads a test header: this file replaces the loaders, the clock,
//! the auth and the application's handler before the runtime ever sees them.

use veld_codegen::naming;
use veld_codegen::{CodeWriter, FileSet, GenDiagnostic};
use veld_emit_typescript::tree::{FileCtx, Tree};
use veld_emit_typescript::util::{add_file, method_key, ts_str};
use veld_emit_typescript::validate::schema_expr;
use veld_ir::{Constraints, Service, Type};

use crate::rules::{loaders_name, service_has_rules};
use crate::server::{builder_name, module_of, std_sigs};

/// Root-relative module path of the test handler of a service.
fn module(tree: &Tree<'_>, svc: &Service) -> String {
    format!(
        "{}server/{}-test-handler",
        tree.root,
        naming::kebab(&svc.name)
    )
}

/// The warning at the top of the file.
const WARNING: &str = "//
// THE VELD TEST HANDLER. NEVER MOUNT IT IN PRODUCTION.
//
// It lets whoever sends a request choose the facts, the principal, the clock and the result of
// that request (`Veld-Test-*` headers), and it skips authentication and middleware. It exists so
// `veld test --live` can run the examples of a contract against this server. Creating it reports
// that once.
//";

/// Writes `server/<service>-test-handler.ts` for every service with endpoint rules, when the
/// option is on.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>, testing: bool) -> Result<(), Vec<GenDiagnostic>> {
    if !testing {
        return Ok(());
    }
    for sid in &tree.services {
        let svc = tree.ir.service(*sid);
        if !service_has_rules(svc) {
            continue;
        }
        let module = module(tree, svc);
        let mut fc = FileCtx::new(tree, &module);
        let mut w = CodeWriter::new();
        write_handler(&mut w, &mut fc, svc);
        let text = fc.finish(&w.finish());
        let text = text.replacen('\n', &format!("\n{WARNING}\n"), 1);
        add_file(files, &format!("{module}.ts"), text)?;
    }
    Ok(())
}

/// The principal of a request, from the wire JSON of `Veld-Test-Principal`: of the generated type
/// of the principal's model when the production hooks are typed that way (`spec/rules.md`
/// section 11.1), the plain JSON otherwise.
fn decoded_principal(fc: &mut FileCtx<'_, '_>, svc: &Service) -> String {
    let Some(p) = svc.principal.as_ref().filter(|_| fc.tree.uses_policies()) else {
        return "principal".to_string();
    };
    let schema = schema_expr(fc, &Type::Decl(p.decl), &Constraints::default());
    format!("{}({schema}, principal)", fc.rt("fromWire"))
}

fn write_helpers(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service) {
    let ctx = fc.rt("RequestContext");
    let base64 = fc.rt("base64Decode");
    let schema = fc.rt("Schema");
    let from_wire = fc.rt("fromWire");
    let principal = decoded_principal(fc, svc);
    w.line(format!(
        "function header(ctx: {ctx}, name: string): string | undefined {{
  const v = ctx.headers[name];
  return Array.isArray(v) ? v[0] : v;
}}

/** A header of base64url JSON: padded or not, with `-_` or `+/`. */
function decode(text: string | undefined): unknown {{
  if (text === undefined) return undefined;
  const standard = text.replace(/-/g, '+').replace(/_/g, '/');
  const json = {base64}(standard + '='.repeat((4 - (standard.length % 4)) % 4));
  if (json === null) throw new Error('a Veld-Test header is not base64url');
  return JSON.parse(json) as unknown;
}}

const parsed = new WeakMap<object, Record<string, unknown>>();

/** The `Veld-Test-Facts` of a request: fact name to wire value, `null` for absent. */
function testFacts(ctx: {ctx}): Record<string, unknown> {{
  let facts = parsed.get(ctx);
  if (facts === undefined) {{
    const raw = decode(header(ctx, 'veld-test-facts'));
    facts = typeof raw === 'object' && raw !== null ? (raw as Record<string, unknown>) : {{}};
    parsed.set(ctx, facts);
  }}
  return facts;
}}

async function testFact<T>(ctx: {ctx}, name: string, schema: {schema}): Promise<T | undefined> {{
  const wire = testFacts(ctx)[name];
  return wire === undefined || wire === null ? undefined : ({from_wire}(schema, wire) as T);
}}

/** The clock and the principal of the request, as the test sent them. */
function prime(ctx: {ctx}): Date {{
  const principal = decode(header(ctx, 'veld-test-principal'));
  if (principal !== undefined) ctx.principal = {principal};
  const now = header(ctx, 'veld-test-now');
  return now === undefined ? new Date() : new Date(now);
}}"
    ));
    w.blank();
}

fn write_loaders(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service, name: &str) {
    if svc.facts.is_empty() {
        return;
    }
    let ctx = fc.rt("RequestContext");
    w.open(format!("const loaders: {} =", loaders_name(name)));
    for fact in &svc.facts {
        let ty = veld_emit_typescript::types::ts_type(fc, &fact.ty);
        let schema = schema_expr(fc, &fact.ty, &Constraints::default());
        let params = if fact.key.is_some() {
            format!("(_key, ctx: {ctx})")
        } else {
            format!("(ctx: {ctx})")
        };
        w.line(format!(
            "{}: {params} => testFact<{ty}>(ctx, {}, {schema}),",
            method_key(&fact.name),
            ts_str(&fact.name)
        ));
    }
    w.close_with("};");
    w.blank();
}

/// What the handler answers with. A contract with an audited rule (`@rollout(audit)`) gets the
/// answer with the response header `Veld-Test-Audited` (`spec/rules.md` section 12.4), and the
/// function that builds it is written here; any other contract gets the plain answer and nothing
/// else, so its file keeps its bytes.
fn write_audited(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>) -> String {
    if !fc.tree.uses_rollout() {
        return "return inner(req);".to_string();
    }
    let violation = fc.rt("Violation");
    let encode = fc.rt("base64Encode");
    let log = fc.rt("auditLog");
    w.line(format!(
        "/** `Veld-Test-Audited`: the first 100 audited reports as compact JSON, in unpadded base64url. */
function auditedHeader(found: readonly {violation}[]): string {{
  return {encode}(JSON.stringify(found.slice(0, 100))).replace(/\\+/g, '-').replace(/\\//g, '_').replace(/=+$/, '');
}}"
    ));
    w.blank();
    format!(
        "const audited = {log}(req);
    const res = await inner(req);
    if (res === null || audited.length === 0) return res;
    return {{ ...res, headers: {{ ...res.headers, 'veld-test-audited': auditedHeader(audited) }} }};"
    )
}

fn write_handler(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service) {
    let name = naming::pascal(&svc.name);
    let prod = module_of(fc.tree, svc);
    fc.import(&prod, &format!("{name}Service"), false);
    fc.import(&prod, &format!("{name}Hooks"), false);
    fc.import(&prod, &builder_name(svc), true);
    if !svc.facts.is_empty() {
        fc.import(&prod, &loaders_name(&name), false);
    }
    write_helpers(w, fc, svc);
    write_loaders(w, fc, svc, &name);
    let answer = write_audited(w, fc);
    let handler = fc.rt("VeldHandler");
    let endpoint = fc.rt("ServerEndpoint");
    let ctx = fc.rt("RequestContext");
    let early = fc.rt("EarlyResponse");
    let from_wire = fc.rt("fromWire");
    let create = fc.rt("createHandler");
    let std = std_sigs(fc);
    let digest = match svc.rules_digest.as_deref() {
        Some(d) => ts_str(d),
        None => "null".to_string(),
    };
    let loaders_arg = if svc.facts.is_empty() {
        ""
    } else {
        ", loaders"
    };
    w.line(format!(
        "/** What the test handler of `{name}` takes from the hooks: the observers of the production hooks. */
export type {name}TestHooks = Omit<{name}Hooks, 'facts' | 'clock' | 'auth' | 'middleware'>;

/** Takes the endpoint apart from authentication and the application's handler. */
function open(ep: {endpoint}): {endpoint} {{
  const call: {endpoint}['call'] = (req, ctx, session) => {{
    prime(ctx);
    if (header(ctx, 'veld-test-dry-run') === '1') {{
      return new {early}({{ status: 204, headers: {{ 'veld-test-passed': '1' }}, body: null }});
    }}
    const wire = decode(header(ctx, 'veld-test-result'));
    if (wire === undefined) return ep.call(req, ctx, session);
    return ep.success === null ? undefined : {from_wire}(ep.success, wire);
  }};
  const copy: {endpoint} = {{ ...ep, middleware: [], call }};
  delete copy.auth;
  return copy;
}}

/**
 * The test handler of `{name}` (spec/rules.md section 12.4): the production pipeline, with the
 * facts, the principal, the clock and, when sent, the result taken from `Veld-Test-*` headers.
 * `Veld-Test-Dry-Run: 1` stops after the rules that run before the handler and answers `204` with
 * `Veld-Test-Passed: 1`. `GET /__veld/test` answers with the rules digest of the service. NEVER
 * mount it in production.
 */
export function create{name}TestHandler(impl: {name}Service, hooks: {name}TestHooks = {{}}): {handler} {{
  const log = (globalThis as {{ console?: {{ warn?: (message: string) => void }} }}).console;
  log?.warn?.('veld: the TEST HANDLER of {name} is active: it takes facts, principal, clock and results from request headers. Never mount it in production.');
  const endpoints = {builder}(impl{loaders_arg}).map(open);
  const inner = {create}(endpoints, {std}, {{ ...hooks, clock: (ctx: {ctx}) => prime(ctx) }});
  const handler = async (req: Parameters<{handler}>[0]) => {{
    if (req.method.toUpperCase() === 'GET' && req.path === '/__veld/test') {{
      const body = JSON.stringify({{ veldTestHandler: 1, digests: {{ {key}: {digest} }} }});
      return {{ status: 200, headers: {{ 'content-type': 'application/json' }}, body }};
    }}
    {answer}
  }};
  return Object.assign(handler, {{ ws: inner.ws }});
}}",
        builder = builder_name(svc),
        key = ts_str(&svc.name),
    ));
}
