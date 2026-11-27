// An audited invariant of an event (`@rollout(audit)`, tests/wire/events/edges.veld, event
// `InvoiceNudged`): it is evaluated and reported to `onRuleViolation` and never rejects, on the
// way out and on the way in. Prints one JSON document: what the hooks heard and what was
// delivered, for a value that breaks the rule and one that does not.
import { createBillingPublisher } from './server/server/billing-events';
import { subscribeProbe0Events } from './server/server/probe0-events';
import { MemoryTransport } from './server/runtime/events';

const INVOICE = '7e6f0a52-3d84-4a4b-9c27-0f8e2b1d6a90';

interface Heard {
  rule: string;
  operation: string;
  position: string;
  outcome: string;
}

async function run(days: number): Promise<unknown> {
  const transport = new MemoryTransport();
  const published: Heard[] = [];
  const consumed: Heard[] = [];
  const delivered: number[] = [];
  const rejected: string[] = [];
  await subscribeProbe0Events(
    transport,
    { onInvoiceNudged: (e) => void delivered.push(e.days) },
    {
      onRuleViolation: (e) => consumed.push(e),
      onEventRejected: (e) => rejected.push(e.reason),
    },
  );
  const sent: string[] = [];
  const spy = {
    publish: (topic: string, key: string | null, bytes: Uint8Array) => {
      sent.push(new TextDecoder().decode(bytes));
      return transport.publish(topic, key, bytes);
    },
    subscribe: transport.subscribe.bind(transport),
  };
  const billing = createBillingPublisher(spy, { onRuleViolation: (e) => published.push(e) });
  await billing.invoiceEvents.invoiceNudged({ invoiceId: INVOICE, days });
  await transport.idle();
  return { days, published, consumed, delivered, rejected, sent };
}

async function main(): Promise<void> {
  process.stdout.write(JSON.stringify({ breaking: await run(45), fine: await run(10) }));
}

main().catch((e: unknown) => {
  process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  process.exit(1);
});
