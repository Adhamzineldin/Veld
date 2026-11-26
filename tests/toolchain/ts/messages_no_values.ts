// Platform phase P4, part 1 (spec/rules.md 19.3): no message repeats a value. A marker string goes
// through every path of the generated server and client that writes a message, a `detail`, a
// `title` or a log line (validation issues of every code, violations of every kind of rule, a
// refused transition, version, policy and idempotency key, the runtime config, a transport that
// fails), and every text that comes out is searched for the marker. Paths and names may hold it
// (`$.extra.<key>` names the map key it is about); texts may not. Expects `./server` and `./client`
// generated from tests/toolchain/contract/platform_p4/no_values.veld (package `bookings`).
// Prints `no values ok`.
import { strict as assert } from 'assert';

import * as cli from './client/index';
import * as srv from './server/index';

const MARKER = 'MARKER-3f9a71c2e5';
const ID = '00000000-0000-4000-8000-000000000001';
const USER = { id: '00000000-0000-4000-8000-0000000000aa', suspended: false };

/** Every text the runtime wrote, labelled by where it came from. */
const texts: { where: string; text: string }[] = [];
const note = (where: string, text: unknown): void => {
  if (typeof text === 'string') texts.push({ where, text });
};

/** The texts of a problem document: `title`, `detail`, the issues, the violations and the message of a denial. */
function problemTexts(where: string, body: string | null | undefined): void {
  if (body === null || body === undefined) return;
  let doc: Record<string, unknown>;
  try {
    doc = JSON.parse(body) as Record<string, unknown>;
  } catch {
    note(`${where} body`, body);
    return;
  }
  note(`${where} title`, doc['title']);
  note(`${where} detail`, doc['detail']);
  const data = (doc['data'] ?? {}) as { issues?: { message: string }[]; message?: string };
  for (const i of data.issues ?? []) note(`${where} issue`, i.message);
  note(`${where} data.message`, data.message);
  for (const v of (doc['violations'] ?? []) as { message: string }[]) note(`${where} violation`, v.message);
}

// What the application does: it is the handler of a request that passed every check.
let booking: srv.Booking = { id: ID, status: 'pending', revision: 3n as never, total: '10.00' } as never;
let resultOverride: Partial<srv.Booking> | undefined;
const impl: srv.ProbeService = {
  shapes: async (req) => req.body,
  byId: async () => ({}) as srv.Shape,
  byName: async () => ({}) as srv.Shape,
  pay: async (req) => ({ ...booking, status: 'paid', ...resultOverride, id: req.id }) as srv.Booking,
};

const errors: unknown[] = [];
const events: srv.veld.RuleViolationEvent[] = [];
let suspended = false;

function probe(extra: Record<string, unknown> = {}): srv.veld.VeldHandler {
  const hooks = {
    auth: { bearer: async (token: string) => (token === 'me' ? { ...USER, suspended } : null) },
    facts: { booking: async () => booking },
    idempotency: { store: new srv.veld.MemoryIdempotencyStore(), caller: (p: srv.User) => p.id },
    onError: (err: unknown) => errors.push(err),
    onRuleViolation: (e: srv.veld.RuleViolationEvent) => events.push(e),
    ...extra,
  } as srv.ProbeHooks;
  return srv.createProbeHandler(impl, hooks);
}

async function send(
  handler: srv.veld.VeldHandler,
  where: string,
  method: string,
  path: string,
  body?: unknown,
  headers: Record<string, string> = {},
  token = 'me',
): Promise<srv.veld.VeldResponse> {
  const res = await handler({ method, path, headers: { authorization: `Bearer ${token}`, ...headers }, ...(body === undefined ? {} : { body }) });
  assert.ok(res !== null, `${where}: ${method} ${path} matched no route`);
  problemTexts(where, res.body);
  for (const [name, value] of Object.entries(res.headers)) note(`${where} header ${name}`, value);
  return res;
}

const shape = (extra: Record<string, unknown> = {}): Record<string, unknown> => ({
  id: ID,
  name: 'abcd',
  code: 'ABC',
  count: 2,
  price: '1.00',
  when: '2026-06-10T12:00:00Z',
  status: 'pending',
  tags: ['a'],
  extra: { a: 1 },
  pay: { kind: 'card', last4: '1234' },
  email: 'a@b.example',
  blob: { any: 'thing' },
  ...extra,
});

