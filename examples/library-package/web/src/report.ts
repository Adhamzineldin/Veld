// A typed client script: places an order, then pays the big invoice, retrying on the shared
// `Unavailable` error. `@veld/web` is the generated client package (see `veld setup`); the
// errors from the `common` library are imported from it like every other generated type.
import { PlanLimitReached, Unavailable, createClients, fromEnv } from '@veld/web';

const auth = { bearer: () => process.env.API_TOKEN ?? 'demo-token' };
// The address of each service is a setting of the deployment. This script reads it from the
// variables VELD_ORDERS_URL and VELD_INVOICES_URL; the names are its own choice.
const runtime = fromEnv({
  'services.invoices.url': 'VELD_INVOICES_URL',
  'services.orders.url': 'VELD_ORDERS_URL',
});
const { orders, invoices } = createClients(runtime, { auth });

async function main(): Promise<void> {
  try {
    const order = await orders.createOrder({ body: { total: { amount: '49.90', currency: 'EUR' }, note: 'report' } });
    console.log(`order ${order.id} ${order.status}: ${order.total.amount} ${order.total.currency}`);
  } catch (e) {
    if (!PlanLimitReached.matches(e, 'orders/Orders')) throw e;
    console.log(`plan limit reached: ${e.data.limit}`);
  }

  const page = await invoices.listInvoices({ query: { page: 1, size: 10 } });
  const big = page.items.find((i) => Number(i.due.amount) > 5000);
  if (!big) throw new Error('no big invoice');
  for (let attempt = 1; ; attempt += 1) {
    try {
      const paid = await invoices.payInvoice({ id: big.id });
      console.log(`invoice ${paid.id} paid after ${attempt} attempt(s)`);
      return;
    } catch (e) {
      // The same `Unavailable` that the orders service can raise: one shared signature. An error
      // declared in a library is matched together with the service that raised it.
      if (!Unavailable.matches(e, 'invoices/Invoices') || attempt >= 3) throw e;
      await new Promise((r) => setTimeout(r, (e.data.retryAfter ?? 1) * 100));
    }
  }
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
