// `publish` on RabbitMQ waits a bounded time for the broker's confirm (spec/events.md section
// 10.7): `publishTimeoutMs` (30000 by default) in the adapter's options. No broker is needed: the
// connection is a stand-in whose confirm channel answers, or never does. Prints
// `events rabbitmq timeout ok`.
import { strict as assert } from 'assert';

import { RabbitMqTransport } from './server/server/rabbitmq';
import type { AmqpChannel, AmqpConnection } from './server/server/rabbitmq';

type Answer = 'confirm' | 'never' | 'nack';

function channel(answer: Answer): AmqpChannel {
  return {
    assertExchange: async () => undefined,
    assertQueue: async () => undefined,
    bindQueue: async () => undefined,
    prefetch: async () => undefined,
    consume: async () => ({ consumerTag: 'tag' }),
    cancel: async () => undefined,
    ack: () => undefined,
    publish: (_exchange, _key, _content, _options, callback) => {
      if (answer === 'confirm') callback?.(null);
      else if (answer === 'nack') callback?.(new Error('nacked'));
      return true;
    },
    close: async () => undefined,
    on: () => undefined,
  };
}

function connection(answer: Answer): AmqpConnection {
  return { createChannel: async () => channel('confirm'), createConfirmChannel: async () => channel(answer) };
}

const bytes = new TextEncoder().encode('{}');

async function main(): Promise<void> {
  // A confirmed message completes, and a nack fails at once.
  await new RabbitMqTransport(connection('confirm'), { publishTimeoutMs: 50 }).publish('t', null, bytes);
  await assert.rejects(new RabbitMqTransport(connection('nack'), { publishTimeoutMs: 5000 }).publish('t', 'k', bytes), /nacked/);

  // A message the broker never confirms fails after `publishTimeoutMs`, and not much earlier.
  const started = Date.now();
  await assert.rejects(
    new RabbitMqTransport(connection('never'), { publishTimeoutMs: 100 }).publish('t', null, bytes),
    /did not confirm the message to `t` within 100 ms/,
  );
  const waited = Date.now() - started;
  assert.ok(waited >= 90 && waited < 5000, `waited ${String(waited)} ms`);

  console.log('events rabbitmq timeout ok');
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