async function serverCases(): Promise<void> {
  const handler = probe();

  // Every validation issue code of a body: type, required, enum, bounds, length, pattern, format,
  // discriminator, a map value, a list that is not a list.
  const bad = shape({
    id: MARKER, name: MARKER, code: MARKER, count: MARKER, price: MARKER, when: MARKER, status: MARKER,
    tags: MARKER, extra: { [MARKER]: MARKER }, pay: { kind: MARKER }, email: MARKER,
  });
  let res = await send(handler, 'shape: everything wrong', 'POST', '/probe/shapes', bad);
  assert.equal(res.status, 422);
  for (const field of ['name', 'code', 'count', 'price', 'when', 'status', 'tags', 'extra', 'pay', 'email']) {
    await send(handler, `shape: ${field}`, 'POST', '/probe/shapes', shape({ [field]: field === 'extra' ? { k: MARKER } : field === 'pay' ? { kind: MARKER } : MARKER }));
  }
  for (const missing of ['name', 'code', 'pay']) {
    const without = shape();
    delete without[missing];
    await send(handler, `shape: ${missing} is missing`, 'POST', '/probe/shapes', without);
  }
  await send(handler, 'shape: bounds', 'POST', '/probe/shapes', shape({ count: 99, name: 'a' }));
  await send(handler, 'shape: not an object', 'POST', '/probe/shapes', MARKER);
  res = await handler({ method: 'POST', path: '/probe/shapes', headers: { authorization: 'Bearer me', 'content-type': 'application/json' }, text: async () => `{"name": "${MARKER}` }) ?? res;
  problemTexts('shape: a body that is not JSON', res.body);

  // Invariants: violated (an odd count), and one that cannot be evaluated (a division by zero).
  res = await send(handler, 'rule: violated', 'POST', '/probe/shapes', shape({ count: 3, name: MARKER.slice(0, 8) }));
  assert.equal(res.status, 422);
  res = await send(handler, 'rule: cannot be evaluated', 'POST', '/probe/shapes', shape({ count: 4 }));
  assert.equal(res.status, 422);
  for (const e of events) note('rule event', e.violation.message);

  // The three parameters of another binding.
  await send(handler, 'path, query and header', 'GET', `/probe/by/${MARKER}`, undefined, { 'x-marker': MARKER }, 'me');
  await send(handler, 'query', 'GET', `/probe/by/${ID}?size=${MARKER}`, undefined, { 'x-marker': ID });
  await send(handler, 'an unknown name in the address', 'GET', `/probe/name/${MARKER}`);
  await send(handler, 'a refused caller', 'GET', `/probe/name/${MARKER}`, undefined, {}, MARKER);

  // The endpoint with every guard: the idempotency key, `If-Match`, the facts, the policy, the
  // transition, the statements and the target check.
  const pay = (note_: string, amount: string, headers: Record<string, string> = {}): Promise<srv.veld.VeldResponse> =>
    send(handler, `pay ${note_}`, 'POST', `/probe/${ID}/pay`, { amount, note: MARKER }, headers);
  await send(handler, 'pay: a body of the wrong type', 'POST', `/probe/${ID}/pay`, { amount: MARKER, note: MARKER });
  await pay('with a bad idempotency key', '5.00', { 'idempotency-key': `${MARKER} \u0001` });
  await pay('with a bad If-Match', '5.00', { 'if-match': MARKER });
  await pay('with a stale version', '5.00', { 'if-match': '"999"' });
  await pay('a statement that fails (named)', '0.00', { 'if-match': '"3"' });
  await pay('a statement that fails (unnamed)', '5000.00', { 'if-match': '"3"' });
  await pay('a statement on a string (named)', '5.00', { 'if-match': '"3"' });
  booking = { ...booking, status: 'paid' };
  await pay('a transition that is not legal', '5.00', { 'if-match': '"3"' });
  booking = { ...booking, status: 'cancelled' };
  await pay('a denial without a message', '5.00', { 'if-match': '"3"' });
  booking = { ...booking, status: 'pending' };
  suspended = true;
  await pay('a denial with a message', '5.00', { 'if-match': '"3"' });
  suspended = false;
  // A body that passes every statement; the handler answers wrongly so that the `ensure` and the target check fail.
  const good = (headers: Record<string, string>): Promise<srv.veld.VeldResponse> =>
    send(handler, 'pay: ok', 'POST', `/probe/${ID}/pay`, { amount: '5.00', note: 'ok-1' }, { 'if-match': '"3"', ...headers });
  resultOverride = { status: 'pending' } as never;
  await good({});
  resultOverride = { revision: -1n as never };
  await good({});
  resultOverride = undefined;
  await good({ 'idempotency-key': 'k1' });

  // The hook of the application is not an answer, but what the runtime tells `onError` is a log line.
  for (const e of errors) note('onError', e instanceof Error ? e.message : undefined);

  // The runtime config: the marker as a value of every setting and as a variable's content.
  for (const value of [{ rules: { output: MARKER } }, { rules: MARKER }, MARKER, 7]) {
    try {
      srv.checkRuntime(value);
    } catch (e) {
      assert.ok(e instanceof srv.VeldConfigError);
      note('runtime config', e.message);
      for (const p of e.problems) note('runtime config problem', p.message);
    }
  }
  try {
    srv.fromEnv({ 'rules.output': 'OUTPUT' }, { OUTPUT: MARKER });
  } catch (e) {
    assert.ok(e instanceof srv.VeldConfigError);
    note('runtime config from the environment', e.message);
  }
}

