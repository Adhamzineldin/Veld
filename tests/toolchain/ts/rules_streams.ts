// Invariants on declared errors, SSE items and WebSocket messages (spec/rules.md section 5.2): a generated
// Node server and a generated client over real sockets. Expects `./server` (node) and `./client` (typescript)
// generated from tests/toolchain/contract/rules_live.
import assert from 'node:assert/strict';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import { WebSocket as RawWebSocket, WebSocketServer } from 'ws';
import * as cli from './client/index';
import { LiveClient } from './client/live/client';
import * as srv from './server/index';
import type { RuleViolationEvent } from './server/runtime/veld';
import { attachWebSocket } from './server/server/ws';
import { impl, received } from './rules_live_impl';

const errors: unknown[] = [];

interface Running {
  url: string;
  events: RuleViolationEvent[];
  close(): Promise<void>;
}

async function start(output?: 'strict' | 'audit' | 'off'): Promise<Running> {
  const events: RuleViolationEvent[] = [];
  const handler = srv.createLiveHandler(
    impl,
    { onRuleViolation: (e) => events.push(e), onError: (e) => errors.push(e) },
    output === undefined ? undefined : { rules: { output } },
  );
  const server = http.createServer(srv.toNodeHttp(handler));
  const wss = new WebSocketServer({ server });
  attachWebSocket(wss, handler);
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
  const port = (server.address() as AddressInfo).port;
  return {
    url: `http://127.0.0.1:${String(port)}`,
    events,
    close: () =>
      new Promise<void>((resolve) => {
        for (const c of wss.clients) c.terminate();
        server.closeAllConnections();
        server.close(() => resolve());
      }),
  };
}

async function seats(url: string, ok: boolean): Promise<{ status: number; text: string }> {
  const r = await fetch(`${url}/live/seats?ok=${String(ok)}`);
  return { status: r.status, text: await r.text() };
}

async function readings(url: string, bad: number): Promise<{ got: { low: number; high: number }[]; error: unknown }> {
  const got: { low: number; high: number }[] = [];
  let error: unknown;
  try {
    for await (const r of new LiveClient({ baseUrl: url }).readings({ bad })) got.push(r);
  } catch (e) {
    error = e;
  }
  return { got, error };
}

function lastRule(events: RuleViolationEvent[]): string {
  return events[events.length - 1]?.rule ?? '';
}

async function errorTests(): Promise<void> {
  const strict = await start();
  const ok = await seats(strict.url, true);
  assert.equal(ok.status, 409);
  assert.equal((JSON.parse(ok.text) as { title: string }).title, 'Overbooked');
  assert.equal(strict.events.length, 0);
  // strict: a declared error that violates its own rule is a 500 with no rule detail; the hook gets the violation.
  const bad = await seats(strict.url, false);
  assert.equal(bad.status, 500);
  const internal = JSON.parse(bad.text) as { title: string; data: unknown; violations?: unknown };
  assert.equal(internal.title, 'Internal');
  assert.deepEqual(internal.data, {});
  assert.equal(internal.violations, undefined);
  assert.ok(!bad.text.includes('overflow') && !bad.text.includes('Overbooked'));
  assert.equal(strict.events.length, 1);
  assert.equal(strict.events[0]?.position, 'output');
  assert.equal(strict.events[0]?.outcome, 'rejected');
  assert.match(lastRule(strict.events), /Overbooked\.overflow@/);
  assert.ok(strict.events[0]?.operation.includes('Live.seats'));
  await strict.close();

  // audit: the error is sent unchanged; the hook gets the violation. The client never checks what it decodes.
  const audit = await start('audit');
  const sent = await seats(audit.url, false);
  assert.equal(sent.status, 409);
  assert.equal((JSON.parse(sent.text) as { data: { seats: number } }).data.seats, 3);
  assert.equal(audit.events.length, 1);
  assert.equal(audit.events[0]?.outcome, 'audited');
  try {
    await new LiveClient({ baseUrl: audit.url }).seats({ ok: false });
    assert.fail('expected the declared error');
  } catch (e) {
    assert.ok(cli.Overbooked.matches(e));
    assert.equal(e.data.seats, 3);
    assert.deepEqual(e.violations, []);
    assert.equal(e.local, false);
    assert.equal(cli.Overbooked.check(e.data).length, 1);
  }
  await audit.close();

  // off: not evaluated.
  const off = await start('off');
  assert.equal((await seats(off.url, false)).status, 409);
  assert.equal(off.events.length, 0);
  await off.close();
}

