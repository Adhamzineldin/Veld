// The orders service. `Money`, `Currency`, `PageQuery` and the errors `PlanLimitReached` and
// `Unavailable` are declared once in the shared `common` library; they are imported here from the
// generated package like any other generated type, and the invoices service uses the same ones.
import { randomUUID } from 'node:crypto';
import express from 'express';
import {
  NotFound,
  PlanLimitReached,
  Unavailable,
  createOrdersHandler,
  toExpress,
} from '@veld/orders-api';
import type { Money, Order, OrdersService } from '@veld/orders-api';

const API_TOKEN = process.env.API_TOKEN ?? 'demo-token';
const MAX_OPEN_ORDERS = 3; // the "plan limit" of the demo
const orders = new Map<string, Order>();

/** Amounts are decimal strings; compare them without floating point error. */
const cents = (m: Money): number => Math.round(Number(m.amount) * 100);

const impl: OrdersService = {
  async getOrder({ id }) {
    const order = orders.get(id);
    if (!order) throw new NotFound({ resource: 'order' });
    return order;
  },

  async listOrders({ query }) {
    const newestFirst = [...orders.values()].sort((a, b) => b.createdAt.localeCompare(a.createdAt));
    const start = (query.page - 1) * query.size;
    return {
      items: newestFirst.slice(start, start + query.size),
      total: String(newestFirst.length),
      page: query.page,
      size: query.size,
    };
  },

  async createOrder({ body }) {
    if (process.env.ORDERS_MAINTENANCE === '1') {
      // The shared error from `common`: invoices raises the very same one.
      throw new Unavailable({ retryAfter: 30 });
    }
    const open = [...orders.values()].filter((o) => o.status === 'pending').length;
    if (open >= MAX_OPEN_ORDERS) throw new PlanLimitReached({ limit: `${MAX_OPEN_ORDERS} open orders` });
    if (cents(body.total) <= 0) throw new PlanLimitReached({ limit: 'orders must have a positive total' });
    const order: Order = {
      id: randomUUID(),
      status: 'pending',
      total: body.total,
      note: body.note,
      createdAt: new Date().toISOString(),
    };
    orders.set(order.id, order);
    return order;
  },
};

const app = express();
app.use(
  toExpress(
    createOrdersHandler(impl, {
      auth: { bearer: async (token) => (token === API_TOKEN ? { token } : null) },
    }),
  ),
);

const port = Number(process.env.PORT ?? 3001);
app.listen(port, process.env.HOST ?? '127.0.0.1', () => {
  console.log(`orders listening on port ${port}`);
});