async function clientCases(): Promise<void> {
  const calls: string[] = [];
  const answer = (status: number, body: string) => async (url: string): Promise<cli.veld.FetchResponse> => {
    calls.push(url);
    return { status, text: async () => body };
  };
  const make = (fetch: cli.veld.FetchLike): cli.ProbeClient => new cli.ProbeClient({ baseUrl: 'http://probe.test', fetch, auth: { bearer: () => 'me' } });
  const refused = async (where: string, p: Promise<unknown>): Promise<unknown> => {
    const e = await p.then(
      () => null,
      (err: unknown) => err,
    );
    assert.ok(e !== null, `${where}: the call failed`);
    const err = e as { message?: unknown; title?: unknown; data?: { issues?: { message: string }[] }; violations?: { message: string }[] };
    note(`client ${where} message`, err.message);
    note(`client ${where} title`, err.title);
    for (const i of err.data?.issues ?? []) note(`client ${where} issue`, i.message);
    for (const v of err.violations ?? []) note(`client ${where} violation`, v.message);
    return e;
  };

  // The local check: every issue of a body and the invariants.
  const client = make(answer(200, '{}'));
  await refused('shape', client.shapes({ body: shape({ id: MARKER, name: MARKER, code: MARKER, count: MARKER, price: MARKER, status: MARKER, pay: { kind: MARKER } }) as never }));
  await refused('rule', client.shapes({ body: shape({ count: 3 }) as never }));
  await refused('rule that cannot be evaluated', client.shapes({ body: shape({ count: 4 }) as never }));
  await refused('path', client.byId({ id: MARKER, size: 1, trace: ID }));
  await refused('query', client.byId({ id: ID, size: MARKER as never, trace: ID }));
  assert.deepEqual(calls, [], 'nothing was sent');

  // Transport failures: the address holds the marker (a path parameter), and so do the failure and the body.
  await refused('a fetch that fails', make(async () => { throw new Error(MARKER); }).byName({ name: MARKER }));
  await refused('a body that is not JSON', make(answer(200, `{"x": "${MARKER}`)).byName({ name: MARKER }));
  await refused('a body that is not readable', make(async () => ({ status: 200, text: async () => { throw new Error(MARKER); } })).byName({ name: MARKER }));
  await refused('an empty body', make(answer(200, '')).byName({ name: MARKER }));
  await refused('a status that is not declared', make(answer(418, `{"detail": "${MARKER}"}`)).byName({ name: MARKER }));
  await refused('a validation error with a violation', make(answer(422, JSON.stringify({ type: 'about:blank', title: 'ValidationError', status: 422, data: { issues: [] }, violations: [] }))).byName({ name: MARKER }));
  // A client built with no address says so without the value.
  try {
    new cli.ProbeClient({ baseUrl: '' });
  } catch (e) {
    note('client without an address', e instanceof Error ? e.message : undefined);
  }
}

async function main(): Promise<void> {
  const warnings: string[] = [];
  const original = console.warn;
  console.warn = (...args: unknown[]): void => void warnings.push(args.map(String).join(' '));
  try {
    await serverCases();
    await clientCases();
    probe({ validateRequests: false });
    probe({ rules: { output: 'audit' } });
  } finally {
    console.warn = original;
  }
  for (const w of warnings) note('console.warn', w);

  assert.ok(texts.length > 40, `texts were collected (${String(texts.length)})`);
  for (const { where, text } of texts) {
    assert.equal(text.includes(MARKER), false, `${where} repeats the value: ${text}`);
  }
  process.stdout.write('no values ok\n');
}

main().catch((e: unknown) => {
  process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  process.exit(1);
});
