// The vector driver of the TypeScript target (spec/rules.md section 15).
//
// Usage: node out/rules_vectors.js <vector file.json>
//
// For every case of the file it decodes `value` from wire form as the case's `type`, runs the check in the
// case's mode (full, `request`, or `partial`) and prints one JSON line per case: `{ name, violations }`, where
// a violation is the object of spec/rules.md section 7, or `{ name, error }` when the check itself threw.
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
}

interface Checked {
  schema: veld.Schema;
  partial(draft: unknown): veld.Violation[];
}

function companionOf(type: string): Checked {
  const found = (gen as unknown as Record<string, Checked | undefined>)[type];
  if (found === undefined || found.schema === undefined) throw new Error(`no declaration named ${type}`);
  return found;
}

function run(c: Case): veld.Violation[] {
  const decl = companionOf(c.type);
  const value = veld.fromWire(decl.schema, c.value, { request: c.request === true });
  if (c.partial === true) return decl.partial(value);
  return veld.checkValue(decl.schema, value, '$', c.request === true ? 'request' : 'full');
}

const file = process.argv[2];
if (file === undefined) throw new Error('usage: rules_vectors <vector file>');
const cases = (JSON.parse(fs.readFileSync(file, 'utf8')) as { cases: Case[] }).cases;
for (const c of cases) {
  try {
    console.log(JSON.stringify({ name: c.name, violations: run(c) }));
  } catch (e) {
    console.log(JSON.stringify({ name: c.name, error: String(e) }));
  }
}
