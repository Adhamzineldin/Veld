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

use crate::events;
use crate::rules::{loaders_name, service_has_builder, service_is_idempotent, service_uses_flags};
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

/// Writes `server/<service>-test-handler.ts` for every service with endpoint rules or with
/// subscriptions, when the option is on.
pub fn emit(files: &mut FileSet, tree: &Tree<'_>, testing: bool) -> Result<(), Vec<GenDiagnostic>> {
    if !testing {
        return Ok(());
    }
    for sid in &tree.services {
        let svc = tree.ir.service(*sid);
        if !service_has_builder(svc) && svc.subscriptions.is_empty() {
            continue;
        }
        let module = module(tree, svc);
        let mut fc = FileCtx::new(tree, &module);
        let mut w = CodeWriter::new();
        if service_has_builder(svc) {
            write_handler(&mut w, &mut fc, svc);
        } else {
            write_events_only_handler(&mut w, &mut fc, svc);
        }
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

/// Whether the production pipeline completes the principal of `svc` (its type has derived fields):
/// the test handler then hands it over in a middleware, before the completion, and `prime` leaves
/// it alone (spec/targets/typescript.md 5i).
fn completes_principal(fc: &FileCtx<'_, '_>, svc: &Service) -> bool {
    fc.tree.uses_derived()
        && svc
            .principal
            .as_ref()
            .is_some_and(|p| fc.tree.ir.type_reaches_derived(&Type::Decl(p.decl)))
}

/// `prime`: the clock and the principal of the request, as the test sent them. When the pipeline
/// completes the principal, the middleware `veld-test-principal` sets it before the completion and
/// `prime` only reads the clock.
fn prime_functions(ctx: &str, principal: &str, completes: bool) -> String {
    let clock = "  const now = header(ctx, 'veld-test-now');
  return now === undefined ? new Date() : new Date(now);
}";
    if completes {
        format!(
            "/** The principal of the request, as the test sent it: the production pipeline completes it afterwards. */
async function primePrincipal(ctx: {ctx}): Promise<void> {{
  const principal = decode(header(ctx, 'veld-test-principal'));
  if (principal !== undefined) ctx.principal = {principal};
}}

/** The clock of the request, as the test sent it. */
function prime(ctx: {ctx}): Date {{
{clock}"
        )
    } else {
        format!(
            "/** The clock and the principal of the request, as the test sent them. */
function prime(ctx: {ctx}): Date {{
  const principal = decode(header(ctx, 'veld-test-principal'));
  if (principal !== undefined) ctx.principal = {principal};
{clock}"
        )
    }
}

fn write_helpers(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service) {
    let ctx = fc.rt("RequestContext");
    let base64 = fc.rt("base64Decode");
    let schema = fc.rt("Schema");
    let from_wire = fc.rt("fromWire");
    let principal = decoded_principal(fc, svc);
    let prime = prime_functions(&ctx, &principal, completes_principal(fc, svc));
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

{prime}"
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

/// The part of the test handler that gives a request the flag provider of its header
/// (`Veld-Test-Flags`, `spec/rules.md` section 12.4). Only a service that uses a flag has it.
fn write_flags(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, idempotent: bool) {
    let request = fc.rt("VeldRequest");
    let memory = fc.rt("MemoryFlagProvider");
    // The header reader belongs to the idempotency part when there is one.
    if !idempotent {
        w.line(format!(
            "function requestHeader(req: {request}, name: string): string | undefined {{
  for (const key of Object.keys(req.headers)) {{
    if (key.toLowerCase() !== name) continue;
    const v = req.headers[key];
    return Array.isArray(v) ? v[0] : v;
  }}
  return undefined;
}}"
        ));
        w.blank();
    }
    w.line(format!(
        "/** `Veld-Test-Flags`: base64url JSON of the names of the flags that are on. Without the header every flag is off. */
function flagsOf(req: {request}): {memory} {{
  const raw = decode(requestHeader(req, 'veld-test-flags'));
  return new {memory}(Array.isArray(raw) ? raw.filter((n): n is string => typeof n === 'string') : []);
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
/// service with an `@idempotent` endpoint or a flag builds one per request, because the store and
/// the flags of a request come from its headers.
fn inner_handler(
    fc: &mut FileCtx<'_, '_>,
    create: &str,
    std: &str,
    per_request: &[(&str, &str)],
    completes: bool,
) -> String {
    let ctx = fc.rt("RequestContext");
    let mut clock = if fc.tree.uses_endpoint_rules() {
        format!(", clock: (ctx: {ctx}) => prime(ctx)")
    } else {
        String::new()
    };
    if completes {
        clock.push_str(", middleware: { 'veld-test-principal': primePrincipal }");
    }
    if per_request.is_empty() {
        return format!("const inner = {create}(endpoints, {std}, {{ ...hooks{clock} }});");
    }
    let handler = fc.rt("VeldHandler");
    let request = fc.rt("VeldRequest");
    let members = |req: &str| {
        per_request
            .iter()
            .map(|(name, make)| format!("{name}: {}", make.replace("REQ", req)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let (empty, own) = (
        members("{ headers: {}, method: 'GET', path: '/' }"),
        members("req"),
    );
    format!(
        "const base = {create}(endpoints, {std}, {{ ...hooks{clock}, {empty} }});
  const inner: {handler} = Object.assign(
    (req: {request}) => {create}(endpoints, {std}, {{ ...hooks{clock}, {own} }})(req),
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
    write_events(w, fc, svc);
    let idempotent = service_is_idempotent(svc);
    if idempotent {
        write_idempotency(w, fc);
    }
    let flagged = service_uses_flags(fc.tree, svc);
    if flagged {
        write_flags(w, fc, idempotent);
    }
    let mut per_request: Vec<(&str, &str)> = Vec::new();
    if idempotent {
        per_request.push(("idempotency", "idempotencyOf(REQ, endpoints)"));
    }
    if flagged {
        per_request.push(("flags", "flagsOf(REQ)"));
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
    let completes = completes_principal(fc, svc);
    let loaders_arg = if svc.facts.is_empty() {
        ""
    } else {
        ", loaders"
    };
    let mut omitted = String::from("'facts' | 'clock' | 'auth' | 'middleware'");
    if idempotent {
        omitted.push_str(" | 'idempotency'");
    }
    if flagged {
        omitted.push_str(" | 'flags'");
    }
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
  const copy: {endpoint} = {{ ...ep, middleware: {middleware}, call }};
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
      const body = JSON.stringify({{ veldTestHandler: 1, digests: {{ {key}: {digest} }}{subscribes} }});
      return {{ status: 200, headers: {{ 'content-type': 'application/json' }}, body }};
    }}
    {event_branch}{answer}
  }};
  return Object.assign(handler, {{ ws: inner.ws }});
}}",
        builder = builder_name(svc),
        middleware = if completes { "['veld-test-principal']" } else { "[]" },
        inner = inner_handler(fc, &create, &std, &per_request, completes),
        key = ts_str(&svc.name),
        subscribes = subscribes_member(fc.tree, svc),
        event_branch = event_branch(svc),
    ));
}

// ---- events (`spec/events.md` section 10.9) ---------------------------------------------------

/// The path a test handler that has subscribers answers to take one message.
const EVENT_PATH: &str = "/__veld/test/event";

/// `, subscribes: { "Service": ["topic", ...] }` for the probe of a service that subscribes, and
/// nothing for one that does not.
fn subscribes_member(tree: &Tree<'_>, svc: &Service) -> String {
    if svc.subscriptions.is_empty() {
        return String::new();
    }
    let mut topics: Vec<String> = svc
        .subscriptions
        .iter()
        .map(|s| ts_str(&tree.ir.channel(s.channel).topic))
        .collect();
    topics.sort();
    topics.dedup();
    format!(
        ", subscribes: {{ {}: [{}] }}",
        ts_str(&svc.name),
        topics.join(", ")
    )
}

/// The line of the handler that gives the event endpoint its requests.
fn event_branch(svc: &Service) -> String {
    if svc.subscriptions.is_empty() {
        return String::new();
    }
    format!(
        "if (req.method.toUpperCase() === 'POST' && req.path === '{EVENT_PATH}') return testEvent(req);\n    "
    )
}

/// `Service.handler` of every `on` line of the service, in source order.
fn handler_names(svc: &Service) -> Vec<String> {
    svc.subscriptions
        .iter()
        .flat_map(|sub| {
            sub.handlers
                .iter()
                .map(|h| format!("{}.{}", svc.name, h.name))
        })
        .collect()
}

/// The event endpoint of the test handler of a service that subscribes: it delivers the message
/// that many times to the generated consumer over a fresh memory transport, with a fresh memory
/// inbox, a `sleep` that does not wait and handlers that only count. The application's handlers
/// are not called.
fn write_events(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service) {
    if svc.subscriptions.is_empty() {
        return;
    }
    let name = naming::pascal(&svc.name);
    let module = events::module_of(fc.tree, svc);
    fc.import(&module, &format!("subscribe{name}Events"), true);
    let ns = fc.events_ns();
    let request = fc.rt("VeldRequest");
    let response = fc.rt("VeldResponse");
    let names: Vec<String> = handler_names(svc).iter().map(|n| ts_str(n)).collect();
    let handlers: Vec<String> = svc
        .subscriptions
        .iter()
        .flat_map(|sub| sub.handlers.iter())
        .map(|h| {
            format!(
                "{}: counting(called, {}),",
                method_key(&h.name),
                ts_str(&format!("{}.{}", svc.name, h.name))
            )
        })
        .collect();
    w.line(format!(
        "/** `Service.handler` of every `on` line of `{name}`. */
const handlerNames: readonly string[] = [{names}];

/** A handler that only counts its runs. */
function counting(called: Record<string, number>, name: string): () => void {{
  return () => {{
    called[name] = (called[name] ?? 0) + 1;
  }};
}}

/** A header of the event request, by its lower-case name. */
function eventHeader(req: {request}, name: string): string | undefined {{
  for (const key of Object.keys(req.headers)) {{
    if (key.toLowerCase() !== name) continue;
    const v = req.headers[key];
    return Array.isArray(v) ? v[0] : v;
  }}
  return undefined;
}}

/** The bytes of the message: the raw body when the adapter has it, the JSON of the parsed body otherwise. */
async function eventBytes(req: {request}): Promise<Uint8Array> {{
  const text = req.text !== undefined ? await req.text() : typeof req.body === 'string' ? req.body : JSON.stringify(req.body ?? null);
  const Encoder = (globalThis as {{ TextEncoder?: new () => {{ encode(text: string): Uint8Array }} }}).TextEncoder;
  if (Encoder === undefined) throw new Error('veld test handler: this runtime has no TextEncoder');
  return new Encoder().encode(text);
}}

/** The memory transport, counting what is published to a dead-letter topic. */
class CountingTransport extends {ns}.MemoryTransport {{
  copies = 0;

  publish(topic: string, key: string | null, bytes: Uint8Array): Promise<void> {{
    if (topic.endsWith('.dlq')) this.copies += 1;
    return super.publish(topic, key, bytes);
  }}
}}

/**
 * `POST {EVENT_PATH}` (spec/events.md section 10.9): the body is the message, `Veld-Test-Topic`
 * its topic, `Veld-Test-Key` its key and `Veld-Test-Deliveries` how often it is delivered (1 or 2).
 */
async function testEvent(req: {request}): Promise<{response}> {{
  const reply = (status: number, value: unknown): {response} => ({{
    status,
    headers: {{ 'content-type': 'application/json' }},
    body: JSON.stringify(value),
  }});
  const topic = eventHeader(req, 'veld-test-topic');
  if (topic === undefined) return reply(400, {{ error: 'Veld-Test-Topic is missing' }});
  const key = eventHeader(req, 'veld-test-key') ?? null;
  const deliveries = eventHeader(req, 'veld-test-deliveries') === '2' ? 2 : 1;
  const bytes = await eventBytes(req);
  const called: Record<string, number> = {{}};
  for (const n of handlerNames) called[n] = 0;
  const rejected: {ns}.EventRejected[] = [];
  const transport = new CountingTransport();
  const handle = await subscribe{name}Events(
    transport,
    {{
      {handlers}
    }},
    {{
      inbox: new {ns}.MemoryInbox(),
      sleep: () => Promise.resolve(),
      onEventRejected: (e) => {{
        rejected.push(e);
      }},
    }},
  );
  try {{
    for (let i = 0; i < deliveries; i += 1) {{
      await transport.publish(topic, key, bytes);
      await transport.idle();
    }}
  }} finally {{
    await handle.close();
  }}
  const first = rejected[0];
  const ran = handlerNames.some((n) => (called[n] ?? 0) > 0);
  return reply(200, {{
    outcome: first !== undefined ? 'rejected' : ran ? 'handled' : 'ignored',
    reason: first === undefined ? null : first.reason,
    called,
    violations: (first === undefined ? [] : first.violations).map((v) => ({{
      rule: v.rule,
      path: v.path,
      fields: v.fields,
      message: v.message,
    }})),
    deadLetters: transport.copies,
  }});
}}",
        names = names.join(", "),
        handlers = handlers.join("\n      "),
    ));
    w.blank();
}

/// The test handler of a service that subscribes and has no endpoint rules: it answers the probe
/// and the event endpoint, and nothing else. It takes no application, because it calls none.
fn write_events_only_handler(w: &mut CodeWriter, fc: &mut FileCtx<'_, '_>, svc: &Service) {
    let name = naming::pascal(&svc.name);
    write_events(w, fc, svc);
    let handler = fc.rt("VeldHandler");
    let request = fc.rt("VeldRequest");
    let response = fc.rt("VeldResponse");
    let digest = match svc.rules_digest.as_deref() {
        Some(d) => ts_str(d),
        None => "null".to_string(),
    };
    w.line(format!(
        "/**
 * The test handler of `{name}` (spec/events.md section 10.9): `GET /__veld/test` answers with the
 * topics it subscribes to, and `POST {EVENT_PATH}` delivers a message to its generated consumer.
 * The application's handlers are not called. NEVER mount it in production.
 */
export function create{name}TestHandler(): {handler} {{
  const log = (globalThis as {{ console?: {{ warn?: (message: string) => void }} }}).console;
  log?.warn?.('veld: the TEST HANDLER of {name} is active: it takes messages from request headers. Never mount it in production.');
  const handler = async (req: {request}): Promise<{response} | null> => {{
    if (req.method.toUpperCase() === 'GET' && req.path === '/__veld/test') {{
      const body = JSON.stringify({{ veldTestHandler: 1, digests: {{ {key}: {digest} }}{subscribes} }});
      return {{ status: 200, headers: {{ 'content-type': 'application/json' }}, body }};
    }}
    {event_branch}return null;
  }};
  return Object.assign(handler, {{ ws: () => Promise.resolve(false) }});
}}",
        key = ts_str(&svc.name),
        subscribes = subscribes_member(fc.tree, svc),
        event_branch = event_branch(svc),
    ));
}
