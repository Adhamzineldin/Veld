// Streaming round trip: the generated TypeScript client against the generated Node server over a
// real socket. SSE: ordered items, abort, typed errors before and after the first event, resume with
// Last-Event-ID, framing on the wire. WebSocket: echo, validation, typed errors, auth.
import assert from 'node:assert/strict';
import { WebSocket as RawWebSocket } from 'ws';
import * as cli from './client/index';
import { LiveClient, VaultClient } from './client/live/client';
import { seen, start } from './stream_server';

async function until(label: string, cond: () => boolean): Promise<void> {
  for (let i = 0; i < 300; i += 1) {
    if (cond()) return;
    await new Promise((r) => setTimeout(r, 10));
  }
  assert.fail(`timed out waiting for: ${label}`);
}

async function collect<T>(it: AsyncIterable<T>): Promise<T[]> {
  const out: T[] = [];
  for await (const x of it) out.push(x);
  return out;
}

async function rejection(p: Promise<unknown>): Promise<unknown> {
  try {
    await p;
  } catch (e) {
    return e;
  }
  return assert.fail('expected a rejection');
}

export async function sseTests(base: string): Promise<void> {
  const live = new LiveClient({ baseUrl: base });
  const foreverBefore = seen.foreverClosed;

  // Ordered items, decoded to the contract type.
  const ticks = await collect(live.ticks({ count: 5 }));
  assert.deepEqual(
    ticks.map((t) => t.n),
    [0, 1, 2, 3, 4],
  );
  assert.equal(ticks[2]?.label, 'tick 2');
  assert.equal(ticks[0]?.when, '2024-05-01T12:00:00Z');
  assert.deepEqual(await collect(live.ticks({ count: 0 })), []);

  // A declared error before the first event is a normal problem+json response, thrown typed.
  const early = await rejection(collect(live.ticks({ count: -1 })));
  assert.ok(cli.QuotaExceeded.matches(early), 'typed QuotaExceeded before the first event');
  assert.equal((early as cli.QuotaExceeded).data.limit, 0);
  assert.equal((early as cli.QuotaExceeded).status, 429);
  assert.ok((early as cli.QuotaExceeded).op?.includes('Live.ticks'), 'op carries the operation signature');

  // A declared error after streaming has started arrives as the final `event: error`.
  const got: number[] = [];
  let late: unknown;
  try {
    for await (const t of live.ticks({ count: 10, failAt: 2 })) got.push(t.n);
  } catch (e) {
    late = e;
  }
  assert.deepEqual(got, [0, 1]);
  assert.ok(cli.QuotaExceeded.matches(late), 'typed QuotaExceeded mid-stream');
  assert.equal((late as cli.QuotaExceeded).data.limit, 2);
  assert.ok(cli.veld.same(early, new cli.QuotaExceeded({ limit: 0 }, { op: (early as cli.QuotaExceeded).op })));
  assert.ok(!cli.veld.same(early, late), 'different data is not the same error');

  // Abort mid-stream: the iteration ends quietly and the server sees the client go away.
  const ctl = new AbortController();
  const seenN: number[] = [];
  for await (const t of live.forever({}, { signal: ctl.signal })) {
    seenN.push(t.n);
    if (t.n === 2) ctl.abort();
  }
  assert.deepEqual(seenN.slice(0, 3), [0, 1, 2]);
  await until('the server cleans up the aborted stream', () => seen.foreverClosed > foreverBefore);
  // Breaking out of the loop closes the request as well.
  const before = seen.foreverClosed;
  for await (const t of live.forever({})) if (t.n === 1) break;
  await until('the server cleans up after break', () => seen.foreverClosed > before);

  // Event names, ids and resuming with Last-Event-ID.
  const ids: string[] = [];
  const first = await collect(live.roomEvents({ room: 'r' }, { onEventId: (id) => ids.push(id) }));
  assert.deepEqual(
    first.map((t) => t.n),
    [0, 1, 2],
  );
  assert.deepEqual(ids, ['0', '1', '2']);
  const resumed = await collect(live.roomEvents({ room: 'r' }, { lastEventId: ids[ids.length - 1] ?? '' }));
  assert.deepEqual(
    resumed.map((t) => t.n),
    [3, 4, 5],
  );
  const missing = await rejection(collect(live.roomEvents({ room: 'nope' })));
  assert.ok(cli.NoSuchRoom.matches(missing));
  assert.equal((missing as cli.NoSuchRoom).data.room, 'nope');

  // Local request validation happens before anything is sent.
  const invalid = await rejection(collect(live.ticks({} as never)));
  assert.equal((invalid as cli.VeldError).title, 'ValidationError');

  // The wire: text/event-stream, canonical JSON in `data:`, ids and event names, a final error event.
  const res = await fetch(`${base}/live/ticks?count=2`);
  assert.equal(res.status, 200);
  assert.ok((res.headers.get('content-type') ?? '').startsWith('text/event-stream'));
  assert.equal(
    await res.text(),
    'data: {"label":"tick 0","n":0,"when":"2024-05-01T12:00:00Z"}\n\n' +
      'data: {"label":"tick 1","n":1,"when":"2024-05-01T12:00:00Z"}\n\n',
  );
  const named = await (await fetch(`${base}/live/rooms/r/events`)).text();
  assert.ok(named.startsWith('id: 0\nevent: tick\ndata: {"label"'), named);
  const failed = await (await fetch(`${base}/live/ticks?count=3&failAt=1`)).text();
  assert.match(failed, /^data: .*\n\nevent: error\ndata: \{.*"data":\{"limit":1\}.*\}\n\n$/s);
  const refused = await fetch(`${base}/live/ticks?count=-1`);
  assert.equal(refused.status, 429);
  // Fastify appends a charset to string payloads; the media type is what matters.
  assert.ok((refused.headers.get('content-type') ?? '').startsWith('application/problem+json'));
  const unknown = await fetch(`${base}/live/ticks?count=x`);
  assert.equal(unknown.status, 422);
}

