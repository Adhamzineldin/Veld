// The generated Node server (plain node:http adapter) with the same Orders implementation as
// roundtrip.ts. Prints `PORT=<n>` and serves until stdin closes. Used by the Java cross-language test.
import { randomUUID } from 'node:crypto';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as srv from './server/index';

const USER_ID = '11111111-1111-4111-8111-111111111111';
const store = new Map<string, srv.Order>();
const user: srv.User = { id: USER_ID, email: 'ada@example.com', displayName: 'Ada' };

const impl: srv.OrdersService = {
  async getOrder(req) {
    const order = store.get(req.id);
    if (!order) throw new srv.NotFound({ resource: 'order' });
    return order;
  },
  async createOrder(req) {
    if (req.body.items.some((i) => i.sku === 'OUT-1')) throw new srv.OutOfStock({ sku: 'OUT-1' });
    if (req.idempotencyKey === 'quota') throw new srv.QuotaExceeded({ limit: 5 }, { status: 429 });
    const order: srv.Order = {
      id: randomUUID(),
      createdAt: '2024-05-01T12:00:00+02:00',
      customer: { ...user, id: req.body.customerId },
      items: req.body.items,
      status: srv.Status.pending,
      total: { amount: '10.50', currency: 'USD' },
      seq: '9007199254740993',
      ...(req.body.note === undefined ? {} : { note: req.body.note }),
      ...(req.body.payment === undefined ? {} : { payment: req.body.payment }),
    };
    if (req.idempotencyKey !== undefined) order.note = req.idempotencyKey;
    store.set(order.id, order);
    return order;
  },
  async listOrders(req) {
    const items = [...store.values()].filter((o) => req.query.status === undefined || o.status === req.query.status);
    const limited = items.slice(0, req.query.limit ?? 100);
    return { items: limited, total: String(items.length), page: 1, size: limited.length };
  },
  async searchOrders(req) {
    return [...store.values()].filter((o) => (o.note ?? '').includes(req.q));
  },
  async payOrder(req) {
    const order = store.get(req.id);
    if (!order) throw new srv.NotFound({});
    const paid: srv.Order = { ...order, status: srv.Status.paid, payment: req.body };
    store.set(paid.id, paid);
    return paid;
  },
  async cancelOrder(req) {
    if (!store.delete(req.id)) throw new srv.NotFound({ resource: 'order' });
  },
  async health() {},
  async boom() {
    throw new Error('secret internal detail');
  },
};

const hooks: srv.OrdersHooks = {
  auth: { bearer: async (token) => (token === 'good' ? { user: 'u1' } : null) },
  middleware: { RateLimit: async () => {} },
};

const runtime = { services: { iam: { url: 'http://iam.test' } } };

const server = http.createServer(srv.toNodeHttp(srv.createOrdersHandler(impl, hooks, runtime)));
server.listen(0, '127.0.0.1', () => {
  process.stdout.write(`PORT=${String((server.address() as AddressInfo).port)}\n`);
});
process.stdin.on('end', () => process.exit(0));
process.stdin.resume();
