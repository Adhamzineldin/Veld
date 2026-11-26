// The audit log (spec/platform.md section 4) over the production handler of
// tests/toolchain/contract/platform_p1/audit.veld: the event of a success, of an early answer
// (`401`, `422`, `404`), of a handler that throws and of a deadline; the one reading of the clock;
// a hook that fails (the answer is sent unchanged and `onError` hears the failure with the event);
// an endpoint that is not audited; and the settings that change nothing. Expects `./server`
// generated from that contract (compiled as the package `bookings`).
import assert from 'node:assert/strict';
import * as srv from './server/index';

const ID = '00000000-0000-4000-8000-000000000001';
const USER = { id: '00000000-0000-4000-8000-0000000000aa', email: 'me@example.test' };

const events: srv.veld.AuditEvent[] = [];
const failures: unknown[] = [];
let clockCalls = 0;
let hook: (event: srv.veld.AuditEvent) => void | Promise<void> = (event) => {
  events.push(event);
};
let note: srv.Note = { id: ID, text: 'hello', secret: 's3cr3t' };
let fact: srv.Note | undefined;
let mode: 'ok' | 'throw' | 'slow' = 'ok';

const impl: srv.NotesService = {
  getNote: async () => note,
  addNote: async (req) => req.body,
  viaFact: async (req) => req.note,
  boom: async () => {
    if (mode === 'throw') throw new Error('the application failed');
    return note;
  },
  slow: async () => {
    await new Promise<void>((resolve) => setTimeout(resolve, 200));
    return note;
  },
  quiet: async () => note,
};

function notes(extra: Record<string, unknown> = {}): srv.veld.VeldHandler {
  const hooks = {
    auth: { bearer: async (token: string) => (token === 'me' ? USER : null) },
    facts: { note: async () => fact },
    clock: () => {
      clockCalls++;
      return new Date('2026-06-10T12:00:02.007Z');
    },
    audit: (event: srv.veld.AuditEvent) => hook(event),
    onError: (err: unknown) => failures.push(err),
    ...extra,
  } as srv.NotesHooks;
  return srv.createNotesHandler(impl, hooks);
}

async function send(handler: srv.veld.VeldHandler, method: string, path: string, token?: string, body?: unknown): Promise<srv.veld.VeldResponse> {
  const headers: Record<string, string> = token === undefined ? {} : { authorization: `Bearer ${token}` };
  const res = await handler({ method, path, headers, ...(body === undefined ? {} : { body }) });
  assert.ok(res !== null, `${method} ${path} matched no route`);
  return res;
}

function only(): srv.veld.AuditEvent {
  assert.equal(events.length, 1, 'the hook is called once');
  const [event] = events;
  assert.ok(event !== undefined);
  return event;
}

