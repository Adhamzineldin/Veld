// Timed moves over the production handler (spec/platform.md section 10, spec/targets/typescript.md
// section 5j): the boundary of the due check, `timedMoves()`, and the due-moves runner. Expects
// `./server` generated from tests/toolchain/contract/platform_p2/timed.veld.
import assert from 'node:assert/strict';
import * as srv from './server/index';
import type { VeldRequest } from './server/runtime/veld';

const NOW = Date.parse('2026-06-11T12:00:00Z');
const ID = (n: number): string => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`;

const rows = new Map<string, srv.Booking>();
const seen: { name: string; principal: unknown; id: string; transition: unknown }[] = [];
const errors: unknown[] = [];
let handlerFails = new Set<string>();
let firstMover: (() => void) | null = null;

const loaders: srv.BookingsFactLoaders = {
  async booking(key) {
    return rows.get(key);
  },
};

const impl: srv.BookingsService = {
  async expireBooking(req, ctx) {
    seen.push({ name: 'expireBooking', principal: ctx.principal, id: req.id, transition: req.transition });
    if (handlerFails.has(req.id)) throw new Error('the handler broke');
    firstMover?.();
    return { ...req.booking, status: srv.Status.cancelled };
  },
  async archiveBooking(req, ctx) {
    seen.push({ name: 'archiveBooking', principal: ctx.principal, id: req.id, transition: req.transition });
    return { ...req.booking, status: srv.Status.cancelled };
  },
};

function put(n: number, since: string, status: srv.Status = srv.Status.pending): string {
  const id = ID(n);
  rows.set(id, { id, status, pendingSince: since });
  return id;
}

function request(path: string, token?: string): VeldRequest {
  const headers: Record<string, string> = token === undefined ? {} : { authorization: `Bearer ${token}` };
  return { method: 'POST', path, headers };
}

const hooks: srv.BookingsHooks = {
  facts: loaders,
  auth: { bearer: async (t) => (t === 'ok' ? { name: 'caller' } : null) },
  clock: () => new Date(NOW),
  timer: { principal: () => ({ name: 'timer' }) },
  onError: (e) => errors.push(e),
};

async function main(): Promise<void> {
  const handler = srv.createBookingsHandler(impl, hooks);
  const since = (ms: number): string => new Date(NOW - ms).toISOString();
  const day = 86_400_000;

  // The boundary: exactly due, one millisecond early, a since in the future, one that cannot be read.
  const exact = put(1, since(day));
  assert.equal((await handler(request(`/bookings/${exact}/archive`)))?.status, 200);
  const early = put(2, since(day - 1));
  const refused = await handler(request(`/bookings/${early}/archive`));
  assert.equal(refused?.status, 409);
  const doc = JSON.parse(refused?.body ?? '{}') as { type: string; data: unknown; violations: { message: string }[] };
  assert.equal(doc.type, srv.InvalidTransition.sig);
  assert.deepEqual(doc.data, { field: 'status', from: 'pending', to: 'cancelled' });
  assert.equal(doc.violations[0]?.message, 'violates rule transition: not due');
  const future = put(3, new Date(NOW + 1000).toISOString());
  assert.equal((await handler(request(`/bookings/${future}/archive`)))?.status, 409);
  const unreadable = put(4, 'not a datetime');
  assert.equal((await handler(request(`/bookings/${unreadable}/archive`)))?.status, 409);
  // A state other than the source is the ordinary refusal.
  const paid = put(5, since(day * 3), srv.Status.paid);
  const other = JSON.parse((await handler(request(`/bookings/${paid}/archive`)))?.body ?? '{}') as {
    violations: { message: string }[];
  };
  assert.equal(other.violations[0]?.message, 'violates rule transition');

  // timedMoves(): the source, the target and the milliseconds.
  assert.deepEqual(srv.Status.timedMoves(), [{ from: 'pending', to: 'cancelled', ms: day }]);

  // The runner: a due move runs through the pipeline and reaches the handler with the timer's principal.
  rows.clear();
  seen.length = 0;
  const a = put(10, since(day));
  const b = put(11, since(day * 2));
  const asked: { before: string; limit: number }[] = [];
  const due = { loaders: { async expireBooking(before: Date, limit: number) {
    asked.push({ before: before.toISOString(), limit });
    return [{ id: a }, { id: b }];
  }, async archiveBooking() { return []; } } } as srv.BookingsDueOptions;
  const report = await srv.runBookingsDueMoves(impl, hooks, { ...due, now: new Date(NOW) });
  assert.deepEqual(asked, [{ before: since(day), limit: 100 }]);
  assert.deepEqual(report['expireBooking'], { ran: 2, moved: 2, refused: 0, failed: 0 });
  assert.deepEqual(seen.map((s) => s.principal), [{ name: 'timer' }, { name: 'timer' }]);
  assert.deepEqual(seen[0]?.transition, { field: 'status', from: 'pending', to: 'cancelled' });

  // A move that another request made first is refused, not failed; a failing handler does not stop the others.
  seen.length = 0;
  const c = put(20, since(day));
  const d = put(21, since(day));
  const e = put(22, since(day));
  handlerFails = new Set([d]);
  firstMover = () => {
    // Between the loader and the guard, the row of `e` moves: the guard sees it.
    const row = rows.get(e);
    if (row !== undefined) rows.set(e, { ...row, status: srv.Status.cancelled });
  };
  const second = await srv.runBookingsDueMoves(impl, hooks, {
    now: new Date(NOW),
    loaders: {
      async expireBooking() {
        return [{ id: c }, { id: d }, { id: e }];
      },
      async archiveBooking() {
        return [];
      },
    },
  });
  assert.deepEqual(second['expireBooking'], { ran: 3, moved: 1, refused: 1, failed: 1 });
  assert.ok(errors.length >= 1, 'a failing handler goes to onError');
  firstMover = null;
  handlerFails = new Set();

  // `limit` caps what the runner runs, even when a loader answers more.
  const f = put(30, since(day));
  const g = put(31, since(day));
  const h = put(32, since(day));
  const capped = await srv.runBookingsDueMoves(impl, hooks, {
    now: new Date(NOW),
    limit: 2,
    loaders: {
      async expireBooking() {
        return [{ id: f }, { id: g }, { id: h }];
      },
      async archiveBooking() {
        return [];
      },
    },
  });
  assert.equal(capped['expireBooking']?.ran, 2);

  // A run on an endpoint with auth and no timer principal does not start.
  const { timer: _timer, ...bare } = hooks;
  await assert.rejects(
    srv.runBookingsDueMoves(impl, bare, { loaders: { async expireBooking() { return []; }, async archiveBooking() { return []; } } }),
    /hooks\.timer\.principal is required/,
  );
  console.log('timed_http ok');
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