async function sseTests(): Promise<void> {
  const strict = await start();
  assert.equal((await readings(strict.url, -1)).got.length, 3);
  assert.equal(strict.events.length, 0);
  // A violating item in the middle is not sent; the stream ends like any server failure: a final error event.
  const mid = await readings(strict.url, 1);
  assert.deepEqual(mid.got, [{ low: 0, high: 1 }]);
  assert.equal((mid.error as { status?: number }).status, 500);
  assert.equal(strict.events.length, 1);
  assert.equal(strict.events[0]?.outcome, 'rejected');
  assert.match(lastRule(strict.events), /Reading\.ordered@/);
  const raw = await (await fetch(`${strict.url}/live/readings?bad=1`)).text();
  assert.ok(raw.includes('event: error') && !raw.includes('"low":5'), raw);
  // A violating first item is still a normal 500 response: nothing was sent yet.
  const first = await readings(strict.url, 0);
  assert.deepEqual(first.got, []);
  assert.equal((first.error as { status?: number }).status, 500);
  assert.equal((await fetch(`${strict.url}/live/readings?bad=0`)).status, 500);
  assert.ok(errors.length >= 3, 'the server failure reaches onError like any other');
  await strict.close();

  const audit = await start('audit');
  const all = await readings(audit.url, 1);
  assert.deepEqual(all.got.map((r) => r.low), [0, 5, 2]);
  assert.equal(all.error, undefined);
  assert.equal(audit.events.length, 1);
  assert.equal(audit.events[0]?.outcome, 'audited');
  await audit.close();

  const off = await start('off');
  assert.equal((await readings(off.url, 1)).got.length, 3);
  assert.equal(off.events.length, 0);
  await off.close();
}

async function firstOf<T>(it: AsyncIterator<T>): Promise<T | undefined> {
  const step = await it.next();
  return step.done === true ? undefined : step.value;
}

async function rawFrames(url: string, text: string): Promise<{ frames: string[]; code: number }> {
  const sock = new RawWebSocket(`${url.replace(/^http/, 'ws')}/live/notes`);
  const frames: string[] = [];
  sock.on('message', (d) => frames.push(String(d)));
  await new Promise((r) => sock.on('open', r));
  sock.send(text);
  const code = await new Promise<number>((resolve) => sock.on('close', (c) => resolve(c)));
  return { frames, code };
}

async function websocketTests(): Promise<void> {
  const strict = await start();
  // Client to server, checked locally like a request body: nothing is sent.
  const before = received.length;
  // A WebSocket upgrade shows no headers to the client, so another call of the client primes it with
  // the rules digest (section 9.7): before the first response it sends anyway.
  const live = new LiveClient({ baseUrl: strict.url });
  await assert.rejects(live.seats({ ok: true }), (e: unknown) => cli.Overbooked.matches(e));
  const chan = live.notes();
  try {
    chan.send({ text: 'x', stars: 9 });
    assert.fail('expected a local ValidationError');
  } catch (e) {
    assert.ok(cli.ValidationError.matches(e));
    assert.equal(e.local, true);
    assert.equal(e.violations[0]?.path, 'message');
    assert.deepEqual(e.violations[0]?.fields, ['stars']);
    assert.equal(e.data.issues[0]?.code, 'rule');
  }
  chan.send({ text: 'fine', stars: 3 });
  assert.deepEqual(await firstOf(chan[Symbol.asyncIterator]()), { low: 0, high: 3 });
  chan.close();
  assert.deepEqual(received.slice(before), ['fine']);
  // With the local check off the server rejects the message the way it rejects a wrong shape, with violations.
  const remote = new LiveClient({ baseUrl: strict.url, rules: false }).notes();
  remote.send({ text: 'sneaky', stars: 9 });
  try {
    await firstOf(remote[Symbol.asyncIterator]());
    assert.fail('expected a ValidationError');
  } catch (e) {
    assert.ok(cli.ValidationError.matches(e));
    assert.equal(e.local, false);
    assert.equal(e.violations.length, 1);
    assert.equal(e.violations[0]?.path, 'message');
    assert.match(e.violations[0]?.rule ?? '', /Note\.starRange@/);
  }
  assert.ok(!received.includes('sneaky'));
  const raw = await rawFrames(strict.url, '{"stars":9,"text":"raw"}');
  assert.equal(raw.code, 1008);
  assert.match(raw.frames[0] ?? '', /^\{"error":\{.*"title":"ValidationError".*"violations":\[\{.*"path":"message"/);
  assert.ok(strict.events.some((e) => e.position === 'input' && e.outcome === 'rejected'));
  // Server to client: a violating message is not sent; the socket ends like any server failure (1011).
  strict.events.length = 0;
  const bad = await rawFrames(strict.url, '{"stars":1,"text":"bad"}');
  assert.equal(bad.code, 1011);
  assert.equal(bad.frames.length, 1);
  assert.match(bad.frames[0] ?? '', /^\{"error":\{.*"title":"Internal"/);
  assert.ok(!(bad.frames[0] ?? '').includes('"low":9'));
  assert.equal(strict.events.length, 1);
  assert.equal(strict.events[0]?.position, 'output');
  assert.equal(strict.events[0]?.outcome, 'rejected');
  await strict.close();

  const audit = await start('audit');
  const loose = new LiveClient({ baseUrl: audit.url }).notes();
  loose.send({ text: 'bad', stars: 1 });
  assert.deepEqual(await firstOf(loose[Symbol.asyncIterator]()), { low: 9, high: 1 });
  loose.close();
  assert.equal(audit.events.length, 1);
  assert.equal(audit.events[0]?.outcome, 'audited');
  await audit.close();
}

async function main(): Promise<void> {
  const warnings: string[] = [];
  const warn = console.warn;
  console.warn = (m: unknown) => warnings.push(String(m));
  try {
    await errorTests();
    await sseTests();
    await websocketTests();
  } finally {
    console.warn = warn;
  }
  // One report per handler built with a mode other than `strict`: audit and off in the error and SSE tests, audit for WebSocket.
  assert.equal(warnings.length, 5, warnings.join('\n'));
  console.log('rules streams ok');
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
