// A flagged invariant of an event (spec/platform.md section 11): the publisher and the consumer
// evaluate it when the provider of their options says the flag is on, with a context that has no
// principal. Expects `./server` generated from tests/toolchain/contract/platform_p2/flags_events.veld.
// Prints `flags events ok`.
import { strict as assert } from 'assert';

import { createDeskPublisher } from './server/server/desk-events';
import { subscribeFloorEvents } from './server/server/floor-events';
import { MemoryTransport } from './server/runtime/events';
import type { EventRejected } from './server/runtime/events';
import type { FlagContext, FlagProvider } from './server/runtime/veld';

const ID = '5d2b3a0e-1c11-4a52-8d0f-0a4f1b7c9e21';

class Provider implements FlagProvider {
  readonly asked: { name: string; context: FlagContext }[] = [];
  failing = false;
  constructor(readonly on: string[]) {}
  isOn(name: string, context: FlagContext): boolean {
    this.asked.push({ name, context });
    if (this.failing) throw new Error('the provider failed');
    return this.on.includes(name);
  }
}

async function main(): Promise<void> {
  // The publisher: without a provider the flag is off.
  const sent = new MemoryTransport();
  const heard: number[] = [];
  const rejected: EventRejected[] = [];
  await subscribeFloorEvents(sent, { seat: (e) => void heard.push(e.guests) }, { onEventRejected: (e) => rejected.push(e) });
  const plain = createDeskPublisher(sent);
  await plain.bookings.booked({ id: ID, guests: 9 });
  await sent.idle();
  assert.deepEqual(heard, [9], 'a flag that is off is not evaluated');

  // With the flag on, the publisher refuses the event and nothing is sent.
  const on = new Provider(['smallGroups']);
  const strict = createDeskPublisher(sent, { flags: on });
  const refused = await strict.bookings.booked({ id: ID, guests: 9 }).then(
    () => null,
    (e: unknown) => e as Error & { title: string },
  );
  assert.ok(refused !== null);
  assert.equal(refused.title, 'ValidationError');
  await sent.idle();
  assert.deepEqual(heard, [9], 'nothing was sent');
  // The provider is asked once per event, with no principal, and a value that holds goes through.
  assert.equal(on.asked.length, 1);
  assert.equal(on.asked[0]?.name, 'smallGroups');
  assert.equal(on.asked[0]?.context.principal, undefined);
  await strict.bookings.booked({ id: ID, guests: 3 });
  await sent.idle();
  assert.deepEqual(heard, [9, 3]);
  assert.equal(on.asked.length, 2);

  // A failing provider counts an invariant as on, and the failure is heard.
  const failing = new Provider([]);
  failing.failing = true;
  const errors: unknown[] = [];
  const broken = createDeskPublisher(sent, { flags: failing, onFlagError: (e) => errors.push(e) });
  await assert.rejects(broken.bookings.booked({ id: ID, guests: 9 }));
  assert.equal(errors.length, 1);

  // The consumer: a message that breaks the invariant is rejected when its provider says the flag is on.
  const strictHeard: number[] = [];
  const receiving = new MemoryTransport();
  const provider = new Provider(['smallGroups']);
  await subscribeFloorEvents(receiving, { seat: (e) => void strictHeard.push(e.guests) }, {
    flags: provider,
    onEventRejected: (e) => rejected.push(e),
  });
  await createDeskPublisher(receiving).bookings.booked({ id: ID, guests: 9 });
  await receiving.idle();
  assert.deepEqual(strictHeard, []);
  assert.equal(rejected.length, 1);
  assert.equal(rejected[0]?.reason, 'rule');
  assert.equal(provider.asked.length, 1);
  assert.equal(provider.asked[0]?.context.principal, undefined);
  await createDeskPublisher(receiving).bookings.booked({ id: ID, guests: 4 });
  await receiving.idle();
  assert.deepEqual(strictHeard, [4]);
  console.log('flags events ok');
}

main().catch((err: unknown) => {
  console.error(err);
  process.exit(1);
});
