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

use crate::rules::{loaders_name, service_has_builder, service_is_idempotent};
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
        if !service_has_builder(svc) {
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

/// The part of the test handler that gives a request the idempotency store of its header
/// (`spec/rules.md` sections 11.9 and 11.12). Only a service with an `@idempotent` endpoint has it.
fn write_idempotency(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>) {
    let store = fc.rt("IdempotencyStore");
    let memory = fc.rt("MemoryIdempotencyStore");
    let begin = fc.rt("IdempotencyBegin");
    let answer = fc.rt("StoredAnswer");
    let endpoint = fc.rt("ServerEndpoint");
    let from_wire = fc.rt("fromWire");
    let canonical = fc.rt("canonical");
    let request = fc.rt("VeldRequest");
    let ctx = fc.rt("RequestContext");
    let hooks = fc.rt("IdempotencyHooks");
    // The `ETag` of a stored success: only a contract with a version field has the member.
    let etag = if fc.tree.uses_version() {
        format!(
            "  const value = ep.success === null ? undefined : ({from_wire}(ep.success, state.data) as Record<string, unknown>);
  const held = ep.etag === undefined || value === undefined ? undefined : value[ep.etag];
  if (typeof held === 'number' || typeof held === 'string') out.etag = `\"${{held}}\"`;
"
        )
    } else {
        String::new()
    };
    w.line(format!(
        "function requestHeader(req: {request}, name: string): string | undefined {{
  for (const key of Object.keys(req.headers)) {{
    if (key.toLowerCase() !== name) continue;
    const v = req.headers[key];
    return Array.isArray(v) ? v[0] : v;
  }}
  return undefined;
}}

/** What `Veld-Test-Idempotency` says the store holds for the key of the request. */
interface StoreState {{
  state?: string;
  status?: number;
  type?: string | null;
  data?: unknown;
}}

/** The stored answer of a `stored` state: an error as a problem document, a success as its body. */
function storedAnswer(ep: {endpoint} | undefined, state: StoreState): {answer} {{
  const status = state.status ?? 200;
  const type = state.type ?? null;
  if (ep === undefined) return {{ status, body: null }};
  if (type !== null) {{
    const spec = ep.errors.find((e) => e.sig === type);
    const data = state.data ?? {{}};
    const title = spec === undefined ? 'Error' : spec.create({from_wire}(spec.schema, data), {{ status, op: ep.sig }}).title;
    const body = JSON.stringify({{ type, title, status, operation: ep.sig, data }});
    return {{ status, body, contentType: 'application/problem+json' }};
  }}
  if (state.data === null || state.data === undefined) return {{ status, body: null }};
  const out: {answer} = {{ status, body: JSON.stringify(state.data), contentType: 'application/json' }};
{etag}  return out;
}}

/**
 * The store of one request: it answers `begin` as `Veld-Test-Idempotency` says (nothing, a stored
 * answer, another fingerprint, a request still running) and otherwise works like the memory store.
 */
class TestStore implements {store} {{
  private readonly memory = new {memory}();

  constructor(
    private readonly state: StoreState | undefined,
    private readonly endpoints: readonly {endpoint}[],
  ) {{}}

  begin(scope: string, key: string, fingerprint: string): Promise<{begin}> {{
    switch (this.state?.state) {{
      case 'stored': {{
        const sig = scope.split('\\n')[0];
        return Promise.resolve({{ state: 'replay', answer: storedAnswer(this.endpoints.find((e) => e.sig === sig), this.state) }});
      }}
      case 'other':
        return Promise.resolve({{ state: 'other' }});
      case 'running':
        return Promise.resolve({{ state: 'running' }});
      default:
        return this.memory.begin(scope, key, fingerprint);
    }}
  }}

  complete(scope: string, key: string, answer: {answer}): Promise<void> {{
    return this.memory.complete(scope, key, answer);
  }}

  release(scope: string, key: string): Promise<void> {{
    return this.memory.release(scope, key);
  }}
}}

/** The caller text of the test handler: the wire JSON of `Veld-Test-Principal` in canonical form, or `caller`. */
function testCaller(_principal: never, ctx: {ctx}): string {{
  const who = decode(header(ctx, 'veld-test-principal'));
  return who === undefined ? 'caller' : {canonical}(who);
}}

function idempotencyOf(req: {request}, endpoints: readonly {endpoint}[]): {hooks} {{
  const raw = decode(requestHeader(req, 'veld-test-idempotency'));
  const state = typeof raw === 'object' && raw !== null ? (raw as StoreState) : undefined;
  return {{ store: new TestStore(state, endpoints), caller: testCaller }};
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

/// `const inner = ...`: the production handler built from the endpoints of the test handler. A
/// service with an `@idempotent` endpoint builds one per request, because the store of a request
/// comes from its header.
fn inner_handler(fc: &mut FileCtx<'_, '_>, create: &str, std: &str, idempotent: bool) -> String {
    let ctx = fc.rt("RequestContext");
    let clock = if fc.tree.uses_endpoint_rules() {
        format!(", clock: (ctx: {ctx}) => prime(ctx)")
    } else {
        String::new()
    };
    if !idempotent {
        return format!("const inner = {create}(endpoints, {std}, {{ ...hooks{clock} }});");
    }
    let handler = fc.rt("VeldHandler");
    let request = fc.rt("VeldRequest");
    format!(
        "const base = {create}(endpoints, {std}, {{ ...hooks{clock}, idempotency: idempotencyOf({{ headers: {{}}, method: 'GET', path: '/' }}, endpoints) }});
  const inner: {handler} = Object.assign(
    (req: {request}) => {create}(endpoints, {std}, {{ ...hooks{clock}, idempotency: idempotencyOf(req, endpoints) }})(req),
    {{ ws: base.ws }},
  );"
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
    let idempotent = service_is_idempotent(svc);
    if idempotent {
        write_idempotency(w, fc);
    }
    write_loaders(w, fc, svc, &name);
    let answer = write_audited(w, fc);
    let handler = fc.rt("VeldHandler");
    let endpoint = fc.rt("ServerEndpoint");
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
    let omitted = if idempotent {
        "'facts' | 'clock' | 'auth' | 'middleware' | 'idempotency'"
    } else {
        "'facts' | 'clock' | 'auth' | 'middleware'"
    };
    // The output mode of the rules is a setting of the runtime config, which the test handler
    // does not take: its hooks keep the member, so a test can choose the mode.
    let (output, note) = if fc.tree.uses_rules() {
        let mode = fc.rt("RuleOutputMode");
        (
            format!(" & {{ rules?: {{ output?: {mode} }} }}"),
            ", and the output mode of the rules, which a test handler takes here because it has no runtime config",
        )
    } else {
        (String::new(), "")
    };
    w.line(format!(
        "/** What the test handler of `{name}` takes from the hooks: the observers of the production hooks{note}. */
export type {name}TestHooks = Omit<{name}Hooks, {omitted}>{output};

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
  {inner}
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
        inner = inner_handler(fc, &create, &std, idempotent),
        key = ts_str(&svc.name),
    ));
}
