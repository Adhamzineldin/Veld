// Field visibility (spec/platform.md section 2) over the production handler of
// tests/toolchain/contract/platform_p1/visibility.veld: a field hidden for one caller and present
// for another, in a value, a list and a page; an expression that cannot be evaluated; the clock;
// an `ensure` that still sees the field; a declared error; a stream item; a service with no
// caller; and the settings that must change nothing (`validateRequests: false`, output mode
// `off`). Expects `./server` generated from that contract (compiled as the package `bookings`).
import assert from 'node:assert/strict';
import * as srv from './server/index';

const ME = '00000000-0000-4000-8000-0000000000aa';
const OTHER = '00000000-0000-4000-8000-0000000000bb';
const BOOKING = '00000000-0000-4000-8000-000000000001';
const principals: Record<string, { id: string; roles: string[] }> = {
  guest: { id: ME, roles: [] },
  other: { id: OTHER, roles: [] },
  support: { id: OTHER, roles: ['support'] },
};

let clockAt = new Date('2026-06-10T12:00:00Z');
let booking: srv.Booking = make();
let list: srv.Booking[] = [];
let fact: srv.Booking | undefined;

function make(over: Partial<srv.Booking> = {}): srv.Booking {
  return {
    id: BOOKING,
    guest: { id: ME, name: 'Guest', phone: '+201000000001' },
    divisor: 1,
    opensAt: '2026-06-11T12:00:00Z',
    note: 'vip',
    ratio: 'r',
    early: 'e',
    ...over,
  };
}

const impl: srv.DeskService = {
  getBooking: async () => booking,
  listBookings: async () => list,
  pageBookings: async () => ({ items: list, total: String(list.length), page: 1, size: list.length }),
  checkedBooking: async () => booking,
  cancelBooking: async () => booking,
  liveBookings: () =>
    (async function* () {
      yield booking;
    })(),
};

/** The production handler; `extra` holds the settings that must change nothing (they are not in the hooks type of every target). */
function desk(extra: Record<string, unknown> = {}): srv.veld.VeldHandler {
  const hooks = {
    auth: { bearer: async (token: string) => principals[token] ?? null },
    facts: { booking: async () => fact },
    clock: () => clockAt,
    ...extra,
  } as srv.DeskHooks;
  return srv.createDeskHandler(impl, hooks);
}

const notices = srv.createNoticesHandler({ publicBooking: async () => booking });

async function get(handler: srv.veld.VeldHandler, path: string, token?: string, method = 'GET'): Promise<srv.veld.VeldResponse> {
  const headers: Record<string, string> = token === undefined ? {} : { authorization: `Bearer ${token}` };
  const res = await handler({ method, path, headers });
  assert.ok(res !== null, `${method} ${path} matched no route`);
  return res;
}

function json(res: srv.veld.VeldResponse): Record<string, unknown> {
  assert.ok(res.body !== null, 'the answer has a body');
  return JSON.parse(res.body) as Record<string, unknown>;
}

function guestOf(body: Record<string, unknown>): Record<string, unknown> {
  return body['guest'] as Record<string, unknown>;
}

