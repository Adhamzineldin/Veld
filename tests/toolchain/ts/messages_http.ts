// Message catalogs (spec/platform.md section 22) over the production handler and the generated
// client of the workspace of tests/wire/messages (an `ar` and a `pt-br` catalog), with a service
// `Intake` appended whose body is a `Booking`, so that the invariants of a `Booking` can be sent.
// Expects `./server` and `./client` generated from it, the cases of tests/wire/messages/cases.json
// next to this file as `cases.json`, and the catalogs the compiler read as `catalogs.json`.
//
// - the 29 rows of `headers` and the 15 rows of `messages` through `chooseLanguage` and
//   `catalogText`, and the catalogs and keys the files embed;
// - one request per kind of rule over the production handler: the language, the text, the
//   `messageKey` and `Content-Language` of the answer, the event the hook hears;
// - the client: its catalogs, the keys of its local check, a response that carries `messageKey`.
// Prints `messages http ok`.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as cli from './client/index';
import * as srv from './server/index';

interface Cases {
  languages: string[];
  headers: { header: string | null; language: string | null }[];
  messages: { header: string | null; key: string; default: string; text: string; language: string | null; note: string }[];
  keys: { what: string; kind: string; key: string }[];
}
const cases = JSON.parse(readFileSync('cases.json', 'utf8')) as Cases;
const catalogs = JSON.parse(readFileSync('catalogs.json', 'utf8')) as Record<string, Record<string, string>>;

const ID = '00000000-0000-4000-8000-000000000001';
const USER = { id: '00000000-0000-4000-8000-0000000000aa', suspended: false, banned: false };

// The application.
let user = { ...USER };
let booking: srv.Booking = { id: ID, status: 'pending', revision: '3', start: '2026-06-10T00:00:00Z', end: '2026-06-12T00:00:00Z', total: '10.00' };
let result: Partial<srv.Booking> = {};
const events: srv.veld.RuleViolationEvent[] = [];

const bookingsImpl: srv.BookingsService = {
  pay: async (req) => ({ ...booking, status: 'paid', ...result, id: req.id }) as srv.Booking,
};
const intakeImpl: srv.IntakeService = { intake: async (req) => req.body };

const bookings = srv.createBookingsHandler(bookingsImpl, {
  auth: { bearer: async (token: string) => (token === 'me' ? user : null) },
  facts: { booking: async () => booking },
  onRuleViolation: (e) => void events.push(e),
});
const intake = srv.createIntakeHandler(intakeImpl, { onRuleViolation: (e) => void events.push(e) });

async function dispatch(req: srv.veld.VeldRequest): Promise<srv.veld.VeldResponse> {
  const res = (await bookings(req)) ?? (await intake(req));
  assert.ok(res !== null, `${req.method} ${req.path} matched no route`);
  return res;
}

async function send(method: string, path: string, body: unknown, language?: string, extra: Record<string, string> = {}): Promise<{ res: srv.veld.VeldResponse; doc: any }> {
  const headers: Record<string, string> = { authorization: 'Bearer me', ...extra };
  if (language !== undefined) headers['accept-language'] = language;
  const res = await dispatch({ method, path, headers, body });
  return { res, doc: res.body === null ? null : JSON.parse(res.body) };
}

const intakeBody = (extra: Record<string, unknown> = {}): Record<string, unknown> => ({
  id: ID,
  revision: '1',
  start: '2026-06-12T00:00:00Z',
  end: '2026-06-10T00:00:00Z',
  total: '1.00',
  ...extra,
});
const AR = catalogs['ar'] ?? {};
const PT = catalogs['pt-br'] ?? {};

function languageOf(res: srv.veld.VeldResponse): string | undefined {
  return res.headers['content-language'];
}

