// The `entries` of tests/wire/events/outbox.json through the generated `emit` (spec/events.md
// section 11.1): per case the handler of a probe endpoint calls `emit.<event>(value)` with the id
// source and the clock of the hooks fixed, and the outbox that records what it is given must have
// exactly the topic, the key and the bytes of the golden. There is no transport to publish to.
// `cases.json` is written by crates/emitters/node/tests/events.rs: the service, the path prefix and
// the cases of one channel. Expects `./server` generated from the contract with the probe moves
// added. Prints `events outbox wire ok`.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import * as srv from './server/index';

interface Case {
  name: string;
  event: string;
  method: string;
  id: string;
  time: string;
  value: unknown;
  topic: string;
  key: string | null;
  canonical: string;
}

interface Group {
  service: string;
  at: string;
  cases: Case[];
}

interface Companion {
  fromWire(wire: unknown): unknown;
}

const group = JSON.parse(readFileSync('cases.json', 'utf8')) as Group;
const pkg = srv as unknown as Record<string, unknown>;
const PROBE = '00000000-0000-4000-8000-0000000000aa';

interface Saved {
  topic: string;
  key: string | null;
  bytes: Uint8Array;
}

async function run(c: Case): Promise<void> {
  const saved: Saved[] = [];
  const outbox: srv.veldEvents.Outbox = {
    save: (topic, key, bytes) => void saved.push({ topic, key, bytes }),
    pending: () => [],
    done: () => undefined,
  };
  const companion = pkg[c.event] as Companion;
  // The probe endpoint `zz<Event>` writes the event of the case and answers its move.
  const impl = new Proxy(
    {},
    {
      get: (_target, name: string) => async (req: { id: string; emit: Record<string, (event: unknown) => Promise<unknown>> }) => {
        if (name !== `zz${c.event}`) return undefined;
        await req.emit[c.method]?.(companion.fromWire(c.value));
        return { id: req.id, state: 'b' };
      },
    },
  );
  const facts = new Proxy({}, { get: () => async (key: string) => ({ id: key, state: 'a' }) });
  const create = pkg[`create${group.service}Handler`] as (impl: unknown, hooks: unknown) => srv.veld.VeldHandler;
  const handler = create(impl, {
    facts,
    outbox,
    events: { newId: () => c.id, clock: () => new Date(c.time) },
    onError: (e: unknown) => {
      throw e;
    },
  });
  const res = await handler({ method: 'POST', path: `${group.at}/zz/${PROBE}/${c.event}`, headers: {} });
  assert.ok(res !== null && res.status < 300, `${c.name}: ${String(res?.status)} ${res?.body ?? ''}`);
  assert.equal(saved.length, 1, `${c.name}: one entry`);
  const entry = saved[0];
  assert.equal(entry?.topic, c.topic, `${c.name}: topic`);
  assert.equal(entry?.key, c.key, `${c.name}: key`);
  assert.equal(new TextDecoder().decode(entry?.bytes ?? new Uint8Array()), c.canonical, `${c.name}: bytes`);
}

async function main(): Promise<void> {
  assert.ok(group.cases.length > 0);
  for (const c of group.cases) await run(c);
  console.log('events outbox wire ok');
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
