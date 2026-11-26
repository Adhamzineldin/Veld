// The capture of `onRuleViolation` (spec/rules.md section 17.3) over the production handler of
// tests/toolchain/contract/platform_p4/capture.veld, generated with `capture-violations = true`:
// an input violation with the principal and `@pii` values at two depths, a failing `require` with
// a fact, an optional fact (found and not found), a `@secret` in the body, the fact and the
// principal, an output violation (`ensure`), an endpoint without auth, the option turned off, a
// fact that cannot be walked. Every event that carries a capture is written to `captures.json`
// for the Rust test, which gives each to `veld_testing::from_violation`. Prints `capture http ok`.
import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import * as srv from './server/index';

const ID = '00000000-0000-4000-8000-000000000001';
const COUPON = '00000000-0000-4000-8000-0000000000cc';
const USER = { id: '00000000-0000-4000-8000-0000000000aa', email: 'me@example.test', password: 'hunter2', roles: [] as string[] };
const NOW = '2026-06-01T00:00:00Z';

const booking = (extra: Record<string, unknown> = {}): srv.Booking =>
  ({ id: ID, guest: { name: 'Ada', phone: '+20100000000' }, total: '10.00', card: { number: '4111', holder: 'Ada' }, ...extra }) as srv.Booking;

let loaded: { booking?: unknown; discount?: unknown } = { booking: booking(), discount: booking() };
let overTotal = '-1.00';
const events: srv.veld.RuleViolationEvent[] = [];
const failures: unknown[] = [];

const impl: srv.BookingsService = {
  create: async (req) => req.body,
  pay: async () => booking(),
  total: async () => booking({ total: overTotal }),
  openStay: async (req) => req.body,
};

function bookings(extra: Record<string, unknown> = {}): srv.veld.VeldHandler {
  const hooks = {
    auth: { bearer: async (token: string) => (token === 'me' ? USER : null) },
    facts: { booking: async () => loaded.booking, discount: async () => loaded.discount },
    clock: () => new Date(NOW),
    onRuleViolation: (e: srv.veld.RuleViolationEvent) => void events.push(e),
    onError: (e: unknown) => void failures.push(e),
    ...extra,
  } as srv.BookingsHooks;
  return srv.createBookingsHandler(impl, hooks);
}

async function send(handler: srv.veld.VeldHandler, method: string, path: string, body?: unknown, token = 'me'): Promise<srv.veld.VeldResponse> {
  const res = await handler({ method, path, headers: { authorization: `Bearer ${token}` }, ...(body === undefined ? {} : { body }) });
  assert.ok(res !== null, `${method} ${path} matched no route`);
  return res;
}

function only(): srv.veld.RuleViolationEvent {
  assert.equal(events.length, 1, `one event, got ${String(events.length)}`);
  const [event] = events;
  assert.ok(event !== undefined);
  return event;
}

const stay = (extra: Record<string, unknown> = {}): Record<string, unknown> => ({
  guests: 2,
  start: '2026-06-12T00:00:00Z',
  end: '2026-06-10T00:00:00Z',
  guest: { name: 'Ada', phone: '+20100000000' },
  companions: [{ name: 'Bo' }, { name: 'Cy', phone: '+20100000001' }],
  ...extra,
});

/** What `from_violation` reads: the event, by the case it is for. */
const written: { name: string; kind: 'input' | 'output'; event: unknown }[] = [];
const keep = (name: string, kind: 'input' | 'output', event: srv.veld.RuleViolationEvent): void => {
  written.push({ name, kind, event: JSON.parse(JSON.stringify(event)) });
};

const OLD_KEYS = ['elapsedMicros', 'operation', 'outcome', 'position', 'rule', 'violation'];

