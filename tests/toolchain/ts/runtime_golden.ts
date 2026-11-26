// The goldens of tests/wire/runtime-config/cases.json through the generated runtime modules: one
// generated target per model of the file (`m_server`, `m_client`, `m_ordering`, `m_rules_only`).
// Every case of `from: "value"` goes through `checkRuntime` and every case of `from: "env"` through
// `fromEnv`; values, problems and the text of the error are compared with the file. The `from: "map"`
// cases are for the targets that have a flat map reader (`fromMap`); TypeScript has none (spec
// `runtime-config.md` 3.1), so they are counted and skipped. Also the server entry of a target whose
// only setting is optional. The variable `CASES` names the file.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as server from './m_server/runtime';
import * as client from './m_client/runtime';
import * as ordering from './m_ordering/runtime';
import * as rulesOnly from './m_rules_only/runtime';
import { createOrdersHandler } from './m_rules_only/server/orders';
import type { OrdersService } from './m_rules_only/server/orders';

interface Module {
  checkRuntime(value: unknown): unknown;
  fromEnv(names: never, env?: Record<string, string | undefined>): unknown;
  VeldConfigError: new (...args: never[]) => Error & { problems: { path: string; message: string }[] };
}

const MODELS: Record<string, Module> = {
  server: server as unknown as Module,
  client: client as unknown as Module,
  ordering: ordering as unknown as Module,
  rules_only: rulesOnly as unknown as Module,
};

interface Problem {
  path: string;
  message: string;
}
interface Case {
  name: string;
  model: string;
  from: 'value' | 'map' | 'env';
  input?: unknown;
  names?: Record<string, string>;
  env?: Record<string, string>;
  expect: { values: Record<string, string> } | { problems: Problem[]; error: string };
}
interface Cases {
  urls: { input: string; normalized: string | null }[];
  cases: Case[];
}

/** `{ services: { iam: { url } } }` as `{ 'services.iam.url': url }`. */
function flatten(value: unknown, prefix = ''): Record<string, string> {
  const out: Record<string, string> = {};
  if (typeof value === 'string') {
    out[prefix] = value;
  } else if (typeof value === 'object' && value !== null) {
    for (const [key, inner] of Object.entries(value)) {
      Object.assign(out, flatten(inner, prefix === '' ? key : `${prefix}.${key}`));
    }
  }
  return out;
}

function outcome(m: Module, c: Case): unknown {
  try {
    const value = c.from === 'env' ? m.fromEnv((c.names ?? {}) as never, c.env ?? {}) : m.checkRuntime(c.input);
    return { values: flatten(value) };
  } catch (e) {
    assert.ok(e instanceof m.VeldConfigError, `${c.name}: ${String(e)}`);
    assert.equal(e.name, 'VeldConfigError');
    return { problems: e.problems.map((p) => ({ path: p.path, message: p.message })), error: e.message };
  }
}

const file = JSON.parse(readFileSync(process.env['CASES'] ?? '', 'utf8')) as Cases;

let checked = 0;
let skipped = 0;
for (const c of file.cases) {
  if (c.from === 'map') {
    skipped += 1;
    continue;
  }
  const m = MODELS[c.model];
  assert.ok(m, `${c.name}: no generated target for the model ${c.model}`);
  assert.deepEqual(outcome(m, c), c.expect, c.name);
  checked += 1;
}
assert.ok(checked > 0 && skipped > 0, 'the cases were read');

// The URL rule, case by case, through a client target: one URL under test, the other one fine.
for (const u of file.urls) {
  const run = (): unknown => client.checkRuntime({ services: { bookings: { url: u.input }, iam: { url: 'http://iam' } } });
  if (u.normalized === null) {
    assert.throws(run, (e: unknown) => e instanceof client.VeldConfigError && e.problems.length === 1 && e.problems[0]?.message === 'services.bookings.url must be an http or https URL', u.input);
  } else {
    assert.equal(flatten(run())['services.bookings.url'], u.normalized, u.input);
  }
}

// A target whose only setting is optional: the runtime config may be left out, and a bad one fails when the
// handler is created, with the whole list.
const impl: OrdersService = {
  async create(req) {
    return req.body;
  },
  async ping() {
    return 1;
  },
};
assert.equal(typeof createOrdersHandler(impl), 'function', 'no runtime config: the default mode');
assert.equal(typeof createOrdersHandler(impl, {}, { rules: { output: 'off' } }), 'function');
assert.throws(
  () => createOrdersHandler(impl, {}, { rules: { output: 'loud' } } as never),
  (e: unknown) => e instanceof rulesOnly.VeldConfigError && e.message === 'invalid runtime config: rules.output must be "strict", "audit" or "off"',
);
assert.throws(
  () => createOrdersHandler(impl, {}, { rules: { ouput: 'off' }, other: 1 } as never),
  (e: unknown) => e instanceof rulesOnly.VeldConfigError && e.message === 'invalid runtime config: other is not a setting; rules.ouput is not a setting',
);
// The check also runs when the runtime config is `null` (an absent value).
assert.equal(typeof createOrdersHandler(impl, {}, null as never), 'function');

process.stdout.write(`runtime golden ok: ${String(checked)} cases, ${String(skipped)} flat-map cases skipped\n`);
