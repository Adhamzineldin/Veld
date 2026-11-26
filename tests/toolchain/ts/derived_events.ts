// Derived fields of an event (spec/rules.md section 17.2, spec/events.md section 6), two generated
// services in one process over the memory transport: the publisher computes them before the
// invariants, a derived field that fails stops the publish, and the subscriber computes them again
// from the data of the message instead of believing the one it carries. Expects `./server`
// generated from tests/toolchain/contract/derived/events.veld. Prints `derived events ok`.
import { strict as assert } from 'assert';

import type { InvoicedOutput } from './server/events/bookings';
import { createBillerPublisher } from './server/server/biller-events';
import { subscribeLedgerEvents } from './server/server/ledger-events';
import { MemoryTransport } from './server/runtime/events';
import type { EventRuleViolation } from './server/runtime/events';

const ID = '5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21';

/** The message bytes a transport carries, as text. */
function textOf(bytes: Uint8Array): string {
  return new TextDecoder().decode(bytes);
}

async function main(): Promise<void> {
  // A transport that remembers what was published, in front of the memory one.
  const memory = new MemoryTransport();
  const wire: string[] = [];
  const transport: typeof memory = Object.assign(Object.create(memory) as typeof memory, {
    publish: (topic: string, key: string | null, bytes: Uint8Array) => {
      wire.push(textOf(bytes));
      return memory.publish(topic, key, bytes);
    },
  });
  const heard: { gross: number; net: number }[] = [];
  await subscribeLedgerEvents(memory, { record: (e) => void heard.push({ gross: e.gross, net: e.net }) });
  const violations: EventRuleViolation[] = [];
  const biller = createBillerPublisher(transport, { onRuleViolation: (e) => violations.push(e) });

  // The publisher computes `gross`, and replaces what the application put there.
  const given: InvoicedOutput = { id: ID, net: 10, tax: 2, gross: 999 };
  const sent = await biller.invoices.invoiced(given);
  await memory.idle();
  const firstEnvelope = JSON.parse(wire[0] ?? '{}') as { data: { gross: number } };
  const data = firstEnvelope.data;
  assert.equal(data.gross, 12, 'the message carries the computed value');
  assert.deepEqual(heard, [{ gross: 12, net: 10 }]);
  assert.equal(given.gross, 999, 'the application value is not changed');

  // The value of the application is optional: nothing has to be given.
  await biller.invoices.invoiced({ id: ID, net: 1, tax: 1 });
  await memory.idle();
  assert.equal(heard[1]?.gross, 2);

  // An invariant reads the computed value.
  const refused = await biller.invoices.invoiced({ id: ID, net: -5, tax: 1 }).then(
    () => null,
    (e: unknown) => e as Error & { title: string },
  );
  assert.ok(refused !== null);
  assert.equal(refused.title, 'ValidationError');
  assert.equal(heard.length, 2, 'nothing was sent');
  assert.equal(violations.length > 0, true);

  // A derived field that fails stops the publish like a violated invariant.
  wire.length = 0;
  const overflow = await biller.invoices.invoiced({ id: ID, net: 2147483647, tax: 1 }).then(
    () => null,
    (e: unknown) => e as Error & { title: string },
  );
  assert.ok(overflow !== null);
  assert.equal(overflow.title, 'ValidationError');
  assert.equal(wire.length, 0, 'nothing was published');
  assert.equal(heard.length, 2);

  // The subscriber does not believe the message: it computes `gross` again, and a malformed value
  // there is not a shape error.
  // Built from a real message, so the rules digest matches and the invariants run too.
  const envelope = (gross: unknown): Uint8Array =>
    new TextEncoder().encode(
      JSON.stringify({
        ...firstEnvelope,
        id: `raw-${String(heard.length)}`,
        data: { id: ID, net: 30, tax: 3, gross },
      }),
    );
  await memory.publish('shop.invoices.v1', ID, envelope(1));
  await memory.idle();
  assert.equal(heard[2]?.gross, 33, 'the value in the message is replaced');
  await memory.publish('shop.invoices.v1', ID, envelope('not a number'));
  await memory.idle();
  assert.equal(heard[3]?.gross, 33, 'a malformed derived value is dropped, not rejected');
  process.stdout.write('derived events ok\n');
}

main().catch((e: unknown) => {
  process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  process.exit(1);
});
