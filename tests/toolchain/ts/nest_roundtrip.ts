// Round trip against a real NestJS app: the generated Nest module is booted on port 0 and the
// generated client runs the SAME scenario as the Express round trip (`roundtrip.ts`).
import 'reflect-metadata';
import assert from 'node:assert/strict';
import type { AddressInfo } from 'node:net';
import { Module } from '@nestjs/common';
import type { INestApplication } from '@nestjs/common';
import { NestFactory } from '@nestjs/core';
import { OrdersClient } from './copy_a/orders/client';
import { OrdersController, OrdersVeldModule, ORDERS_SERVICE } from './server/server/nest/orders';
import type * as srv from './server/index';
import { countRequests, hooks, impl, scenario } from './roundtrip';

async function boot(imports: NonNullable<Parameters<typeof Module>[0]>['imports']): Promise<{ app: INestApplication; base: string }> {
  @Module({ imports })
  class AppModule {}
  const app = await NestFactory.create(AppModule, { logger: false, abortOnError: false });
  app.use(countRequests);
  await app.listen(0, '127.0.0.1');
  const { port } = app.getHttpServer().address() as AddressInfo;
  return { app, base: `http://127.0.0.1:${port}` };
}

async function main(): Promise<void> {
  // The routes Nest (and Swagger) see are the real ones, one decorated method per endpoint.
  const proto = OrdersController.prototype as unknown as Record<string, () => void>;
  assert.equal(Reflect.getMetadata('path', OrdersController), 'orders');
  assert.equal(Reflect.getMetadata('path', proto['getOrder']!), ':id');
  assert.equal(Reflect.getMetadata('method', proto['getOrder']!), 0, 'GET');
  assert.equal(Reflect.getMetadata('path', proto['createOrder']!), '/');
  assert.equal(Reflect.getMetadata('method', proto['createOrder']!), 1, 'POST');
  assert.equal(Reflect.getMetadata('path', proto['payOrder']!), ':id/pay');
  assert.equal(Reflect.getMetadata('__httpCode__', proto['createOrder']!), 201);

  // 1. useValue: the full scenario, same assertions as the Express round trip.
  const value = await boot([OrdersVeldModule.forRoot({ useValue: impl, hooks })]);
  await scenario('nest', value.base);
  assert.equal(await fetch(`${value.base}/nothing`).then((r) => r.status), 404);

  // 2. useClass: Nest instantiates the implementation (with its own dependencies injected).
  const cls = await boot([
    OrdersVeldModule.forRoot({
      useClass: class implements srv.OrdersService {
        health = impl.health.bind(impl);
        getOrder = impl.getOrder.bind(impl);
        createOrder = impl.createOrder.bind(impl);
        listOrders = impl.listOrders.bind(impl);
        searchOrders = impl.searchOrders.bind(impl);
        payOrder = impl.payOrder.bind(impl);
        cancelOrder = impl.cancelOrder.bind(impl);
        boom = impl.boom.bind(impl);
      },
      hooks,
    }),
  ]);
  const clsClient = new OrdersClient({ baseUrl: cls.base, auth: { bearer: () => 'good' } });
  assert.equal(await clsClient.health(), undefined);

  // 3. useFactory with an injected provider, and the service token exported by the module.
  @Module({ providers: [{ provide: 'IMPL', useValue: impl }], exports: ['IMPL'] })
  class ImplModule {}
  const factory = await boot([
    OrdersVeldModule.forRoot({
      imports: [ImplModule],
      useFactory: (injected: srv.OrdersService) => injected,
      inject: ['IMPL'],
      hooks,
    }),
  ]);
  const facClient = new OrdersClient({ baseUrl: factory.base, auth: { bearer: () => 'good' } });
  assert.equal(await facClient.health(), undefined);
  assert.ok(factory.app.get(ORDERS_SERVICE) === impl, 'the implementation is injectable through its token');

  // A missing required hook fails at boot, not at the first request.
  await assert.rejects(
    boot([OrdersVeldModule.forRoot({ useValue: impl, hooks: {} as srv.OrdersHooks })]),
    /hooks\.auth\.bearer is required/,
  );

  await Promise.all([value.app.close(), cls.app.close(), factory.app.close()]);
  process.stdout.write('nest roundtrip ok\n');
  process.exit(0);
}

main().catch((e: unknown) => {
  process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  process.exit(1);
});