async function tables(): Promise<void> {
  // The catalogs the files hold, as the compiler read them, and the languages of the table.
  assert.deepEqual(srv.messages, catalogs);
  assert.deepEqual(cli.messages, catalogs);
  assert.deepEqual(Object.keys(srv.messages).sort(), [...cases.languages].sort());
  for (const veld of [srv.veld, cli.veld]) {
    const available = Object.keys(catalogs);
    for (const row of cases.headers) {
      const chosen = veld.chooseLanguage(row.header ?? undefined, available);
      assert.equal(chosen ?? null, row.language, `header ${JSON.stringify(row.header)}`);
    }
    for (const row of cases.messages) {
      const chosen = veld.chooseLanguage(row.header ?? undefined, available);
      const found = veld.catalogText(catalogs, chosen, row.key, row.default);
      assert.equal(found.text, row.text, `${row.note}: ${JSON.stringify(row.header)} ${row.key}`);
      assert.equal(found.language ?? null, row.language, `${row.note}: the language`);
    }
  }
  // The fixed keys are the four of the table, and every key of the cases is one of the keys of a rule or a fixed one.
  assert.deepEqual(srv.fixedMessageKeys, {
    transition: 'veld/transition',
    transitionNotDue: 'veld/transition-not-due',
    version: 'veld/version',
    notPermitted: 'veld/not-permitted',
  });
  const known = new Set([...Object.values(srv.messageKeys), ...Object.values(srv.fixedMessageKeys)]);
  // A denial has the key of its `forbid`, which the verdict function carries, not the table.
  for (const row of cases.keys.filter((k) => k.kind !== 'denial')) assert.ok(known.has(row.key), `${row.what}: ${row.key} is a key of the server`);
}

async function invariants(): Promise<void> {
  // An invariant with a message: every language of the table, and the answer's `Content-Language`.
  const want: [string | undefined, string, string | undefined][] = [
    ['ar', AR['bookings/Booking.endAfterStart'] ?? '', 'ar'],
    ['pt-BR', PT['bookings/Booking.endAfterStart'] ?? '', 'pt-br'],
    [undefined, 'A booking must end after it starts', undefined],
    ['fr', 'A booking must end after it starts', undefined],
    ['pt', 'A booking must end after it starts', undefined],
    ['*', 'A booking must end after it starts', undefined],
    ['ar;q=0.2, pt-br', PT['bookings/Booking.endAfterStart'] ?? '', 'pt-br'],
  ];
  for (const [header, text, language] of want) {
    const { res, doc } = await send('POST', '/intake', intakeBody(), header);
    assert.equal(res.status, 422, String(header));
    assert.equal(doc.violations.length, 1);
    assert.equal(doc.violations[0].message, text, `the violation in ${String(header)}`);
    assert.equal(doc.violations[0].messageKey, 'bookings/Booking.endAfterStart');
    assert.equal(doc.data.issues[0].message, text, 'the issue says the same');
    assert.equal(doc.data.issues[0].code, 'rule');
    assert.equal(languageOf(res), language, `Content-Language for ${String(header)}`);
  }

  // A rule without a message has its key and the text of the language: a catalog lacks it, so none is used.
  let { res, doc } = await send('POST', '/intake', intakeBody({ start: '2026-06-10T00:00:00Z', end: '2026-06-12T00:00:00Z', total: '-1.00' }), 'ar');
  assert.equal(doc.violations[0].messageKey, 'bookings/Booking.totalKnown');
  assert.equal(doc.violations[0].message, 'violates rule totalKnown');
  assert.equal(languageOf(res), undefined, 'no message of the answer came from a catalog');

  // Two violations, one from the catalog: the answer says so.
  ({ res, doc } = await send('POST', '/intake', intakeBody({ total: '-1.00' }), 'ar'));
  assert.deepEqual(
    doc.violations.map((v: { messageKey: string }) => v.messageKey).sort(),
    ['bookings/Booking.endAfterStart', 'bookings/Booking.totalKnown'],
  );
  assert.equal(languageOf(res), 'ar');

  // A shape issue has no key and is not translated.
  ({ res, doc } = await send('POST', '/intake', intakeBody({ total: 'x' }), 'ar'));
  assert.equal(res.status, 422);
  assert.equal(doc.data.issues[0].code, 'type');
  assert.equal(doc.violations, undefined);
  assert.equal(languageOf(res), undefined);

  // The hook of the application hears the key and the text of the contract.
  events.length = 0;
  await send('POST', '/intake', intakeBody(), 'ar');
  assert.equal(events.length, 1);
  assert.equal(events[0]?.violation.messageKey, 'bookings/Booking.endAfterStart');
  assert.equal(events[0]?.violation.message, 'A booking must end after it starts');
}

