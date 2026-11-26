// Every case of tests/wire/visibility/cases.json through the generated runtime (spec/platform.md
// section 2, tests/wire/visibility/README.md): the value after visibility, canonical, must equal
// `shown` byte for byte, and the paths of what was dropped must equal `hidden`. Expects `./server`
// generated from tests/wire/visibility/contract.veld with a service appended (a server needs
// one), and the cases copied next to this file as `cases.json`.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as srv from './server/index';

interface Case {
  name: string;
  type: string;
  caller: unknown;
  now: string;
  value: unknown;
  shown: string;
  hidden: string[];
}

const cases = (JSON.parse(readFileSync('cases.json', 'utf8')) as { cases: Case[] }).cases;

// The declarations of the contract by name: the companion of a model or the schema of an error.
const types: Record<string, { sig: string; schema: srv.veld.Schema }> = {
  Address: srv.Address,
  Guest: srv.Guest,
  Order: srv.Order,
  Vip: srv.Vip,
  Rejected: { sig: srv.Rejected.sig, schema: srv.Rejected.schema },
};

/** `$.a[0].b` for the unrooted `a[0].b` of the runtime, `$[0].b` for `[0].b`. */
function rooted(path: string): string {
  return path === '' ? '$' : path.startsWith('[') ? `$${path}` : `$.${path}`;
}

let ran = 0;
for (const c of cases) {
  const decl = types[c.type];
  assert.ok(decl !== undefined, `${c.name}: the contract has no declaration ${c.type}`);
  // No field of the contract has a wire name of its own, so a wire value is read as it is: the
  // members the contract does not declare stay, as the reference keeps them.
  const caller = c.caller === null ? undefined : { model: srv.User.sig, value: c.caller };
  const now = BigInt(Date.parse(c.now));
  const shown = srv.veld.showValue(decl.schema, c.value, caller, now);
  assert.equal(srv.veld.canonical(shown.value), c.shown, `${c.name}: the value that is sent`);
  assert.deepEqual(shown.hidden.map(rooted), c.hidden, `${c.name}: the paths that were dropped`);
  ran++;
}
assert.equal(ran, cases.length);
assert.ok(ran >= 20, 'the goldens have their cases');
console.log(`visibility goldens: ${ran} cases`);
