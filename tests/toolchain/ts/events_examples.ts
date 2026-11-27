// The event examples of a contract (`given event ... expect S.handler called`) against the
// GENERATED publisher and subscribers of the Node server, over the memory transport.
// `crates/emitters/node/tests/events.rs` writes `cases.json`: the subscribers, the filled-in
// events as wire values and the same events as envelope bytes. Per example this prints:
//
//  - `raw`: the handlers called when the message is put on the transport as it is (a rejected
//    event reaches the consumer, which is what `expect rejected` means);
//  - `viaPublisher`: the handlers called when the typed event goes through the generated
//    publisher, or the title of the error the publisher threw (nothing is sent then).
import { readFileSync } from 'fs';

import { MemoryTransport } from './server/runtime/events';

interface Service {
  name: string;
  module: string;
  subscribe: string;
  handlers: string[];
}

interface Example {
  name: string;
  topic: string;
  key: string | null;
  message: string;
  publisher: { module: string; factory: string; channel: string; method: string; companion: string };
  value: unknown;
}

interface Companion {
  fromWire(wire: unknown): unknown;
}

const load = (path: string): Record<string, (...args: unknown[]) => unknown> =>
  // eslint-disable-next-line @typescript-eslint/no-require-imports
  require(`./${path}`) as Record<string, (...args: unknown[]) => unknown>;
// eslint-disable-next-line @typescript-eslint/no-require-imports
const pkg = require('./server/index') as Record<string, Companion>;

async function subscribeAll(transport: MemoryTransport, services: Service[], called: string[]): Promise<{ close(): Promise<void> }[]> {
  const handles: { close(): Promise<void> }[] = [];
  for (const s of services) {
    const handlers: Record<string, () => void> = {};
    for (const h of s.handlers) {
      handlers[h] = () => {
        called.push(`${s.name}.${h}`);
      };
    }
    const subscribe = load(s.module)[s.subscribe] as (t: unknown, h: unknown, o: unknown) => Promise<{ close(): Promise<void> }>;
    handles.push(await subscribe(transport, handlers, {}));
  }
  return handles;
}

async function raw(services: Service[], ex: Example): Promise<string[]> {
  const transport = new MemoryTransport();
  const called: string[] = [];
  const handles = await subscribeAll(transport, services, called);
  await transport.publish(ex.topic, ex.key, new TextEncoder().encode(ex.message));
  await transport.idle();
  for (const h of handles) await h.close();
  return called.sort();
}

async function viaPublisher(services: Service[], ex: Example): Promise<{ refused: string | null; called: string[] }> {
  const transport = new MemoryTransport();
  const called: string[] = [];
  const handles = await subscribeAll(transport, services, called);
  const p = ex.publisher;
  const make = load(p.module)[p.factory] as (t: unknown, o: unknown) => Record<string, Record<string, (e: unknown) => Promise<unknown>>>;
  const method = make(transport, {})[p.channel]?.[p.method];
  if (method === undefined) throw new Error(`${ex.name}: no publisher method ${p.channel}.${p.method}`);
  let refused: string | null = null;
  try {
    await method((pkg[p.companion] as Companion).fromWire(ex.value));
  } catch (e) {
    refused = e instanceof Error ? e.name : String(e);
  }
  await transport.idle();
  for (const h of handles) await h.close();
  return { refused, called: called.sort() };
}

async function main(): Promise<void> {
  const cases = JSON.parse(readFileSync('cases.json', 'utf8')) as { services: Service[]; examples: Example[] };
  const out: unknown[] = [];
  for (const ex of cases.examples) {
    out.push({ name: ex.name, raw: await raw(cases.services, ex), viaPublisher: await viaPublisher(cases.services, ex) });
  }
  process.stdout.write(JSON.stringify(out));
}

main().catch((e: unknown) => {
  process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  process.exit(1);
});
