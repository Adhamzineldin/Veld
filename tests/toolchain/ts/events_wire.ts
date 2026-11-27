// The wire goldens of events (tests/wire/events) run through the GENERATED publisher and
// subscriber of the Node server. The Rust test (`crates/emitters/node/tests/events.rs`) writes
// `cases.json` from `envelopes.json` and `consume.json`, so a new case needs no code here.
//
//  - `publish`: the typed value is published through a transport that keeps the bytes, with the id
//    source and the clock fixed; the bytes are printed as hex.
//  - `consume`: a message is handed to the generated subscriber of a service that handles the
//    listed events; what its handlers, hooks and the transport saw is printed.
//
// Prints one JSON document on stdout.
import { readFileSync } from 'fs';

import { MemoryTransport } from './server/runtime/events';
import type { EventMeta, Transport } from './server/runtime/events';

interface PublishCase {
  name: string;
  module: string;
  factory: string;
  channel: string;
  method: string;
  companion: string;
  id: string;
  time: string;
  value: unknown;
}

interface ConsumeCase {
  name: string;
  module: string;
  subscribe: string;
  topic: string;
  key: string | null;
  message: string;
  handlers: { method: string; event: string }[];
}

interface Companion {
  fromWire(wire: unknown): unknown;
  toWire(value: unknown): unknown;
}

// eslint-disable-next-line @typescript-eslint/no-require-imports
const pkg = require('./server/index') as Record<string, Companion>;
const load = (path: string): Record<string, (...args: unknown[]) => unknown> =>
  // eslint-disable-next-line @typescript-eslint/no-require-imports
  require(`./${path}`) as Record<string, (...args: unknown[]) => unknown>;

const hex = (bytes: Uint8Array): string => Buffer.from(bytes).toString('hex');

async function publish(c: PublishCase): Promise<unknown> {
  const sent: { topic: string; key: string | null; hex: string }[] = [];
  const recording: Transport = {
    publish: (topic, key, bytes) => {
      sent.push({ topic, key, hex: hex(bytes) });
      return Promise.resolve();
    },
    subscribe: () => Promise.resolve({ close: () => Promise.resolve() }),
  };
  const make = load(c.module)[c.factory] as (t: Transport, o: unknown) => Record<string, Record<string, (e: unknown) => Promise<unknown>>>;
  const publisher = make(recording, { newId: () => c.id, clock: () => new Date(c.time) });
  const channel = publisher[c.channel];
  const method = channel?.[c.method];
  if (method === undefined) throw new Error(`${c.name}: no publisher method ${c.channel}.${c.method}`);
  const typed = (pkg[c.companion] as Companion).fromWire(c.value);
  const receipt = await method(typed);
  return { name: c.name, sent, receipt };
}

async function consume(c: ConsumeCase): Promise<unknown> {
  const transport = new MemoryTransport();
  const delivered: { event: string; data: unknown; meta: EventMeta }[] = [];
  const rejected: { reason: string; event: string | null; violations: number; hex: string }[] = [];
  const mismatches: unknown[] = [];
  const rules: unknown[] = [];
  const handlers: Record<string, (event: unknown, meta: EventMeta) => void> = {};
  for (const h of c.handlers) {
    handlers[h.method] = (event, meta) => {
      delivered.push({ event: h.event, data: (pkg[h.event] as Companion).toWire(event), meta });
    };
  }
  const subscribe = load(c.module)[c.subscribe] as (t: Transport, h: unknown, o: unknown) => Promise<{ close(): Promise<void> }>;
  const handle = await subscribe(transport, handlers, {
    onEventRejected: (e: { reason: string; event: string | null; violations: unknown[]; bytes: Uint8Array }) =>
      rejected.push({ reason: e.reason, event: e.event, violations: e.violations.length, hex: hex(e.bytes) }),
    onContractMismatch: (e: unknown) => mismatches.push(e),
    onRuleViolation: (e: unknown) => rules.push(e),
  });
  await transport.publish(c.topic, c.key, new TextEncoder().encode(c.message));
  await transport.idle();
  await handle.close();
  return { name: c.name, delivered, rejected, mismatches, rules, dead: transport.deadLetters().length };
}

async function main(): Promise<void> {
  const cases = JSON.parse(readFileSync('cases.json', 'utf8')) as { publish: PublishCase[]; consume: ConsumeCase[] };
  const publishes: unknown[] = [];
  for (const c of cases.publish) publishes.push(await publish(c));
  const consumes: unknown[] = [];
  for (const c of cases.consume) consumes.push(await consume(c));
  process.stdout.write(JSON.stringify({ publish: publishes, consume: consumes }));
}

main().catch((e: unknown) => {
  process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  process.exit(1);
});