const PAY = `/bookings/${ID}/pay`;
const pay = (amount: string, language?: string, extra: Record<string, string> = {}): ReturnType<typeof send> =>
  send('POST', PAY, { amount }, language, { 'if-match': '"3"', ...extra });

async function statements(): Promise<void> {
  // A named `require`: `pt-BR` lacks it, so the contract's text and no Content-Language; `ar` has it.
  let { res, doc } = await pay('0.00', 'pt-BR');
  assert.equal(res.status, 422);
  assert.equal(doc.violations[0].message, 'The amount must be positive');
  assert.equal(doc.violations[0].messageKey, 'bookings/Bookings.pay.notZero');
  assert.equal(languageOf(res), undefined);
  ({ res, doc } = await pay('0.00', 'ar'));
  assert.equal(doc.violations[0].message, AR['bookings/Bookings.pay.notZero']);
  assert.equal(doc.data.issues[0].message, AR['bookings/Bookings.pay.notZero']);
  assert.equal(languageOf(res), 'ar');

  // An unnamed `require` has the key every unnamed one of the endpoint shares.
  ({ res, doc } = await pay('2000000.00', 'ar'));
  assert.equal(doc.violations[0].messageKey, 'bookings/Bookings.pay.require');
  assert.equal(doc.violations[0].message, AR['bookings/Bookings.pay.require']);

  // The version guard: a fixed text, in the languages that have it.
  ({ res, doc } = await send('POST', PAY, { amount: '5.00' }, 'pt-br', { 'if-match': '"999"' }));
  assert.equal(res.status, 412);
  assert.equal(doc.violations[0].messageKey, 'veld/version');
  assert.equal(doc.violations[0].message, PT['veld/version']);
  assert.equal(languageOf(res), 'pt-br');
  ({ res, doc } = await send('POST', PAY, { amount: '5.00' }, 'ar', { 'if-match': '"999"' }));
  assert.equal(doc.violations[0].messageKey, 'veld/version');
  assert.equal(languageOf(res), undefined, 'ar has no text for it');

  // A move that is not legal.
  const original = booking;
  booking = { ...booking, status: 'paid' };
  ({ res, doc } = await pay('5.00', 'ar;q=0.2, pt-br'));
  assert.equal(res.status, 409);
  assert.equal(doc.violations[0].messageKey, 'veld/transition');
  assert.equal(doc.violations[0].message, PT['veld/transition']);
  assert.equal(languageOf(res), 'pt-br');

  // The denial of a policy: the key of the `forbid` that was true, in the data and in the violation.
  booking = original;
  user = { ...USER, suspended: true };
  events.length = 0;
  ({ res, doc } = await pay('5.00', 'ar'));
  assert.equal(res.status, 403);
  // The hook hears the key and the contract's text, whatever the language of the answer.
  assert.equal(events.length, 1);
  assert.equal(events[0]?.violation.messageKey, 'bookings/Manage.forbid1');
  assert.equal(events[0]?.violation.message, 'A suspended account cannot manage bookings');
  assert.equal(doc.violations[0].messageKey, 'bookings/Manage.forbid1');
  assert.equal(doc.violations[0].message, AR['bookings/Manage.forbid1']);
  assert.equal(doc.data.message, AR['bookings/Manage.forbid1']);
  assert.equal(languageOf(res), 'ar');
  ({ res, doc } = await pay('5.00', 'pt-br'));
  assert.equal(doc.violations[0].messageKey, 'bookings/Manage.forbid1');
  assert.equal(doc.data.message, 'A suspended account cannot manage bookings', 'the catalog lacks the forbid: the contract text');
  assert.equal(languageOf(res), undefined);
  // The third forbid, counting the one without a message.
  user = { ...USER };
  booking = { ...booking, status: 'cancelled' };
  ({ res, doc } = await pay('5.00', 'ar'));
  assert.equal(doc.violations[0].messageKey, 'bookings/Manage.forbid3');
  assert.equal(doc.data.message, 'A cancelled booking cannot be managed');
  assert.equal(languageOf(res), undefined);
  // A forbid without a message: the fixed text.
  booking = original;
  user = { ...USER, banned: true };
  for (const [header, text, language] of [
    ['ar-EG', AR['veld/not-permitted'], 'ar'],
    ['pt-br', PT['veld/not-permitted'], 'pt-br'],
    [undefined, 'not permitted', undefined],
  ] as const) {
    ({ res, doc } = await pay('5.00', header));
    assert.equal(res.status, 403);
    assert.equal(doc.violations[0].messageKey, 'veld/not-permitted');
    assert.equal(doc.violations[0].message, text);
    assert.equal(doc.data.message, text);
    assert.equal(languageOf(res), language);
  }
  user = { ...USER };

  // An `ensure` is an output: the answer is a `500` with no text, and the hook hears the key.
  events.length = 0;
  result = { revision: '-1' };
  ({ res, doc } = await pay('5.00', 'ar'));
  assert.equal(res.status, 500);
  assert.equal(languageOf(res), undefined);
  assert.equal(events[0]?.violation.messageKey, 'bookings/Bookings.pay.ensure');
  assert.equal(events[0]?.violation.message, 'The revision never goes down');
  result = {};
  // The target check of the move has the key of the transition.
  events.length = 0;
  result = { status: 'pending' };
  await pay('5.00', 'ar');
  assert.equal(events[0]?.violation.messageKey, 'veld/transition');
  result = {};

  // A request with no header, and an answer that carries none of it, as before.
  ({ res, doc } = await pay('0.00'));
  assert.equal(languageOf(res), undefined);
  assert.equal(doc.violations[0].messageKey, 'bookings/Bookings.pay.notZero');
  assert.equal(doc.violations[0].message, 'The amount must be positive');
}

