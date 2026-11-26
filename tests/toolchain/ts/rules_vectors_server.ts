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

// The application: `in<Type>` takes the value, `out<Type>` answers with the value of the case.
const impl = new Proxy(
  {},
  { get: (_target, name) => async () => (String(name).startsWith('out') ? current : undefined) },
) as unknown as gen.VectorsService;

const strict = gen.createVectorsHandler(impl, { onRuleViolation: (e) => events.push(e) });
const off = gen.createVectorsHandler(impl, { onRuleViolation: (e) => events.push(e) }, { rules: { output: 'off' } });

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
  return { name: c.name, status: res.status, violations: reported('rejected'), audited: reported('audited') };
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
