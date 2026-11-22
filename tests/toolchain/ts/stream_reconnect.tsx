// Reconnect with exponential backoff for SSE and WebSocket (spec/targets/typescript.md section 4b),
// against a hand-written FLAKY server that drops connections on purpose: the plain client (off by
// default, on with `reconnect`), and the React and Vue hooks, where it is on by default.
import assert from 'node:assert/strict';
import type { AddressInfo } from 'node:net';
import { act, create } from 'react-test-renderer';
import { effectScope, nextTick } from 'vue';
import * as cli from './client/index';
import { LiveClient } from './client/live/client';
import { seen, serve } from './flaky';
import { VeldProvider, useRoomEventsStream } from './client_react/live/react';
import { LiveClient as ReactLiveClient } from './client_react/live/client';
import { LiveClient as VueLiveClient } from './client_vue/live/client';
import { useRoomEventsStream as useVueRoomEventsStream } from './client_vue/live/vue';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const fast = { initialMs: 30, maxMs: 100, retries: 5 };

async function collect<T>(it: AsyncIterable<T>): Promise<T[]> {
  const out: T[] = [];
  for await (const x of it) out.push(x);
  return out;
}

async function failure(p: Promise<unknown>): Promise<unknown> {
  try {
    await p;
  } catch (e) {
    return e;
  }
  return assert.fail('expected a failure');
}

async function sleep(ms: number): Promise<void> {
  await new Promise((r) => setTimeout(r, ms));
}

async function until(label: string, cond: () => boolean, ms = 6000): Promise<void> {
  for (let i = 0; i < ms / 10; i += 1) {
    if (cond()) return;
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
      await nextTick();
    });
  }
  assert.fail(`timed out waiting for: ${label}`);
}

async function sseTests(base: string): Promise<void> {
  const live = new LiveClient({ baseUrl: base });

  // Off by default: a dropped connection is a transport error and nothing is retried.
  const got: number[] = [];
  const dropped = await failure(
    (async () => {
      for await (const t of live.roomEvents({ room: 'resume-plain' })) got.push(t.n);
    })(),
  );
  assert.ok(dropped instanceof cli.VeldTransportError, 'a broken stream is a transport error');
  assert.deepEqual(got, [0, 1]);
  assert.equal(seen('resume-plain').length, 1, 'no reconnect by default');

  // With a policy: the stream resumes with Last-Event-ID and the caller sees one continuous stream.
  const ids: string[] = [];
  const items = await collect(live.roomEvents({ room: 'resume-a' }, { reconnect: fast, onEventId: (id) => ids.push(id) }));
  assert.deepEqual(items.map((t) => t.n), [0, 1, 2, 3]);
  assert.deepEqual(ids, ['0', '1', '2', '3']);
  assert.deepEqual(seen('resume-a').map((c) => c.lastEventId), [undefined, '1'], 'resumed after the last event');
  // `reconnect: false` is the same as not asking for it.
  await failure(collect(live.roomEvents({ room: 'resume-b' }, { reconnect: false })));
  assert.equal(seen('resume-b').length, 1);

  // Backoff: the delay doubles and is capped at maxMs; the last failure is thrown after `retries`.
  const exhausted = await failure(
    collect(live.roomEvents({ room: 'always-a' }, { reconnect: { initialMs: 30, maxMs: 100, retries: 3 } })),
  );
  assert.ok(exhausted instanceof cli.VeldTransportError);
  const at = seen('always-a').map((c) => c.at);
  assert.equal(at.length, 4, 'the first attempt and three retries');
  const gaps = at.slice(1).map((t, i) => t - (at[i] ?? 0));
  assert.ok((gaps[0] ?? 0) >= 25 && (gaps[1] ?? 0) >= 55 && (gaps[2] ?? 0) >= 95, `backoff 30, 60, 100 (capped): ${gaps.join(', ')}`);

  // A typed error is an answer, not a connection failure: no retry.
  const missing = await failure(collect(live.roomEvents({ room: 'missing-a' }, { reconnect: fast })));
  assert.ok(cli.NoSuchRoom.matches(missing), 'typed error surfaces');
  assert.equal(seen('missing-a').length, 1, 'a typed error is never retried');

  // A server that is not up yet: connection refused is retried until it is.
  const probe = await serve();
  const port = (probe.server.address() as AddressInfo).port;
  await new Promise((r) => probe.server.close(r));
  const late = new LiveClient({ baseUrl: `http://127.0.0.1:${String(port)}` });
  const pending = collect(late.roomEvents({ room: 'ok-a' }, { reconnect: { initialMs: 30, maxMs: 60, retries: 30 } }));
  await sleep(150);
  const up = await serve(port);
  try {
    assert.deepEqual((await pending).map((t) => t.n), [0, 1]);
  } finally {
    up.server.closeAllConnections();
    up.server.close();
  }

  // Aborting during the backoff ends the stream quietly.
  const ctl = new AbortController();
  const aborted = collect(live.roomEvents({ room: 'always-b' }, { reconnect: { initialMs: 200, retries: 9 }, signal: ctl.signal }));
  await sleep(60);
  ctl.abort();
  assert.deepEqual(await aborted, []);
  assert.equal(seen('always-b').length, 1);
}