async function client(): Promise<void> {
  const calls: { path: string; headers: Record<string, string> }[] = [];
  const fetch: cli.veld.FetchLike = async (url, init) => {
    const at = new URL(url);
    calls.push({ path: at.pathname, headers: init.headers });
    const res = await dispatch({
      method: init.method,
      path: at.pathname,
      ...(at.search === '' ? {} : { query: at.search.slice(1) }),
      headers: { authorization: 'Bearer me', ...init.headers },
      ...(init.body === undefined ? {} : { body: JSON.parse(init.body) as unknown }),
    });
    return { status: res.status, text: async () => res.body ?? '', headers: { get: (n: string) => res.headers[n.toLowerCase()] ?? null } };
  };
  const make = (headers: Record<string, string> = {}): cli.IntakeClient => new cli.IntakeClient({ baseUrl: 'http://bookings.test', fetch, headers });

  // The violations of the local check have the key, and the text of the contract; the app picks its own language.
  const local = await make().intake({ body: intakeBody() as never }).then(
    () => null,
    (e: unknown) => e,
  );
  assert.ok(local !== null);
  const err = local as unknown as { local: boolean; violations: { rule: string; message: string; messageKey?: string }[] };
  assert.equal(err.local, true);
  assert.equal(err.violations[0]?.messageKey, 'bookings/Booking.endAfterStart');
  assert.equal(err.violations[0]?.message, 'A booking must end after it starts');
  assert.deepEqual(calls, [], 'nothing was sent');
  const language = cli.veld.chooseLanguage('ar-EG', Object.keys(cli.messages));
  assert.equal(language, 'ar');
  const violation = err.violations[0];
  assert.ok(violation !== undefined && violation.messageKey !== undefined);
  assert.equal(cli.veld.catalogText(cli.messages, language, violation.messageKey, violation.message).text, AR['bookings/Booking.endAfterStart']);
  // The keys of the rules are exported with the catalogs.
  assert.equal(cli.messageKeys[violation.rule], 'bookings/Booking.endAfterStart');

  // A response that carries `messageKey` (and the text of the server's language) keeps both.
  const remote = await new cli.IntakeClient({ baseUrl: 'http://bookings.test', fetch, headers: { 'accept-language': 'pt-br' }, rules: false })
    .intake({ body: intakeBody() as never })
    .then(
      () => null,
      (e: unknown) => e as { violations: { messageKey?: string; message: string }[]; local: boolean },
    );
  assert.ok(remote !== null);
  assert.equal(remote.local, false);
  assert.equal(remote.violations[0]?.messageKey, 'bookings/Booking.endAfterStart');
  assert.equal(remote.violations[0]?.message, PT['bookings/Booking.endAfterStart']);
}

async function main(): Promise<void> {
  await tables();
  await invariants();
  await statements();
  await client();
  process.stdout.write('messages http ok\n');
}

main().catch((e: unknown) => {
  process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  process.exit(1);
});
