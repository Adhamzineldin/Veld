// Events in a contract with no rule at all: the generated package keeps neither `runtime/rules.ts`
// nor the rules regions of the runtime, and still publishes, consumes and refuses a value of the
// wrong shape. Prints `events plain ok`.
import { strict as assert } from 'assert';

import { createPingerPublisher } from './server/server/pinger-events';
import { subscribeListenerEvents } from './server/server/listener-events';
import { MemoryTransport } from './server/runtime/events';

async function main(): Promise<void> {
  const transport = new MemoryTransport();
  const heard: string[] = [];
  await subscribeListenerEvents(transport, { heard: (e) => void heard.push(e.when) });
  const pinger = createPingerPublisher(transport);
  await pinger.pings.pinged({ when: '2026-06-10T13:59:58.250+02:00' });
  await transport.idle();
  assert.deepEqual(heard, ['2026-06-10T11:59:58.250Z'], 'the datetime is written in UTC');
  const err = await pinger.pings.pinged({ when: 'yesterday' }).then(
    () => null,
    (e: unknown) => e as Error & { title: string; data: { issues: { path: string; code: string }[] } },
  );
  assert.ok(err !== null);
  assert.equal(err.title, 'ValidationError');
  assert.equal(err.data.issues[0]?.path, '$.when');
  await transport.idle();
  assert.equal(heard.length, 1, 'nothing was sent for the invalid value');
  process.stdout.write('events plain ok\n');
}

main().catch((e: unknown) => {
  process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  process.exit(1);
});
