// Runtime test of the generated Vue stream composables, in an effect scope, against a live SSE
// endpoint and a WebSocket channel of the REAL generated server.
import assert from 'node:assert/strict';
import { effectScope, nextTick, ref } from 'vue';
import { LiveClient } from './client_vue/live/client';
import type { TicksRequest } from './client_vue/live/client';
import { QuotaExceeded } from './client_vue/live/errors/live';
import { useChatStream, useForeverStream, useTicksStream } from './client_vue/live/vue';
import { seen, start } from './stream_server';

/** Reads a ref through a function so the compiler does not narrow what the assertions already checked. */
const valueOf = <T,>(r: { readonly value: T }): T => r.value;

async function until(label: string, cond: () => boolean): Promise<void> {
  for (let i = 0; i < 300; i += 1) {
    if (cond()) return;
    await nextTick();
    await new Promise((r) => setTimeout(r, 10));
  }
  assert.fail(`timed out waiting for: ${label}`);
}

async function main(): Promise<void> {
  const server = await start();
  const client = new LiveClient({ baseUrl: server.url });
  try {
    const scope = effectScope();
    const req = ref<TicksRequest | null>({ count: 3 });
    const ticks = scope.run(() => useTicksStream(req, { client }));
    assert.ok(ticks);
    await until('three ticks', () => valueOf(ticks.items).length === 3 && !valueOf(ticks.streaming));
    assert.deepEqual(valueOf(ticks.items).map((t) => t.n), [0, 1, 2]);

    // A changed request starts over; a typed error ends the stream and keeps the items so far.
    req.value = { count: 9, failAt: 2 };
    await until('the stream failed', () => valueOf(ticks.error) !== undefined);
    assert.ok(QuotaExceeded.matches(valueOf(ticks.error)));
    assert.deepEqual(valueOf(ticks.items).map((t) => t.n), [0, 1]);
    req.value = null;
    await until('held back', () => !valueOf(ticks.streaming));
    scope.stop();

    // Scope disposal aborts an endless stream.
    const before = seen.foreverClosed;
    const endless = effectScope();
    const forever = endless.run(() => useForeverStream({}, { client, max: 2 }));
    assert.ok(forever);
    await until('ticks', () => (valueOf(forever.latest)?.n ?? 0) >= 3);
    assert.ok(valueOf(forever.items).length <= 2);
    endless.stop();
    await until('the server cleans up', () => seen.foreverClosed > before);

    // A WebSocket channel: send plus typed replies.
    const room = effectScope();
    const chat = room.run(() => useChatStream({ room: 'vue', nick: 'eve' }, { client }));
    assert.ok(chat);
    chat.send({ text: 'hi' });
    await until('the echo', () => valueOf(chat.items).length === 1);
    assert.deepEqual(valueOf(chat.items)[0], { from: 'eve', text: 'hi', seq: 0 });
    room.stop();
  } finally {
    await server.close();
  }
  console.log('vue stream composables ok');
}

main().then(
  () => process.exit(0),
  (err: unknown) => {
    console.error(err);
    process.exit(1);
  },
);
