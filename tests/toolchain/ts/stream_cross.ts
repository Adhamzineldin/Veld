// The generated TypeScript client for the streaming contract against a server in another process
// (the generated Java server): `node out/stream_cross.js <base url> [full [spring]]`. SSE and auth always;
// WebSocket (echo, validation, typed errors, auth in the query string) with `full`. The server's own
// counters are not visible from here, so only what the client can see is asserted.
import assert from 'node:assert/strict';
import { WebSocket as RawWebSocket } from 'ws';
import * as cli from './client/index';
import { LiveClient, VaultClient } from './client/live/client';

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

async function sseTests(base: string): Promise<void> {
  const live = new LiveClient({ baseUrl: base });

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

  // After streaming has started the error arrives as the final `event: error`.
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

  // Abort mid-stream: the iteration ends quietly.
  const ctl = new AbortController();
  const seenN: number[] = [];
  for await (const t of live.forever({}, { signal: ctl.signal })) {
    seenN.push(t.n);
    if (t.n === 2) ctl.abort();
  }
  assert.deepEqual(seenN.slice(0, 3), [0, 1, 2]);
  for await (const t of live.forever({})) if (t.n === 1) break;

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

  // The wire.
  const res = await fetch(`${base}/live/ticks?count=2`);
  assert.equal(res.status, 200);
  assert.ok((res.headers.get('content-type') ?? '').startsWith('text/event-stream'));
  assert.equal(
    await res.text(),
    'data: {"label":"tick 0","n":0,"when":"2024-05-01T12:00:00Z"}\n\n' +
      'data: {"label":"tick 1","n":1,"when":"2024-05-01T12:00:00Z"}\n\n',
  );
  const refused = await fetch(`${base}/live/ticks?count=-1`);
  assert.equal(refused.status, 429);
  assert.equal(refused.headers.get('content-type'), 'application/problem+json');
}

async function wsTests(base: string, closes4404: boolean): Promise<void> {
  const live = new LiveClient({ baseUrl: base });

  const chat = live.chat({ room: 'r1', nick: 'ada' });
  chat.send({ text: 'hi' });
  chat.send({ text: 'yo', loud: true });
  const it = chat[Symbol.asyncIterator]();
  assert.deepEqual((await it.next()).value, { from: 'ada', text: 'hi', seq: 0 });
  assert.deepEqual((await it.next()).value, { from: 'ada', text: 'YO', seq: 1 });
  chat.close();

  const guarded = live.chat({ room: 'r2' });
  assert.throws(
    () => guarded.send({ text: '' }),
    (e: unknown) => (e as cli.VeldError).title === 'ValidationError',
  );
  guarded.close();

  const lax = new LiveClient({ baseUrl: base, validate: false });
  const bad = lax.chat({ room: 'r3' });
  bad.send({ text: '' });
  const failure = await rejection(collect(bad));
  assert.equal((failure as cli.VeldError).title, 'ValidationError');
  assert.equal((failure as cli.VeldError).status, 422);
  const issues = ((failure as cli.VeldError).data as { issues: { path: string }[] }).issues;
  assert.ok(issues.some((i) => i.path.startsWith('message')), JSON.stringify(issues));

  const locked = await rejection(collect(live.chat({ room: 'locked' })));
  assert.ok(cli.NoSuchRoom.matches(locked));
  assert.ok((locked as cli.NoSuchRoom).op?.includes('Live.chat'));

  assert.deepEqual(
    (await collect(live.announce())).map((t) => t.n),
    [0, 1, 2],
  );
  const sink = live.sink();
  sink.send({ text: 'a' });
  sink.send({ text: 'b' });
  await collect(sink);

  const wsBase = base.replace(/^http/, 'ws');
  const nowhere = new RawWebSocket(`${wsBase}/nowhere`);
  if (closes4404) {
    assert.equal(await new Promise<number>((resolve) => nowhere.on('close', (c) => resolve(c))), 4404);
  } else {
    // Spring: the upgrade itself is refused with a 404, there is no socket to close.
    assert.match(String(await new Promise((resolve) => nowhere.on('error', resolve))), /404/);
  }
  const raw = new RawWebSocket(`${wsBase}/live/chat/raw?nick=zed`);
  await new Promise((r) => raw.on('open', r));
  raw.send('{"loud":false,"text":"a"}');
  assert.equal(await new Promise<string>((resolve) => raw.once('message', (d) => resolve(String(d)))), '{"data":{"from":"zed","seq":0,"text":"a"}}');
  raw.send('not json');
  const problem = await new Promise<string>((resolve) => raw.once('message', (d) => resolve(String(d))));
  assert.match(problem, /^\{"error":\{.*"title":"ValidationError".*"status":422/);
  assert.equal(await new Promise<number>((resolve) => raw.on('close', (c) => resolve(c))), 1008);
}

async function authTests(base: string, ws: boolean): Promise<void> {
  const anonymous = new VaultClient({ baseUrl: base });
  const denied = await rejection(collect(anonymous.feed()));
  assert.equal((denied as cli.VeldError).title, 'Unauthorized');
  assert.equal((denied as cli.VeldError).status, 401);
  const vault = new VaultClient({ baseUrl: base, auth: { bearer: () => 'good' } });
  assert.deepEqual(
    (await collect(vault.feed())).map((t) => t.n),
    [0, 1],
  );
  if (!ws) return;

  const rejected = anonymous.echo();
  rejected.send({ text: 'hi' });
  assert.equal(((await rejection(collect(rejected))) as cli.VeldError).title, 'Unauthorized');
  const echo = vault.echo();
  echo.send({ text: 'one' });
  echo.send({ text: 'two' });
  const it = echo[Symbol.asyncIterator]();
  assert.deepEqual((await it.next()).value, { from: 'vault', text: 'one', seq: 0 });
  assert.deepEqual((await it.next()).value, { from: 'vault', text: 'two', seq: 1 });
  echo.close();
}

async function main(): Promise<void> {
  const base = process.argv[2];
  if (base === undefined) throw new Error('usage: stream_cross <base url> [full]');
  const full = process.argv[3] === 'full';
  await sseTests(base);
  console.log('sse ok');
  if (full) {
    await wsTests(base, process.argv[4] !== 'spring');
    console.log('ws ok');
  }
  await authTests(base, full);
  console.log('auth ok');
  console.log('ts stream client against java server ok');
}

main().then(
  () => process.exit(0),
  (err: unknown) => {
    console.error(err);
    process.exit(1);
  },
);
