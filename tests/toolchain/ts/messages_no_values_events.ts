// Platform phase P4, part 1 (spec/rules.md 19.3): no message of the events runtime repeats a value.
// The marker goes through every path of `runtime/events.ts` that writes a message: a publisher that
// refuses a value of the wrong shape and one that violates an invariant, and a consumer that is
// given a malformed envelope, a message of the wrong shape and one that violates a rule. Every
// message, `message` of a rejection and issue and violation text is searched for the marker.
// Expects `./server` generated from tests/toolchain/contract/platform_p4/no_values_events.veld.
// Prints `no values events ok`.
import { strict as assert } from 'assert';

import { createWriterPublisher } from './server/server/writer-events';
import { subscribeReaderEvents } from './server/server/reader-events';
import { MemoryTransport } from './server/runtime/events';
import type { EventRejected, EventRuleViolation } from './server/runtime/events';

const MARKER = 'MARKER-3f9a71c2e5';
const ID = '5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21';

/** Every text the runtime wrote, labelled by where it came from. */
const texts: { where: string; text: string }[] = [];
const note = (where: string, text: unknown): void => {
  if (typeof text === 'string') texts.push({ where, text });
};

function collect(where: string, error: unknown): void {
  const e = error as { message?: unknown; title?: unknown; data?: { issues?: { message: string }[] }; violations?: { message: string }[] };
  note(`${where} message`, e.message);
  note(`${where} title`, e.title);
  for (const i of e.data?.issues ?? []) note(`${where} issue`, i.message);
  for (const v of e.violations ?? []) note(`${where} violation`, v.message);
}

async function main(): Promise<void> {
  const memory = new MemoryTransport();
  const wire: Uint8Array[] = [];
  const transport: typeof memory = Object.assign(Object.create(memory) as typeof memory, {
    publish: (topic: string, key: string | null, bytes: Uint8Array) => {
      wire.push(bytes);
      return memory.publish(topic, key, bytes);
    },
  });
  const rejected: EventRejected[] = [];
  const ruleEvents: EventRuleViolation[] = [];
  await subscribeReaderEvents(
    memory,
    { noted: () => undefined },
    { onEventRejected: (e) => void rejected.push(e), onRuleViolation: (e) => void ruleEvents.push(e) },
  );
  const writer = createWriterPublisher(transport, { onRuleViolation: (e) => void ruleEvents.push(e) });

  // A good message, to build the others from.
  await writer.notes.noted({ id: ID, text: 'abcd', count: 2 });
  await memory.idle();
  const base = JSON.parse(new TextDecoder().decode(wire[0])) as Record<string, unknown>;

  // The publisher: the marker where a uuid, a string and a number go, and a violated invariant.
  const attempts: [string, unknown][] = [
    ['an id that is not a uuid', { id: MARKER, text: 'abcd', count: 2 }],
    ['a text of the wrong length', { id: ID, text: MARKER, count: 2 }],
    ['a count that is not a number', { id: ID, text: 'abcd', count: MARKER }],
    ['an odd count', { id: ID, text: 'abcd', count: 3 }],
    ['a value that is not an object', MARKER],
  ];
  for (const [where, value] of attempts) {
    const error = await writer.notes.noted(value as never).then(
      () => null,
      (e: unknown) => e,
    );
    assert.ok(error !== null, `publish: ${where} is refused`);
    collect(`publish ${where}`, error);
  }

  // The consumer: a malformed message, an envelope with another content type, one of the wrong shape
  // and one that violates a rule.
  const send = async (json: unknown): Promise<void> => {
    const bytes = typeof json === 'string' ? new TextEncoder().encode(json) : new TextEncoder().encode(JSON.stringify(json));
    await memory.publish('notes.v1', ID, bytes);
    await memory.idle();
  };
  await send(MARKER);
  await send(JSON.stringify(MARKER));
  await send({ ...base, specversion: MARKER });
  await send({ ...base, id: 7, source: MARKER });
  await send({ ...base, subject: 7 });
  await send({ ...base, datacontenttype: MARKER });
  await send({ ...base, data: MARKER });
  await send({ ...base, data: { id: MARKER, text: MARKER, count: MARKER } });
  await send({ ...base, data: { id: ID, text: 'abcd', count: 3 } });
  await send({ ...base, type: MARKER });
  assert.ok(rejected.length >= 7, `every message was rejected (${String(rejected.length)})`);
  for (const r of rejected) {
    note(`rejected ${r.reason} message`, r.message);
    for (const i of r.issues) note('rejected issue', i.message);
    for (const v of r.violations) note('rejected violation', v.message);
  }
  for (const e of ruleEvents) note('rule violation event', e.violation.message);

  assert.ok(texts.length > 15, `texts were collected (${String(texts.length)})`);
  for (const { where, text } of texts) {
    assert.equal(text.includes(MARKER), false, `${where} repeats the value: ${text}`);
  }
  process.stdout.write('no values events ok\n');
}

main().catch((e: unknown) => {
  process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  process.exit(1);
});