async function wsTests(base: string): Promise<void> {
  const live = new LiveClient({ baseUrl: base });

  // Off by default.
  const plain = live.chat({ room: 'resume-plain' });
  const first: number[] = [];
  const dropped = await failure(
    (async () => {
      for await (const h of plain) first.push(h.seq);
    })(),
  );
  assert.ok(dropped instanceof cli.VeldTransportError);
  assert.deepEqual(first, [0]);
  assert.equal(seen('ws:resume-plain').length, 1);

  // On: the socket is reopened and the iteration continues.
  const heard = await collect(live.chat({ room: 'resume-a' }, { reconnect: fast }));
  assert.deepEqual(heard.map((h) => h.seq), [0, 1, 2]);
  assert.equal(seen('ws:resume-a').length, 2);

  // Messages sent while the connection is down are queued and delivered after the reconnect.
  const channel = live.chat({ room: 'queue-a' }, { reconnect: { initialMs: 200, maxMs: 200, retries: 3 } });
  while (seen('ws:queue-a').length < 1) await sleep(5);
  await sleep(60);
  channel.send({ text: 'later' });
  const echoed = await collect(channel);
  assert.deepEqual(echoed.map((h) => h.text), ['later']);
  assert.equal(seen('ws:queue-a').length, 2);

  // Retries exhausted, and a typed error frame is never retried.
  const gone = await failure(collect(live.chat({ room: 'always-a' }, { reconnect: { initialMs: 20, maxMs: 5000, retries: 2 } })));
  assert.ok(gone instanceof cli.VeldTransportError);
  assert.equal(seen('ws:always-a').length, 3, 'first attempt plus two retries');
  const typed = await failure(collect(live.chat({ room: 'nosuch-a' }, { reconnect: fast })));
  assert.ok(cli.NoSuchRoom.matches(typed));
  assert.equal(seen('ws:nosuch-a').length, 1);

  // close() during the backoff stops reconnecting.
  const closing = live.chat({ room: 'always-b' }, { reconnect: { initialMs: 150, retries: 9 } });
  while (seen('ws:always-b').length < 1) await sleep(5);
  closing.close();
  assert.deepEqual(await collect(closing), []);
  await sleep(250);
  assert.equal(seen('ws:always-b').length, 1);
}

/** The hooks reconnect by default: no policy is passed. */
async function hookTests(base: string): Promise<void> {
  const scope = effectScope();
  const vue = scope.run(() => useVueRoomEventsStream({ room: 'resume-vue' }, { client: new VueLiveClient({ baseUrl: base }) }));
  assert.ok(vue);
  await until('vue stream resumed', () => vue.items.value.length === 4 && !vue.streaming.value);
  assert.deepEqual(vue.items.value.map((t) => t.n), [0, 1, 2, 3]);
  assert.equal(vue.error.value, undefined, 'the drop was not surfaced');
  assert.equal(seen('resume-vue').length, 2);
  scope.stop();

  let result: { items: { n: number }[]; streaming: boolean; error: unknown } | undefined;
  const client = new ReactLiveClient({ baseUrl: base });
  function Room(): null {
    result = useRoomEventsStream({ room: 'resume-react' });
    return null;
  }
  let tree: ReturnType<typeof create> | undefined;
  act(() => {
    tree = create(
      <VeldProvider clients={{ live: client }}>
        <Room />
      </VeldProvider>,
    );
  });
  await until('react stream resumed', () => result?.items.length === 4 && result.streaming === false);
  assert.deepEqual(result?.items.map((t) => t.n), [0, 1, 2, 3]);
  assert.equal(result?.error, undefined);
  assert.equal(seen('resume-react').length, 2);
  act(() => tree?.unmount());
}

async function main(): Promise<void> {
  const { server, base } = await serve();
  try {
    await sseTests(base);
    await wsTests(base);
    await hookTests(base);
    process.stdout.write('reconnect ok\n');
  } finally {
    server.closeAllConnections();
    server.close();
  }
  process.exit(0);
}

main().catch((e: unknown) => {
  process.stderr.write(`${String(e instanceof Error ? (e.stack ?? e.message) : e)}\n`);
  process.exit(1);
});