async function main(): Promise<void> {
  const handler = desk();

  // A field hidden for one caller and present for another.
  booking = make();
  let res = await get(handler, `/desk/${BOOKING}`, 'guest');
  assert.equal(res.status, 200);
  assert.equal(guestOf(json(res))['phone'], '+201000000001', 'the guest sees their own phone');
  assert.equal(json(res)['note'], undefined, 'only support sees the note');
  res = await get(handler, `/desk/${BOOKING}`, 'other');
  assert.equal(guestOf(json(res))['phone'], undefined, 'another caller does not see the phone');
  assert.equal(guestOf(json(res))['name'], 'Guest', 'what is not @visible stays');
  res = await get(handler, `/desk/${BOOKING}`, 'support');
  assert.equal(guestOf(json(res))['phone'], '+201000000001', 'support sees every phone');
  assert.equal(json(res)['note'], 'vip', 'support sees the note');

  // An ETag-free check that the handler's own value is not changed by the filter.
  assert.equal(booking.note, 'vip');
  assert.equal(booking.guest.phone, '+201000000001');

  // A list: each element is judged for the caller.
  list = [make(), make({ id: OTHER, guest: { id: OTHER, name: 'Other', phone: '+2' } })];
  res = await get(handler, '/desk', 'guest');
  let items = JSON.parse(res.body ?? 'null') as Record<string, unknown>[];
  assert.equal(guestOf(items[0] ?? {})['phone'], '+201000000001');
  assert.equal(guestOf(items[1] ?? {})['phone'], undefined);

  // A page: each item of `items` is judged for the caller.
  res = await get(handler, '/desk/page', 'guest');
  const page = json(res);
  items = page['items'] as Record<string, unknown>[];
  assert.equal(page['total'], '2');
  assert.equal(guestOf(items[0] ?? {})['phone'], '+201000000001');
  assert.equal(guestOf(items[1] ?? {})['phone'], undefined);

  // An expression that cannot be evaluated hides the field, for support too.
  booking = make({ divisor: 0 });
  res = await get(handler, `/desk/${BOOKING}`, 'support');
  assert.equal(json(res)['ratio'], undefined, 'a division by zero hides the field');
  booking = make({ divisor: 2 });
  res = await get(handler, `/desk/${BOOKING}`, 'other');
  assert.equal(json(res)['ratio'], 'r', 'an expression that can be evaluated shows it');

  // The clock of the request, to the millisecond.
  booking = make({ opensAt: '2026-06-11T12:00:00Z' });
  clockAt = new Date('2026-06-11T11:59:59.999Z');
  res = await get(handler, `/desk/${BOOKING}`, 'other');
  assert.equal(json(res)['early'], undefined, 'before the opening time it is hidden');
  clockAt = new Date('2026-06-11T12:00:00Z');
  res = await get(handler, `/desk/${BOOKING}`, 'other');
  assert.equal(json(res)['early'], 'e', 'from the opening time on it is shown');
  clockAt = new Date('2026-06-10T12:00:00Z');

  // An `ensure` sees the whole value, the note that visibility drops afterwards.
  booking = make();
  res = await get(handler, `/desk/checked/${BOOKING}`, 'other');
  assert.equal(res.status, 200, 'the ensure held: it saw the note');
  assert.equal(json(res)['note'], undefined, 'and the caller does not');

  // A declared error: its `who` is judged for the caller.
  fact = make({ divisor: 0 });
  res = await get(handler, `/desk/${BOOKING}/cancel`, 'other', 'POST');
  assert.equal(res.status, 409);
  const data = json(res)['data'] as Record<string, unknown>;
  assert.equal(data['reason'], 'closed');
  assert.equal(data['who'], undefined, 'another caller does not see who');
  res = await get(handler, `/desk/${BOOKING}/cancel`, 'support', 'POST');
  assert.equal((json(res)['data'] as Record<string, unknown>)['who'], 'front desk');

  // A stream item is judged for the caller.
  booking = make();
  res = await get(handler, '/desk/live', 'other');
  assert.ok(res.stream !== undefined, 'the answer is a stream');
  let frame = '';
  for await (const f of res.stream) {
    frame = f;
    break;
  }
  const item = JSON.parse(frame.replace(/^data: /, '').trim()) as Record<string, unknown>;
  assert.equal(item['note'], undefined, 'the item of a stream is filtered');
  assert.equal(guestOf(item)['phone'], undefined);

  // No caller: every @visible field is dropped, whatever the caller would have been.
  booking = make();
  res = await get(notices, `/notices/${BOOKING}`);
  assert.equal(res.status, 200);
  assert.equal(json(res)['note'], undefined);
  assert.equal(guestOf(json(res))['phone'], undefined);
  assert.equal(json(res)['id'], BOOKING);

  // No setting turns it off: request validation off, rule output off.
  for (const extra of [{ validateRequests: false }, { rules: { output: 'off' as const } }]) {
    const loose = desk(extra);
    res = await get(loose, `/desk/${BOOKING}`, 'other');
    assert.equal(guestOf(json(res))['phone'], undefined, `visibility holds with ${JSON.stringify(extra)}`);
    assert.equal(json(res)['note'], undefined);
  }

  console.log('visibility_http: ok');
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
