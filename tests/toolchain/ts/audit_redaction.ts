// Every case of tests/wire/redaction/cases.json through the redaction of the generated runtime
// (spec/rules.md section 19.3, tests/wire/redaction/README.md): the value with its `@pii` and
// `@secret` fields left out, as canonical JSON, must equal `redacted` byte for byte, and the paths
// of what was left out must equal `paths`. The contract has no service, and a server needs one
// that is audited (the redaction is part of the audit log): the Rust test appends it. Expects
// `./server` generated from it and the cases copied next to this file as `cases.json`.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as srv from './server/index';

interface Case {
  name: string;
  type: string;
  value: unknown;
  redacted: string;
  paths: string[];
}

const cases = (JSON.parse(readFileSync('cases.json', 'utf8')) as { cases: Case[] }).cases;

// The declarations of the contract by name: a model and an event have a companion, an error a class.
const types: Record<string, { schema: srv.veld.Schema }> = {
  Address: srv.Address,
  Guest: srv.Guest,
  GuestRegistered: srv.GuestRegistered,
  Order: srv.Order,
  Rejected: srv.Rejected,
  Staff: srv.Staff,
};

let ran = 0;
for (const c of cases) {
  const decl = types[c.type];
  assert.ok(decl !== undefined, `${c.name}: the contract has no declaration ${c.type}`);
  const paths: string[] = [];
  const kept = srv.veld.redactNode(decl.schema, c.value, '$', paths);
  assert.equal(srv.veld.canonical(kept), c.redacted, `${c.name}: the value that is kept`);
  assert.deepEqual([...new Set(paths)].sort(), c.paths, `${c.name}: the paths that were left out`);
  ran++;
}
assert.equal(ran, cases.length);
assert.ok(ran >= 10, 'the goldens have their cases');
console.log(`redaction goldens: ${ran} cases`);
