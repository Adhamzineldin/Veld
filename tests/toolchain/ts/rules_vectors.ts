// The vector driver of the TypeScript target (spec/rules.md section 15).
//
// Usage: node out/rules_vectors.js <vector file.json>
//
// For every case of the file it decodes `value` from wire form as the case's `type`, runs the check in the
// case's mode (full, `request`, or `partial`) and prints one JSON line per case: `{ name, violations }`, where
// a violation is the object of spec/rules.md section 7, or `{ name, error }` when the check itself threw. `computed` holds the wire value at each path of the case's `computed` (`null` for none): the derived fields of spec/rules.md section 17.2.
// The Rust test compares the lines with the expected violations (`veld_rules::vectors::compare_produced`).
// The generated package is expected in `./gen`.

import * as fs from 'fs';
import * as gen from './gen/index';
import * as veld from './gen/runtime/veld';

interface Case {
  name: string;
  type: string;
  value: unknown;
  request?: boolean;
  partial?: boolean;
  computed?: Record<string, unknown>;
  /** The flags that are on (spec/platform.md section 11); `provider: "failed"` is a provider that failed. */
  flags?: string[];
  provider?: string;
}

/** `runtime/flags.ts`: there only when the contract has a flagged statement. */
interface FlagsApi {
  withReadings<T>(readings: (name: string) => 'on' | 'off' | 'failed', body: () => T): T;
}

function flagsApi(): FlagsApi | undefined {
  try {
    return require('./gen/runtime/flags') as FlagsApi;
  } catch {
    return undefined;
  }
}
const flagging = flagsApi();

interface Checked {
  schema: veld.Schema;
  partial(draft: unknown): veld.Violation[];
}

/** `deriveValue` of the runtime: there only when the contract has a derived field (spec/rules.md section 17.2). */
const derive = (veld as unknown as Record<string, unknown>)['deriveValue'] as
  | ((schema: veld.Schema, value: unknown, path: string, out: veld.Violation[]) => unknown)
  | undefined;

function companionOf(type: string): Checked {
  const found = (gen as unknown as Record<string, Checked | undefined>)[type];
  if (found === undefined || found.schema === undefined) throw new Error(`no declaration named ${type}`);
  return found;
}

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

function run(c: Case): { violations: veld.Violation[]; computed: Record<string, unknown> } {
  const decl = companionOf(c.type);
  const value = veld.fromWire(decl.schema, c.value, { request: c.request === true });
  const failed: veld.Violation[] = [];
  // What the derived fields of the value are: the same walk the server runs (`Companion.compute`).
  const done = derive === undefined ? value : derive(decl.schema, value, '$', failed);
  const full = (): veld.Violation[] => [
    ...failed,
    ...veld.checkValue(decl.schema, done, '$', c.request === true ? 'request' : 'full'),
  ];
  // The partial check of a client has no provider: it skips every flagged invariant. Any other check
  // runs with the readings of the case, as a server's request would have them.
  const reading = (name: string): 'on' | 'off' | 'failed' =>
    c.provider === 'failed' ? 'failed' : (c.flags ?? []).includes(name) ? 'on' : 'off';
  const violations =
    c.partial === true
      ? decl.partial(value)
      : flagging === undefined
        ? full()
        : flagging.withReadings(reading, full);
  const wire = veld.toWire(decl.schema, done);
  const computed: Record<string, unknown> = {};
  for (const path of Object.keys(c.computed ?? {})) computed[path] = valueAt(wire, path);
  return { violations, computed };
}

const file = process.argv[2];
if (file === undefined) throw new Error('usage: rules_vectors <vector file>');
const cases = (JSON.parse(fs.readFileSync(file, 'utf8')) as { cases: Case[] }).cases;
for (const c of cases) {
  try {
    console.log(JSON.stringify({ name: c.name, ...run(c) }));
  } catch (e) {
    console.log(JSON.stringify({ name: c.name, error: String(e) }));
  }
}
