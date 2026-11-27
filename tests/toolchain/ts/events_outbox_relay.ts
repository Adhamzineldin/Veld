// The `relay` and `written` goldens of tests/wire/events/outbox.json (copied next to this file as
// `outbox.json`) through the GENERATED relay `runBookingsOutboxRelay` and the generated check
// `veld.checkWritten` (spec/events.md section 11, tests/wire/events/README.md). Expects `./server`
// generated from tests/events/outbox.veld.pending (compiled as the file `t.veld` of the package
// `bookings`). Prints `events outbox relay ok`.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as srv from './server/index';

type Step = 'accepted' | 'crashAfterPublish' | { publishFails: string } | { doneFails: string };

interface RelayCase {
  name: string;
  saves: number;
  runs: {
    limit: number;
    script: Step[];
    expect: {
      published: string[];
      done: string[];
      sent: number;
      failure: { entry: string; error: string } | null;
    };
    pendingAfter: string[];
  }[];
}

interface WrittenCase {
  name: string;
  expected: string[];
  written: string[];
  result: string;
}

const golden = JSON.parse(readFileSync('outbox.json', 'utf8')) as { relay: RelayCase[]; written: WrittenCase[] };

const TOPIC = 'shop.bookings.v1';
const encode = (text: string): Uint8Array => new TextEncoder().encode(text);
const decode = (bytes: Uint8Array): string => new TextDecoder().decode(bytes);

/** What the process that "ends" between the publish and `done` throws: the relay reports it, the run does not. */
const CRASH = new Error('the process ended');

async function runCase(c: RelayCase): Promise<void> {
  const memory = new srv.veldEvents.MemoryOutbox();
  // Entry `n` is saved n-th: the memory outbox names it "n", and its bytes say so.
  for (let i = 1; i <= c.saves; i += 1) memory.save(TOPIC, null, encode(String(i)));
  for (const [n, run] of c.runs.entries()) {
    const what = `${c.name} / run ${String(n + 1)}`;
    const published: string[] = [];
    const done: string[] = [];
    let seen = -1;
    const step = (): Step => run.script[seen] ?? 'accepted';
    const transport: srv.veldEvents.Transport = {
      publish: (_topic, _key, bytes) => {
        seen += 1;
        const s = step();
        if (typeof s === 'object' && 'publishFails' in s) return Promise.reject(new Error(s.publishFails));
        published.push(decode(bytes));
        return Promise.resolve();
      },
      subscribe: () => Promise.resolve({ close: () => Promise.resolve() }),
    };
    const outbox: srv.veldEvents.Outbox = {
      save: (topic, key, bytes) => memory.save(topic, key, bytes),
      pending: (limit) => memory.pending(limit),
      done: (entry) => {
        const s = step();
        if (s === 'crashAfterPublish') throw CRASH;
        if (typeof s === 'object' && 'doneFails' in s) throw new Error(s.doneFails);
        done.push(entry.id);
        memory.done(entry);
      },
    };
    let failure: { entry: string; error: string } | null = null;
    const sent = await srv.runBookingsOutboxRelay(outbox, transport, {
      limit: run.limit,
      onError: (error, entry) => {
        // A crash reports nothing: the process is gone.
        if (error !== CRASH) failure = { entry: entry.id, error: (error as Error).message };
      },
    });
    assert.deepEqual(published, run.expect.published, `${what}: published`);
    assert.deepEqual(done, run.expect.done, `${what}: done`);
    assert.equal(sent, run.expect.sent, `${what}: sent`);
    assert.deepEqual(failure, run.expect.failure, `${what}: failure`);
    assert.deepEqual(
      memory.pending(1000).map((e) => e.id),
      run.pendingAfter,
      `${what}: pending after`,
    );
  }
}

async function main(): Promise<void> {
  assert.ok(golden.relay.length > 0 && golden.written.length > 0);
  for (const c of golden.relay) await runCase(c);
  // `written`: what the server checks after the handler of a move that emits.
  for (const c of golden.written) {
    assert.equal(srv.veld.checkWritten(c.expected, c.written), c.result === 'ok' ? 'ok' : c.result, c.name);
  }
  // The relay hands the transport the options of the channel of the topic (an unknown topic: none set).
  const seen: unknown[] = [];
  const outbox = new srv.veldEvents.MemoryOutbox();
  outbox.save(TOPIC, 'k', encode('x'));
  await srv.runBookingsOutboxRelay(outbox, {
    publish: (_t, _k, _b, options) => {
      seen.push(options);
      return Promise.resolve();
    },
    subscribe: () => Promise.resolve({ close: () => Promise.resolve() }),
  });
  assert.deepEqual(seen, [{ ordered: false, deadLetter: false }]);
  console.log('events outbox relay ok');
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
