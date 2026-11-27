// The vector driver of the Node server (spec/rules.md section 15), the counterpart of rules_vectors.ts
// for a target that evaluates rules on the server and reports to `onRuleViolation`.
//
// Usage: node out/rules_vectors_server.js <vector file.json>
//
// Every case runs through the generated handler of the service `Vectors`: a case in request position is
// posted to `/v/in/<Type>` (step 5), any other case is the result of `GET /v/out/<Type>` (step 8), under
// the output mode the case names (`"output": "off"`) or `strict`. The driver prints one JSON line per case:
// `{ name, status, violations, audited }`, where `violations` are the events with the outcome `rejected`
// and `audited` those with the outcome `audited`, each the violation object of spec/rules.md section 7 with
// the path of the value (`$`, `$.rows[1]`). The Rust test compares the lines with the case's `expect` and
// `audited` as sets. The generated package is expected in `./gen`.

import * as fs from 'fs';
import * as gen from './gen/index';
import * as veld from './gen/runtime/veld';

interface Case {
  name: string;
  type: string;
  value: unknown;
  request?: boolean;
  partial?: boolean;
  output?: string;
  computed?: Record<string, unknown>;
  /** The flags that are on (spec/platform.md section 11); `provider: "failed"` makes the provider throw. */
  flags?: string[];
  provider?: string;
}

interface Checked {
  schema: veld.Schema;
}

function companionOf(type: string): Checked {
  const found = (gen as unknown as Record<string, Checked | undefined>)[type];
  if (found === undefined || found.schema === undefined) throw new Error(`no declaration named ${type}`);
  return found;
}

const events: veld.RuleViolationEvent[] = [];
let current: unknown;
/** The request body the handler of `in<Type>` was given: with its derived fields computed. */
let seen: unknown;

// The application: `in<Type>` takes the value, `out<Type>` answers with the value of the case.
const impl = new Proxy(
  {},
  {
    get: (_target, name) => async (req?: { body?: unknown }) => {
      if (String(name).startsWith('out')) return current;
      seen = req?.body;
      return undefined;
    },
  },
) as unknown as gen.VectorsService;

// The flag provider of a contract that uses flags (the hooks of one that does not have no such member).
let readings: { on: string[]; failed: boolean } = { on: [], failed: false };
const provider = {
  isOn(name: string): boolean {
    if (readings.failed) throw new Error('the flag provider failed');
    return readings.on.includes(name);
  },
};
const hooksOf = (extra: Record<string, unknown> = {}): gen.VectorsHooks =>
  ({ onRuleViolation: (e: veld.RuleViolationEvent) => events.push(e), flags: provider, onError: () => undefined, ...extra }) as unknown as gen.VectorsHooks;

const strict = gen.createVectorsHandler(impl, hooksOf());
const off = gen.createVectorsHandler(impl, hooksOf(), { rules: { output: 'off' } });
// Nothing is rejected here: the handler is reached by every case, and what it received (a request)
// or what it answered (a result, output mode `off`) shows what the server computed.
const lax = gen.createVectorsHandler(impl, hooksOf({ validateRequests: false, onRuleViolation: undefined }), { rules: { output: 'off' } });

/** The value at `$`, `$.name`, `$.items[1].line` of a wire value, `null` when there is none. */
function valueAt(root: unknown, path: string): unknown {
  let cur: unknown = root;
  const steps = path.slice(1).match(/\.[^.[]+|\[\d+\]/g) ?? [];
  for (const step of steps) {
    if (cur === null || typeof cur !== 'object') return null;
    const key = step.startsWith('[') ? Number(step.slice(1, -1)) : step.slice(1);
    cur = (cur as Record<string | number, unknown>)[key];
    if (cur === undefined) return null;
  }
  return cur ?? null;
}

/** What the server computed for the derived fields the case names, by the way the case reaches the server. */
async function computedOf(c: Case, decl: Checked): Promise<Record<string, unknown>> {
  const paths = Object.keys(c.computed ?? {});
  if (paths.length === 0) return {};
  let wire: unknown;
  if (c.request === true) {
    seen = undefined;
    await lax({ method: 'POST', path: `/v/in/${c.type}`, headers: { 'content-type': 'application/json' }, body: c.value });
    wire = seen === undefined ? undefined : veld.toWire(decl.schema, seen);
  } else {
    current = veld.fromWire(decl.schema, c.value);
    const res = await lax({ method: 'GET', path: `/v/out/${c.type}`, headers: {} });
    wire = res?.body == null ? undefined : (JSON.parse(res.body) as unknown);
  }
  const out: Record<string, unknown> = {};
  for (const path of paths) out[path] = valueAt(wire, path);
  return out;
}

/** The path of the value that carries the rule: `body.rows[1]` and `result.rows[1]` are `$.rows[1]`. */
function valuePath(path: string): string {
  for (const root of ['body', 'result']) {
    if (path === root || path.startsWith(`${root}.`) || path.startsWith(`${root}[`)) return `$${path.slice(root.length)}`;
  }
  return path;
}

function reported(outcome: 'rejected' | 'audited'): veld.Violation[] {
  return events.filter((e) => e.outcome === outcome).map((e) => ({ ...e.violation, path: valuePath(e.violation.path) }));
}

async function run(c: Case): Promise<unknown> {
  events.length = 0;
  readings = { on: c.flags ?? [], failed: c.provider === 'failed' };
  const decl = companionOf(c.type);
  const handler = c.output === 'off' ? off : strict;
  let res: veld.VeldResponse | null;
  if (c.request === true) {
    res = await handler({
      method: 'POST',
      path: `/v/in/${c.type}`,
      headers: { 'content-type': 'application/json' },
      body: c.value,
    });
  } else {
    current = veld.fromWire(decl.schema, c.value);
    res = await handler({ method: 'GET', path: `/v/out/${c.type}`, headers: {} });
  }
  if (res === null) throw new Error('no route');
  const answer = { name: c.name, status: res.status, violations: reported('rejected'), audited: reported('audited') };
  return { ...answer, computed: await computedOf(c, decl) };
}

async function main(): Promise<void> {
  const file = process.argv[2];
  if (file === undefined) throw new Error('usage: rules_vectors_server <vector file>');
  const cases = (JSON.parse(fs.readFileSync(file, 'utf8')) as { cases: Case[] }).cases;
  for (const c of cases) {
    if (c.partial === true) continue; // the partial check is a client's
    try {
      console.log(JSON.stringify(await run(c)));
    } catch (e) {
      console.log(JSON.stringify({ name: c.name, error: String(e) }));
    }
  }
}

void main();
