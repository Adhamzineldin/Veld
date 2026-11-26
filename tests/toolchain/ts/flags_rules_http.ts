// Flagged invariants over the production handler and the client's local check (spec/platform.md
// section 11). Expects `./server` and `./client` generated from
// tests/toolchain/contract/platform_p2/flags_rules.veld.
import assert from 'node:assert/strict';
import * as cli from './client/index';
import * as srv from './server/index';
import type { RuleViolationEvent, VeldRequest } from './server/runtime/veld';

const wire = (guests: number): Record<string, unknown> => ({ start: '2026-01-01T00:00:00Z', end: '2026-03-01T00:00:00Z', guests });
let result: Record<string, unknown> = wire(2);

const impl: srv.VisitsService = {
  async bookStay() {
    return undefined;
  },
  async getStay() {
    return srv.Stay.fromWire(result);
  },
};

class Provider implements srv.veld.FlagProvider {
  asked: string[] = [];
  failing = false;
  constructor(readonly on: string[]) {}
  isOn(name: string): boolean {
    this.asked.push(name);
    if (this.failing) throw new Error('the provider failed');
    return this.on.includes(name);
  }
}

function make(provider: Provider, events: RuleViolationEvent[] = []): srv.veld.VeldHandler {
  return srv.createVisitsHandler(impl, { flags: provider, onRuleViolation: (e) => events.push(e), onError: () => undefined });
}

function book(body: unknown): VeldRequest {
  return { method: 'POST', path: '/stays', headers: { 'content-type': 'application/json' }, body };
}

async function status(handler: srv.veld.VeldHandler, req: VeldRequest): Promise<{ status: number; rules: string[] }> {
  const res = await handler(req);
  const doc = JSON.parse(res?.body ?? '{}') as { violations?: { rule: string }[] };
  return { status: res?.status ?? 0, rules: (doc.violations ?? []).map((v) => v.rule) };
}

async function main(): Promise<void> {
  // A request: the flagged invariant is not there while its flag is off, and applies once it is on.
  assert.deepEqual(await status(make(new Provider([])), book(wire(9))), { status: 204, rules: [] });
  const small = await status(make(new Provider(['smallGroups'])), book(wire(9)));
  assert.equal(small.status, 422);
  assert.equal(small.rules.length, 1);
  assert.equal((await status(make(new Provider(['smallGroups'])), book(wire(3)))).status, 204);
  // Two flags are independent; both on report both rules.
  const both = await status(make(new Provider(['smallGroups', 'shortStays'])), book(wire(9)));
  assert.equal(both.status, 422);
  assert.equal(both.rules.length, 2);
  // An invariant without a flag always applies.
  assert.equal((await status(make(new Provider([])), book(wire(0)))).status, 422);
  // A failing provider: an invariant counts as on, and the failure is read once per flag.
  const failing = new Provider([]);
  failing.failing = true;
  const broken = await status(make(failing), book(wire(9)));
  assert.equal(broken.status, 422);
  assert.equal(broken.rules.length, 2);
  assert.deepEqual([...failing.asked].sort(), ['shortStays', 'smallGroups']);

  // A result: strict output mode refuses a result that breaks a flagged invariant that is on.
  result = wire(9);
  const get: VeldRequest = { method: 'GET', path: '/stays/one', headers: {} };
  assert.equal((await make(new Provider([]))(get))?.status, 200);
  assert.equal((await make(new Provider(['smallGroups']))(get))?.status, 500);

  // One reading per request, however many values have the invariant.
  const counting = new Provider(['smallGroups']);
  const handler = make(counting);
  await handler(book(wire(2)));
  await handler(book(wire(2)));
  assert.deepEqual(counting.asked.filter((n) => n === 'smallGroups').length, 2);

  // The client's local check has no provider: it skips every flagged invariant, in `check` and `partial`.
  const stay = srv.Stay.fromWire(wire(9));
  assert.deepEqual(cli.Stay.check(cli.Stay.fromWire(wire(9))).map((v) => v.rule), []);
  assert.deepEqual(cli.Stay.partial(stay).map((v) => v.rule), []);
  assert.equal(cli.Stay.check(cli.Stay.fromWire(wire(0))).length, 1, 'an invariant without a flag still applies');
  // The client sends what the application gives it: nothing is rejected locally for a flagged rule.
  console.log('flags_rules_http ok');
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
