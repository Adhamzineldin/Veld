// A service with an endpoint that has parameters has the prelude `ValidationError`: the publisher
// of the same service throws that class, with the violations and `local`. Prints `events prelude ok`.
import { strict as assert } from 'assert';

import { ValidationError } from './server/prelude';
import { createHelloPublisher } from './server/server/hello-events';
import { MemoryTransport } from './server/runtime/events';

async function main(): Promise<void> {
  const transport = new MemoryTransport();
  const hello = createHelloPublisher(transport);
  const err = await hello.greetingEvents.greeted({ name: '' }).then(
    () => null,
    (e: unknown) => e,
  );
  assert.ok(err instanceof ValidationError);
  assert.equal(err.local, true);
  assert.equal(err.violations.length, 1);
  assert.equal(err.data.issues[0]?.code, 'rule');
  assert.equal(err.status, 422);
  process.stdout.write('events prelude ok\n');
}

main().catch((e: unknown) => {
  process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  process.exit(1);
});
