// Tenancy (spec/platform.md section 3) over the production handler of
// tests/toolchain/contract/platform_p1/tenancy.veld: a fact of another tenant is not found (404,
// absent for an optional one); a value of another tenant in a result, a list, a page, a declared
// error and a stream item is never sent (500, nothing in the body, one `onRuleViolation` event per
// value); a caller without a tenant owns nothing; a `@tenant` member of a request may be left out
// and is kept as received; no setting turns it off. Expects `./server` generated from that
// contract (compiled as the package `bookings`).
import assert from 'node:assert/strict';
import * as srv from './server/index';

const MINE = '00000000-0000-4000-8000-0000000000aa';
const OTHER = '00000000-0000-4000-8000-0000000000bb';
const ID = '00000000-0000-4000-8000-000000000001';
const TENANT_RULE = srv.Booking.schema.k === 'object' ? (srv.Booking.schema.tn?.id ?? '') : '';

const principals: Record<string, { id: string; tenantId?: string }> = {
  mine: { id: '00000000-0000-4000-8000-0000000000c1', tenantId: MINE },
  none: { id: '00000000-0000-4000-8000-0000000000c2' },
};

function booking(tenantId: string | undefined, over: Partial<srv.Booking> = {}): srv.Booking {
  return { id: ID, tenantId: tenantId as string, title: 't', open: true, ...over };
}

let facts: Record<string, srv.Booking | undefined> = {};
let returned: srv.Booking[] = [];
let single: srv.Booking = booking(MINE);
const seen: { body?: srv.Booking } = {};
const bodySeen = (): srv.Booking | undefined => seen.body;
let called = 0;
const events: srv.veld.RuleViolationEvent[] = [];
const failures: unknown[] = [];

const impl: srv.TenantsService = {
  getBooking: async (req) => {
    called++;
    return req.booking;
  },
  seen: async (req) => {
    called++;
    return { title: req.held?.title ?? 'none' };
  },
  listBookings: async () => returned,
  pageBookings: async () => ({ items: returned, total: String(returned.length), page: 1, size: returned.length }),
  me: async (_req, ctx) => ctx.principal as srv.User,
  copyBooking: async (req) => req.booking,
  createBooking: async (req, ctx) => {
    seen.body = req.body;
    const who = ctx.principal as srv.User;
    return { ...req.body, tenantId: who.tenantId as string };
  },
  liveBookings: () =>
    (async function* () {
      yield single;
    })(),
};

function tenants(extra: Record<string, unknown> = {}): srv.veld.VeldHandler {
  const hooks = {
    auth: { bearer: async (token: string) => principals[token] ?? null },
    facts: { booking: async () => facts['booking'], held: async () => facts['held'] },
    onRuleViolation: (e: srv.veld.RuleViolationEvent) => events.push(e),
    onError: (err: unknown) => failures.push(err),
    ...extra,
  } as srv.TenantsHooks;
  return srv.createTenantsHandler(impl, hooks);
}

async function send(
  handler: srv.veld.VeldHandler,
  method: string,
  path: string,
  token: string,
  body?: unknown,
): Promise<srv.veld.VeldResponse> {
  const headers: Record<string, string> = { authorization: `Bearer ${token}` };
  if (body !== undefined) headers['content-type'] = 'application/json';
  const res = await handler({ method, path, headers, ...(body === undefined ? {} : { body }) });
  assert.ok(res !== null, `${method} ${path} matched no route`);
  return res;
}

function json(res: srv.veld.VeldResponse): Record<string, unknown> {
  assert.ok(res.body !== null, 'the answer has a body');
  return JSON.parse(res.body) as Record<string, unknown>;
}

/** The answer of a refused value: `500 Internal` with no detail. */
function assertInternal(res: srv.veld.VeldResponse, what: string): void {
  assert.equal(res.status, 500, what);
  const doc = json(res);
  assert.equal(doc['title'], 'Internal', what);
  assert.deepEqual(doc['data'], {}, `${what}: no detail`);
  assert.ok(!(res.body ?? '').includes(OTHER), `${what}: the value of the other tenant is not in the answer`);
}

