// Every case of tests/wire/audit/cases.json through the generated server (spec/platform.md
// section 4, tests/wire/audit/README.md). Each request is sent to the production handler of its
// service, built with the stubs the case describes: the auth hook answers `principal` (`null` is a
// refusal, `401`), the loaders answer `facts`, the handler answers `returns`, the clock is `clock`.
// The `audit` hook records the event, and its RFC 8785 canonical JSON must equal `event` byte for
// byte; an endpoint that is not audited must not call it. Expects `./server` generated from
// tests/wire/audit/contract.veld (compiled as the file `t.veld` of the package `bookings`) and the
// cases copied next to this file as `cases.json`.
//
// The contract of the goldens has no `@limit`, and a refused call needs one: the Rust test adds
// `@limit(10, per: minute, by: user)` to `payBooking`, which changes no signature, and the limiter
// of this program answers what the case says `limiter` is (`null`: the call is allowed).
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as srv from './server/index';

interface Case {
  name: string;
  service: 'Bookings' | 'Public';
  endpoint: string;
  request: { method: string; path: string; query: Record<string, string>; headers: Record<string, string>; body?: unknown };
  clock: string;
  principal: unknown;
  facts: Record<string, unknown>;
  returns: unknown;
  limiter: unknown;
  status: number;
  error: string | null;
  event: string | null;
}

const cases = (JSON.parse(readFileSync('cases.json', 'utf8')) as { cases: Case[] }).cases;

let current: Case | undefined;
const events: srv.veld.AuditEvent[] = [];
const here = (): Case => {
  assert.ok(current !== undefined);
  return current;
};

// Every handler answers what the case says `returns`; the case that does not reach it says `null`.
const impl = new Proxy(
  {},
  {
    get: (_target, name) => async () => (name === here().endpoint && here().returns !== null ? here().returns : undefined),
  },
) as unknown as srv.BookingsService & srv.PublicService;

const common = {
  limiter: {
    take: () => {
      const c = here();
      return c.limiter === null ? { allowed: true, retryAfterSeconds: 0 } : (c.limiter as { allowed: boolean; retryAfterSeconds: number });
    },
  },
  clock: () => new Date(here().clock),
  audit: (event: srv.veld.AuditEvent) => {
    events.push(event);
  },
};
const handlers = {
  Bookings: srv.createBookingsHandler(impl, {
    auth: { bearer: async () => here().principal },
    caller: async (principal: { id: string }) => principal.id,
    facts: { booking: async () => here().facts['booking'] ?? undefined },
    ...common,
  } as unknown as srv.BookingsHooks),
  Public: srv.createPublicHandler(impl, common as unknown as srv.PublicHooks),
};

async function main(): Promise<void> {
  let ran = 0;
  for (const c of cases) {
    current = c;
    events.length = 0;
    assert.deepEqual(c.request.query, {}, `${c.name}: the cases send no query`);
    const headers = { authorization: 'Bearer test', ...c.request.headers };
    const res = await handlers[c.service]({
      method: c.request.method,
      path: c.request.path,
      headers,
      ...(c.request.body === undefined ? {} : { body: c.request.body }),
    });
    assert.ok(res !== null, `${c.name}: no route matched`);
    assert.equal(res.status, c.status, `${c.name}: the status`);
    if (c.error !== null) {
      const doc = JSON.parse(res.body ?? '{}') as { title?: string };
      assert.equal(doc.title, c.error, `${c.name}: the error`);
    }
    if (c.event === null) {
      assert.equal(events.length, 0, `${c.name}: an endpoint that is not audited calls no hook`);
    } else {
      assert.equal(events.length, 1, `${c.name}: the hook is called once`);
      assert.equal(srv.veld.canonical(events[0]), c.event, `${c.name}: the event`);
    }
    ran++;
  }
  assert.equal(ran, cases.length, 'every case ran');
  assert.ok(ran >= 15, 'the goldens have their cases');
  console.log(`audit goldens: ${ran} cases`);
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