async function main(): Promise<void> {
  const handler = notes();

  // A success: the result is in the event without the secret; the principal without its email.
  let res = await send(handler, 'GET', `/notes/${ID}`, 'me');
  assert.equal(res.status, 200);
  let event = only();
  assert.equal(event.outcome.status, 200);
  assert.equal(event.outcome.error, undefined);
  assert.equal(event.time, '2026-06-10T12:00:02.007Z');
  assert.deepEqual(event.principal, { id: USER.id });
  assert.deepEqual(event.parameters, { id: ID });
  assert.deepEqual(event.facts, {});
  assert.deepEqual(event.result, { id: ID, text: 'hello' });
  assert.deepEqual(event.redacted, ['principal.email', 'result.secret']);
  assert.deepEqual(event.hidden, []);
  assert.ok(event.operation.startsWith('veld:bookings/Notes.getNote@'));
  assert.equal(JSON.parse(res.body ?? 'null').secret, 's3cr3t', 'the caller is sent the whole value: the redaction is of the event');

  // A parameter with a secret.
  events.length = 0;
  res = await send(handler, 'POST', '/notes', 'me', { id: ID, text: 'new', secret: 'x' });
  assert.equal(res.status, 201);
  event = only();
  assert.deepEqual(event.parameters, { body: { id: ID, text: 'new' } });
  assert.ok(event.redacted.includes('parameters.body.secret'));

  // A fact: wire form, redacted, and the clock was read once for the whole request.
  events.length = 0;
  clockCalls = 0;
  fact = { id: ID, text: 'f', secret: 'x' };
  res = await send(handler, 'GET', `/notes/fact/${ID}`, 'me');
  assert.equal(res.status, 200);
  event = only();
  assert.deepEqual(event.facts, { note: { id: ID, text: 'f' } });
  assert.ok(event.redacted.includes('facts.note.secret'));
  assert.equal(clockCalls, 1, 'the clock is read once per request');

  // A fact that is not found: `404`, audited, with no fact.
  events.length = 0;
  fact = undefined;
  res = await send(handler, 'GET', `/notes/fact/${ID}`, 'me');
  assert.equal(res.status, 404);
  event = only();
  assert.deepEqual(event.outcome, { status: 404, error: 'NotFound' });
  assert.deepEqual(event.facts, {});
  assert.equal(event.result, undefined, 'an error of the prelude has no result');

  // A refused caller: `401`, audited, with no principal and no parameters.
  events.length = 0;
  res = await send(handler, 'GET', `/notes/${ID}`, 'nobody');
  assert.equal(res.status, 401);
  event = only();
  assert.deepEqual(event.outcome, { status: 401, error: 'Unauthorized' });
  assert.equal('principal' in event, false);
  assert.deepEqual(event.parameters, {});
  assert.deepEqual(event.redacted, []);

  // A request that fails validation: `422`, audited, with the parameters bound so far (none).
  events.length = 0;
  res = await send(handler, 'GET', '/notes/not-a-uuid', 'me');
  assert.equal(res.status, 422);
  event = only();
  assert.deepEqual(event.outcome, { status: 422, error: 'ValidationError' });
  assert.deepEqual(event.parameters, {});

  // A handler that throws: the caller gets a `500`, the event records it, and the failure is reported.
  events.length = 0;
  failures.length = 0;
  mode = 'throw';
  res = await send(handler, 'GET', `/notes/boom/${ID}`, 'me');
  assert.equal(res.status, 500);
  event = only();
  assert.deepEqual(event.outcome, { status: 500, error: 'Internal' });
  assert.equal(event.result, undefined);
  mode = 'ok';

  // A deadline: the `504` is audited too, once, even though the handler runs on.
  events.length = 0;
  res = await send(handler, 'GET', `/notes/slow/${ID}`, 'me');
  assert.equal(res.status, 504);
  event = only();
  assert.equal(event.outcome.status, 504);
  assert.equal(event.outcome.error, 'DeadlineExceeded');

  // An endpoint that is not audited does not call the hook.
  events.length = 0;
  res = await send(handler, 'GET', `/notes/quiet/${ID}`, 'me');
  assert.equal(res.status, 200);
  assert.equal(events.length, 0);

  // A hook that throws, and one that rejects: the answer is sent unchanged, `onError` hears the
  // failure with the event.
  for (const make of [
    (): void => {
      throw new Error('the log is full');
    },
    (): Promise<void> => Promise.reject(new Error('the log is full')),
  ]) {
    failures.length = 0;
    hook = make;
    res = await send(handler, 'GET', `/notes/${ID}`, 'me');
    assert.equal(res.status, 200, 'the answer is sent unchanged');
    assert.deepEqual(JSON.parse(res.body ?? 'null'), { id: ID, text: 'hello', secret: 's3cr3t' });
    assert.equal(failures.length, 1);
    const failure = failures[0];
    assert.ok(failure instanceof srv.veld.AuditFailure);
    assert.equal(failure.event.outcome.status, 200);
    assert.ok(failure.reason instanceof Error);
  }
  hook = (e) => {
    events.push(e);
  };

  // A server created without the hook does not start.
  assert.throws(() => srv.createNotesHandler(impl, { auth: { bearer: async () => USER }, facts: { note: async () => fact } } as unknown as srv.NotesHooks), /hooks\.audit is required/);

  // No setting turns it off: request validation off, rule output off.
  for (const extra of [{ validateRequests: false }, { rules: { output: 'off' as const } }]) {
    events.length = 0;
    res = await send(notes(extra), 'GET', `/notes/${ID}`, 'me');
    assert.equal(res.status, 200);
    assert.equal(only().outcome.status, 200, `audit holds with ${JSON.stringify(extra)}`);
  }

  console.log('audit_http: ok');
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