async function main(): Promise<void> {
  assert.ok(TENANT_RULE.startsWith('veld:bookings/Booking.tenant@'), `the rule of the tenant id: ${TENANT_RULE}`);
  const handler = tenants();

  // A fact of the caller's tenant is loaded.
  facts = { booking: booking(MINE) };
  let res = await send(handler, 'GET', `/tenants/${ID}`, 'mine');
  assert.equal(res.status, 200);
  assert.equal(called, 1);

  // A fact of another tenant is not found, never forbidden, and the handler never runs.
  facts = { booking: booking(OTHER) };
  res = await send(handler, 'GET', `/tenants/${ID}`, 'mine');
  assert.equal(res.status, 404);
  assert.equal(json(res)['title'], 'NotFound');
  assert.deepEqual(json(res)['data'], { resource: 'booking' });
  assert.equal(called, 1, 'the handler did not run');

  // A uuid is the same tenant in any case.
  facts = { booking: booking(MINE.toUpperCase()) };
  res = await send(handler, 'GET', `/tenants/${ID}`, 'mine');
  assert.equal(res.status, 200, 'the same uuid in capitals');

  // An optional fact of another tenant is absent; one of the caller's tenant is there.
  facts = { held: booking(OTHER, { title: 'x' }) };
  res = await send(handler, 'GET', `/tenants/${ID}/seen`, 'mine');
  assert.equal(res.status, 404);
  assert.deepEqual(json(res)['data'], { resource: 'held' });
  facts = { held: booking(MINE, { title: 'x' }) };
  res = await send(handler, 'GET', `/tenants/${ID}/seen`, 'mine');
  assert.equal(res.status, 200);
  assert.deepEqual(json(res), { title: 'x' });

  // A list: one value of another tenant makes the answer a 500; the hook hears that value once.
  events.length = 0;
  returned = [booking(MINE), booking(OTHER, { id: '00000000-0000-4000-8000-000000000002' })];
  res = await send(handler, 'GET', '/tenants', 'mine');
  assertInternal(res, 'a list');
  assert.equal(events.length, 1, 'one event per value');
  const e = events[0];
  assert.ok(e !== undefined);
  assert.equal(e.position, 'output');
  assert.equal(e.outcome, 'rejected');
  assert.equal(e.rule, TENANT_RULE);
  assert.equal(e.violation.path, 'result[1]');
  assert.deepEqual(e.violation.fields, ['tenantId']);
  assert.equal(e.violation.message, 'violates rule tenant');
  assert.equal(e.operation.startsWith('veld:bookings/Tenants.listBookings@'), true);

  // The values of the caller's tenant are sent.
  returned = [booking(MINE), booking(MINE)];
  res = await send(handler, 'GET', '/tenants', 'mine');
  assert.equal(res.status, 200);
  assert.equal((JSON.parse(res.body ?? 'null') as unknown[]).length, 2);

  // A page: the path is the one of the item.
  events.length = 0;
  returned = [booking(MINE), booking(OTHER)];
  res = await send(handler, 'GET', '/tenants/page', 'mine');
  assertInternal(res, 'a page');
  assert.equal(events[0]?.violation.path, 'result.items[1]');

  // A value that is the principal itself: its own tenant is compared too.
  events.length = 0;
  res = await send(handler, 'GET', '/tenants/me', 'mine');
  assert.equal(res.status, 200, 'the caller may be returned');

  // A declared error that holds a value of another tenant is never sent; one that holds the caller's is.
  events.length = 0;
  facts = { booking: booking(MINE, { open: false }) };
  const copy = (tenantId: string): unknown => ({ copy: { id: ID, tenantId, title: 'c', open: true } });
  res = await send(handler, 'POST', `/tenants/${ID}/copy`, 'mine', copy(OTHER));
  assertInternal(res, 'a declared error');
  assert.equal(events[0]?.violation.path, 'error.copy');
  res = await send(handler, 'POST', `/tenants/${ID}/copy`, 'mine', copy(MINE));
  assert.equal(res.status, 409, 'the declared error of the caller own tenant is sent');
  assert.equal(json(res)['title'], 'Dup');

  // A stream item of another tenant is not sent: the answer is a 500 before the first event.
  events.length = 0;
  failures.length = 0;
  single = booking(OTHER);
  res = await send(handler, 'GET', '/tenants/live', 'mine');
  assert.equal(res.status, 500, 'the first item is refused before the stream starts');
  assert.equal(res.stream, undefined, 'no stream');
  assert.equal(events[0]?.violation.path, 'item');
  single = booking(MINE);
  res = await send(handler, 'GET', '/tenants/live', 'mine');
  assert.equal(res.status, 200);
  assert.ok(res.stream !== undefined);

  // A caller without a tenant owns nothing: no fact is found, no value is sent.
  facts = { booking: booking(MINE) };
  res = await send(handler, 'GET', `/tenants/${ID}`, 'none');
  assert.equal(res.status, 404, 'a caller without a tenant finds no fact');
  returned = [];
  res = await send(handler, 'GET', '/tenants', 'none');
  assert.equal(res.status, 200, 'an empty list holds nothing of another tenant');
  returned = [booking(MINE)];
  res = await send(handler, 'GET', '/tenants', 'none');
  assertInternal(res, 'a caller without a tenant');

  // A request may lack the tenant member; one that carries it is kept as received.
  delete seen.body;
  res = await send(handler, 'POST', '/tenants', 'mine', { id: ID, title: 'n', open: true });
  assert.equal(res.status, 200, 'a request without the tenant member is not a 422');
  assert.equal(bodySeen()?.tenantId, undefined);
  assert.equal(json(res)['tenantId'], MINE, 'the application sets the tenant from the caller');
  res = await send(handler, 'POST', '/tenants', 'mine', { id: ID, tenantId: OTHER, title: 'n', open: true });
  assert.equal(bodySeen()?.tenantId, OTHER, 'a member the request carries is kept as received');
  assert.equal(json(res)['tenantId'], MINE);

  // No setting turns it off: request validation off, rule output off.
  for (const extra of [{ validateRequests: false }, { rules: { output: 'off' as const } }]) {
    const loose = tenants(extra);
    returned = [booking(OTHER)];
    res = await send(loose, 'GET', '/tenants', 'mine');
    assertInternal(res, `a list with ${JSON.stringify(extra)}`);
    facts = { booking: booking(OTHER) };
    res = await send(loose, 'GET', `/tenants/${ID}`, 'mine');
    assert.equal(res.status, 404, `a fact with ${JSON.stringify(extra)}`);
  }

  console.log('tenancy_http: ok');
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