export async function validatedSseTests(base: string): Promise<void> {
  const live = new LiveClient({ baseUrl: base });
  // With validateResponses a bad first item is a normal 500 Internal; a later one ends the stream
  // with `event: error` (Internal).
  const first = await rejection(collect(live.roomEvents({ room: 'bad' })));
  assert.equal((first as cli.VeldError).title, 'Internal');
  assert.equal((first as cli.VeldError).status, 500);
  const got: number[] = [];
  let later: unknown;
  try {
    for await (const t of live.roomEvents({ room: 'bad2' })) got.push(t.n);
  } catch (e) {
    later = e;
  }
  assert.deepEqual(got, [0]);
  assert.equal((later as cli.VeldError).title, 'Internal');
}

export async function wsTests(base: string): Promise<void> {
  const live = new LiveClient({ baseUrl: base });
  const chatBefore = seen.chatEnded;

  // Bidirectional chat: messages are queued until the socket opens, replies are typed.
  const chat = live.chat({ room: 'r1', nick: 'ada' });
  chat.send({ text: 'hi' });
  chat.send({ text: 'yo', loud: true });
  const it = chat[Symbol.asyncIterator]();
  assert.deepEqual((await it.next()).value, { from: 'ada', text: 'hi', seq: 0 });
  assert.deepEqual((await it.next()).value, { from: 'ada', text: 'YO', seq: 1 });
  chat.close();
  await until('the server sees the chat end', () => seen.chatEnded > chatBefore);

  // The client validates outgoing messages.
  const guarded = live.chat({ room: 'r2' });
  assert.throws(
    () => guarded.send({ text: '' }),
    (e: unknown) => (e as cli.VeldError).title === 'ValidationError',
  );
  guarded.close();

  // The server validates incoming messages: a bad one comes back as a typed ValidationError.
  const lax = new LiveClient({ baseUrl: base, validate: false });
  const bad = lax.chat({ room: 'r3' });
  bad.send({ text: '' });
  const failure = await rejection(collect(bad));
  assert.equal((failure as cli.VeldError).title, 'ValidationError');
  assert.equal((failure as cli.VeldError).status, 422);
  const issues = ((failure as cli.VeldError).data as { issues: { path: string }[] }).issues;
  assert.ok(issues.some((i) => i.path.startsWith('message')), JSON.stringify(issues));

  // A declared error closes the socket after a problem frame, typed on the client.
  const locked = await rejection(collect(live.chat({ room: 'locked' })));
  assert.ok(cli.NoSuchRoom.matches(locked));
  assert.ok((locked as cli.NoSuchRoom).op?.includes('Live.chat'));

  // Server-only and client-only endpoints.
  assert.deepEqual(
    (await collect(live.announce())).map((t) => t.n),
    [0, 1, 2],
  );
  seen.sink.length = 0;
  const sink = live.sink();
  sink.send({ text: 'a' });
  sink.send({ text: 'b' });
  await collect(sink);
  assert.deepEqual(seen.sink, ['a', 'b']);

  // Aborting the signal closes the socket and ends the iteration.
  const ctl = new AbortController();
  const aborted = live.chat({ room: 'r4' }, { signal: ctl.signal });
  const ended = seen.chatEnded;
  aborted.send({ text: 'ping' });
  const reader = aborted[Symbol.asyncIterator]();
  assert.equal(((await reader.next()).value as { text: string }).text, 'ping');
  ctl.abort();
  assert.equal((await reader.next()).done, true);
  await until('the server sees the aborted chat', () => seen.chatEnded > ended);

  // A path that is not a ws endpoint is closed with 4404; the raw frames are envelopes of canonical JSON.
  const wsBase = base.replace(/^http/, 'ws');
  const nowhere = new RawWebSocket(`${wsBase}/nowhere`);
  const code = await new Promise<number>((resolve) => nowhere.on('close', (c) => resolve(c)));
  assert.equal(code, 4404);
  const raw = new RawWebSocket(`${wsBase}/live/chat/raw?nick=zed`);
  await new Promise((r) => raw.on('open', r));
  raw.send('{"loud":false,"text":"a"}');
  const frame = await new Promise<string>((resolve) => raw.once('message', (d) => resolve(String(d))));
  assert.equal(frame, '{"data":{"from":"zed","seq":0,"text":"a"}}');
  raw.send('not json');
  const problem = await new Promise<string>((resolve) => raw.once('message', (d) => resolve(String(d))));
  assert.match(problem, /^\{"error":\{.*"title":"ValidationError".*"status":422/);
  assert.equal(await new Promise<number>((resolve) => raw.on('close', (c) => resolve(c))), 1008);

  // Unreachable server: a transport error, not a hang.
  const dead = new LiveClient({ baseUrl: 'http://127.0.0.1:1' }).announce();
  const refused = await rejection(collect(dead));
  assert.equal((refused as Error).name, 'VeldTransportError');
}

export async function authTests(base: string): Promise<void> {
  const anonymous = new VaultClient({ baseUrl: base });
  const denied = await rejection(collect(anonymous.feed()));
  assert.equal((denied as cli.VeldError).title, 'Unauthorized');
  assert.equal((denied as cli.VeldError).status, 401);
  const wrong = new VaultClient({ baseUrl: base, auth: { bearer: () => 'bad' } });
  assert.equal(((await rejection(collect(wrong.feed()))) as cli.VeldError).title, 'Unauthorized');

  const vault = new VaultClient({ baseUrl: base, auth: { bearer: () => 'good' } });
  assert.deepEqual(
    (await collect(vault.feed())).map((t) => t.n),
    [0, 1],
  );

  // WebSocket auth rejection: the same hook, a problem frame, close 1008.
  const rejected = anonymous.echo();
  rejected.send({ text: 'hi' });
  const failure = await rejection(collect(rejected));
  assert.equal((failure as cli.VeldError).title, 'Unauthorized');
  const badToken = new VaultClient({ baseUrl: base, auth: { bearer: () => 'bad' } }).echo();
  assert.equal(((await rejection(collect(badToken))) as cli.VeldError).title, 'Unauthorized');

  // With the token (query parameter from the browser-safe client) it works.
  const echo = vault.echo();
  echo.send({ text: 'one' });
  echo.send({ text: 'two' });
  const it = echo[Symbol.asyncIterator]();
  assert.deepEqual((await it.next()).value, { from: 'vault', text: 'one', seq: 0 });
  assert.deepEqual((await it.next()).value, { from: 'vault', text: 'two', seq: 1 });
  echo.close();

  // The same hook also accepts the Authorization header (non-browser clients).
  const wsBase = base.replace(/^http/, 'ws');
  const raw = new RawWebSocket(`${wsBase}/vault/echo`, { headers: { authorization: 'Bearer good' } });
  await new Promise((r) => raw.on('open', r));
  raw.send('{"text":"h"}');
  assert.equal(await new Promise<string>((resolve) => raw.once('message', (d) => resolve(String(d)))), '{"data":{"from":"vault","seq":0,"text":"h"}}');
  raw.close();
  assert.ok(seen.principals.length >= 3);
}

async function main(): Promise<void> {
  const server = await start();
  const checked = await start({ validateResponses: true });
  try {
    await sseTests(server.url);
    console.log('sse ok');
    await validatedSseTests(checked.url);
    console.log('validated sse ok');
    await wsTests(server.url);
    console.log('ws ok');
    await authTests(server.url);
    console.log('auth ok');
  } finally {
    await server.close();
    await checked.close();
  }
  console.log('stream round trip ok');
}

if (require.main === module) {
  main().then(
    () => process.exit(0),
    (err: unknown) => {
      console.error(err);
      process.exit(1);
    },
  );
}
