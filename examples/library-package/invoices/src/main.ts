// The invoices service, on Fastify. It uses the same shared `Money` and `Unavailable` from the
// `common` library as the orders service (imported from the generated `@veld/invoices-api`).
import { randomUUID } from 'node:crypto';
import Fastify from 'fastify';
import { NotFound, Unavailable, createInvoicesHandler, toFastify } from '@veld/invoices-api';
import type { Invoice, InvoicesService, Money } from '@veld/invoices-api';

const API_TOKEN = process.env.API_TOKEN ?? 'demo-token';
const BIG = 500000; // cents: the payment provider is flaky above 5000.00

const money = (amount: string, currency: Money['currency']): Money => ({ amount, currency });
const today = () => new Date().toISOString().slice(0, 10);

const invoices = new Map<string, Invoice>(
  [
    ['120.00', 'EUR'],
    ['75.50', 'USD'],
    ['9800.00', 'GBP'],
  ].map(([amount, currency], i) => {
    const invoice: Invoice = {
      id: randomUUID(),
      orderId: randomUUID(),
      due: money(amount!, currency as Money['currency']),
      paid: false,
      issuedAt: `2026-01-0${i + 1}`,
    };
    return [invoice.id, invoice];
  }),
);
const flaky = new Set<string>(); // invoices whose first payment attempt already failed

const impl: InvoicesService = {
  async getInvoice({ id }) {
    const invoice = invoices.get(id);
    if (!invoice) throw new NotFound({ resource: 'invoice' });
    return invoice;
  },

  async listInvoices({ query }) {
    const newestFirst = [...invoices.values()].sort((a, b) => b.issuedAt.localeCompare(a.issuedAt));
    const start = (query.page - 1) * query.size;
    return {
      items: newestFirst.slice(start, start + query.size),
      total: String(newestFirst.length),
      page: query.page,
      size: query.size,
    };
  },

  async payInvoice({ id }) {
    const invoice = invoices.get(id);
    if (!invoice) throw new NotFound({ resource: 'invoice' });
    if (Math.round(Number(invoice.due.amount) * 100) > BIG && !flaky.has(id)) {
      flaky.add(id);
      throw new Unavailable({ retryAfter: 1 }); // the shared error from `common`
    }
    const paid: Invoice = { ...invoice, paid: true };
    invoices.set(id, paid);
    return paid;
  },
};

const app = Fastify();
app.all(
  '/*',
  toFastify(
    createInvoicesHandler(impl, {
      auth: { bearer: async (token) => (token === API_TOKEN ? { token } : null) },
    }),
  ),
);

const port = Number(process.env.PORT ?? 3002);
app.listen({ port, host: process.env.HOST ?? '127.0.0.1' }).then(
  () => console.log(`invoices listening on port ${port}`),
  (err: unknown) => {
    console.error(err);
    process.exit(1);
  },
);