async function main(): Promise<void> {
  const handler = bookings();

  // An input violation: the body with `@pii` values at two depths, and the principal.
  let res = await send(handler, 'POST', '/bookings', stay());
  assert.equal(res.status, 422);
  let event = only();
  assert.equal(event.position, 'input');
  assert.equal(event.outcome, 'rejected');
  assert.ok(event.capture !== undefined, 'the event has a capture');
  assert.deepEqual(event.capture, {
    service: 'Bookings',
    endpoint: 'create',
    operation: event.operation,
    params: {
      body: {
        guests: 2,
        start: '2026-06-12T00:00:00Z',
        end: '2026-06-10T00:00:00Z',
        guest: { name: 'Ada' },
        companions: [{ name: 'Bo' }, { name: 'Cy' }],
      },
    },
    facts: {},
    principal: { id: USER.id, roles: [] },
    now: NOW,
    redacted: ['params.body.companions[1].phone', 'params.body.guest.phone', 'principal.email', 'principal.password'],
  });
  assert.ok(event.operation.startsWith('veld:bookings/Bookings.create@'));
  assert.equal(event.violation.message, 'The stay must end after it starts');
  keep('an input violation with the principal and pii at two depths', 'input', event);

  // The caller is sent what the contract says, whatever the capture leaves out.
  assert.equal(JSON.parse(res.body ?? '{}').violations.length, 1);

  // A `require` that fails with a fact, an optional fact that is found, the principal and secrets
  // in the body, the fact and the principal.
  events.length = 0;
  res = await send(handler, 'POST', `/bookings/${ID}/pay?coupon=${COUPON}`, { amount: '99.00', card: { number: '4242', holder: 'Ada' } });
  assert.equal(res.status, 422);
  event = only();
  assert.equal(event.position, 'input');
  assert.equal(event.violation.message, 'The amount is more than the booking costs');
  assert.deepEqual(event.capture, {
    service: 'Bookings',
    endpoint: 'pay',
    operation: event.operation,
    params: { id: ID, body: { amount: '99.00', card: { holder: 'Ada' } }, coupon: COUPON },
    facts: {
      booking: { id: ID, guest: { name: 'Ada' }, total: '10.00', card: { holder: 'Ada' } },
      discount: { id: ID, guest: { name: 'Ada' }, total: '10.00', card: { holder: 'Ada' } },
    },
    principal: { id: USER.id, roles: [] },
    now: NOW,
    redacted: [
      'facts.booking.card.number',
      'facts.booking.guest.phone',
      'facts.discount.card.number',
      'facts.discount.guest.phone',
      'params.body.card.number',
      'principal.email',
      'principal.password',
    ],
  });
  keep('a require with facts, an optional fact and secrets', 'input', event);

  // The optional fact is not asked for (no coupon) and is not found (the loader says so): `null`.
  events.length = 0;
  res = await send(handler, 'POST', `/bookings/${ID}/pay`, { amount: '99.00', card: { number: '4242', holder: 'Ada' } });
  event = only();
  const noCoupon = event.capture;
  assert.ok(noCoupon !== undefined);
  assert.equal(noCoupon.facts['discount'], null, 'an optional fact that was not asked for is null');
  assert.ok(!('coupon' in noCoupon.params), 'a parameter that was not sent is absent');
  keep('an optional fact that is not loaded', 'input', event);
  events.length = 0;
  loaded = { booking: booking(), discount: null };
  await send(handler, 'POST', `/bookings/${ID}/pay?coupon=${COUPON}`, { amount: '99.00', card: { number: '4242', holder: 'Ada' } });
  event = only();
  assert.equal(event.capture?.facts['discount'], null, 'an optional fact the loader did not find is null');
  assert.equal(event.capture?.params['coupon'], COUPON);
  keep('an optional fact the loader did not find', 'input', event);
  loaded = { booking: booking(), discount: booking() };

  // An output violation (`ensure`): the capture holds what the request had.
  events.length = 0;
  res = await send(handler, 'GET', `/bookings/${ID}/total`);
  assert.equal(res.status, 500);
  event = only();
  assert.equal(event.position, 'output');
  assert.equal(event.violation.message, 'The total is never negative');
  assert.deepEqual(event.capture?.params, { id: ID });
  assert.deepEqual(Object.keys(event.capture?.facts ?? {}), ['booking']);
  assert.deepEqual(event.capture?.redacted, ['facts.booking.card.number', 'facts.booking.guest.phone', 'principal.email', 'principal.password']);
  keep('an output violation', 'output', event);

  // No auth, no principal, no fact: `principal` is null.
  events.length = 0;
  res = await handler({ method: 'POST', path: '/bookings/open', headers: {}, body: stay() }) ?? res;
  event = only();
  assert.equal(event.capture?.principal, null);
  assert.deepEqual(event.capture?.facts, {});
  assert.deepEqual(event.capture?.redacted, ['params.body.companions[1].phone', 'params.body.guest.phone']);
  keep('no auth and no principal', 'input', event);

  // The clock is the one reading of the request: what a rule that reads `now` would see.
  assert.equal(event.capture?.now, NOW);
  const fraction = bookings({ clock: () => new Date('2026-06-01T10:20:30.045Z') });
  events.length = 0;
  await send(fraction, 'POST', '/bookings', stay());
  assert.equal(only().capture?.now, '2026-06-01T10:20:30.045Z');

  // The option is off: the event is what it was before, with the same members, and a handler
  // without a hook builds nothing.
  events.length = 0;
  await send(bookings({ captureViolations: false }), 'POST', '/bookings', stay());
  event = only();
  assert.deepEqual(Object.keys(event).sort(), OLD_KEYS);
  assert.equal('capture' in event, false);
  events.length = 0;
  const silent = bookings({ onRuleViolation: undefined });
  res = await send(silent, 'POST', '/bookings', stay());
  assert.equal(res.status, 422);
  assert.equal(events.length, 0);

  // A fact that cannot be walked cannot be known to be clean: the event has no capture, and
  // `onError` hears why. The answer is the same.
  events.length = 0;
  failures.length = 0;
  loaded = { booking: { id: ID, guest: 'not an object', total: '10.00' }, discount: null };
  res = await send(handler, 'POST', `/bookings/${ID}/pay`, { amount: '99.00', card: { number: '4242', holder: 'Ada' } });
  assert.equal(res.status, 422);
  event = only();
  assert.equal('capture' in event, false);
  assert.equal(failures.length, 1);
  assert.deepEqual(Object.keys(event).sort(), OLD_KEYS);

  // A hook that throws never changes the answer.
  loaded = { booking: booking(), discount: booking() };
  const throwing = bookings({ onRuleViolation: () => { throw new Error('the hook failed'); } });
  res = await send(throwing, 'POST', '/bookings', stay());
  assert.equal(res.status, 422);

  writeFileSync('captures.json', JSON.stringify(written));
  process.stdout.write('capture http ok\n');
}

main().catch((e: unknown) => {
  process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  process.exit(1);
});
