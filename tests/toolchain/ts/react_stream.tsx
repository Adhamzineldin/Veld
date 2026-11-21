// Runtime test of the generated React stream hooks: components rendered with react-test-renderer
// consume a live Server-Sent Events endpoint and a WebSocket channel of the REAL generated server.
import assert from 'node:assert/strict';
import { act, create } from 'react-test-renderer';
import type { ReactTestRenderer } from 'react-test-renderer';
import { LiveClient } from './client_react/live/client';
import { QuotaExceeded } from './client_react/live/errors/live';
import { VeldProvider, useChatStream, useForeverStream, useTicksStream } from './client_react/live/react';
import type { StreamResult } from './client_react/runtime/react';
import { seen, start } from './stream_server';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

async function until(label: string, cond: () => boolean): Promise<void> {
  for (let i = 0; i < 300; i += 1) {
    if (cond()) return;
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });
  }
  assert.fail(`timed out waiting for: ${label}`);
}

type Seen<T> = { current: T | undefined };
const peek = <T,>(s: Seen<T>): T | undefined => s.current;

function render(client: LiveClient, element: JSX.Element): ReactTestRenderer {
  let tree: ReactTestRenderer | undefined;
  act(() => {
    tree = create(<VeldProvider clients={{ live: client }}>{element}</VeldProvider>);
  });
  assert.ok(tree);
  return tree;
}

async function main(): Promise<void> {
  const server = await start();
  const client = new LiveClient({ baseUrl: server.url });
  try {
    // Items accumulate in order and `streaming` turns false when the stream ends.
    const ticks: Seen<StreamResult<{ n: number }>> = { current: undefined };
    function Ticks(props: { count: number; failAt?: number }): JSX.Element {
      const r = useTicksStream(props.failAt === undefined ? { count: props.count } : { count: props.count, failAt: props.failAt });
      ticks.current = r;
      return <span>{r.items.map((t) => t.n).join(',')}</span>;
    }
    const tree = render(client, <Ticks count={4} />);
    await until('four ticks', () => peek(ticks)?.items.length === 4 && peek(ticks)?.streaming === false);
    assert.deepEqual(peek(ticks)?.items.map((t) => t.n), [0, 1, 2, 3]);
    assert.equal(peek(ticks)?.latest?.n, 3);
    assert.equal(peek(ticks)?.error, undefined);

    // A new request (different canonical bytes) starts over, and does not show stale items.
    act(() => tree.update(<VeldProvider clients={{ live: client }}><Ticks count={2} /></VeldProvider>));
    await until('two ticks', () => peek(ticks)?.streaming === false && peek(ticks)?.items.length === 2);
    assert.deepEqual(peek(ticks)?.items.map((t) => t.n), [0, 1]);

    // A typed error after the first events lands in `error`; the items so far stay.
    act(() => tree.update(<VeldProvider clients={{ live: client }}><Ticks count={9} failAt={2} /></VeldProvider>));
    await until('the stream failed', () => peek(ticks)?.error !== undefined);
    assert.ok(QuotaExceeded.matches(peek(ticks)?.error), 'typed error from the hook');
    assert.deepEqual(peek(ticks)?.items.map((t) => t.n), [0, 1]);
    assert.equal(peek(ticks)?.streaming, false);
    act(() => tree.unmount());

    // An endless stream is aborted on unmount: the server notices.
    const forever: Seen<StreamResult<{ n: number }>> = { current: undefined };
    function Forever(): JSX.Element {
      const r = useForeverStream({}, { max: 2 });
      forever.current = r;
      return <span>{String(r.latest?.n ?? '')}</span>;
    }
    const before = seen.foreverClosed;
    const endless = render(client, <Forever />);
    await until('five ticks seen', () => (peek(forever)?.latest?.n ?? 0) >= 4);
    assert.ok((peek(forever)?.items.length ?? 0) <= 2, 'max keeps the newest items only');
    assert.equal(peek(forever)?.streaming, true);
    act(() => endless.unmount());
    await until('the server cleans up after unmount', () => seen.foreverClosed > before);

    // `enabled: false` (and a null request) hold the stream back.
    function Held(): JSX.Element {
      const r = useTicksStream({ count: 3 }, { enabled: false });
      ticks.current = r;
      return <span>{String(r.streaming)}</span>;
    }
    const held = render(client, <Held />);
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });
    assert.equal(peek(ticks)?.streaming, false);
    assert.deepEqual(peek(ticks)?.items, []);
    act(() => held.unmount());

    // A WebSocket channel through the same hook: `send` plus the typed replies.
    const chat: Seen<ReturnType<typeof useChatStream>> = { current: undefined };
    function Chat(): JSX.Element {
      const r = useChatStream({ room: 'hooks', nick: 'rex' });
      chat.current = r;
      return <span>{r.items.length}</span>;
    }
    const room = render(client, <Chat />);
    await act(async () => {
      peek(chat)?.send({ text: 'hello' });
    });
    await until('the echo', () => (peek(chat)?.items.length ?? 0) === 1);
    assert.deepEqual(peek(chat)?.items[0], { from: 'rex', text: 'hello', seq: 0 });
    const ended = seen.chatEnded;
    act(() => room.unmount());
    await until('the server sees the chat close', () => seen.chatEnded > ended);
  } finally {
    await server.close();
  }
  console.log('react stream hooks ok');
}

main().then(
  () => process.exit(0),
  (err: unknown) => {
    console.error(err);
    process.exit(1);
  },
);
