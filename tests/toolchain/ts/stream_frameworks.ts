// Streaming on real Fastify and Hono servers: the generated adapters (`toFastify`, `toHono`) serve
// the streaming contract and the generated TypeScript client runs the same SSE and WebSocket
// round trips as for node:http (stream_roundtrip.ts). `ws` endpoints are served by `ws` attached to
// the framework's own Node `http.Server`: `fastify.server`, and the server `@hono/node-server`
// hosts Hono on.
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import { getRequestListener } from '@hono/node-server';
import Fastify from 'fastify';
import { Hono } from 'hono';
import { WebSocketServer } from 'ws';
import * as srv from './server/index';
import { attachWebSocket } from './server/server/ws';
import { authTests, sseTests, validatedSseTests, wsTests } from './stream_roundtrip';
import { bothHandlers, type Running } from './stream_server';

type Framework = (options: { validateResponses: boolean }) => Promise<Running>;

async function listening(server: http.Server): Promise<string> {
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
  return `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
}

function closer(server: http.Server, wss: WebSocketServer, after: () => Promise<void>): () => Promise<void> {
  return async () => {
    for (const c of wss.clients) c.terminate();
    server.closeAllConnections();
    await after();
  };
}

const fastify: Framework = async (options) => {
  const handler = bothHandlers(options);
  const app = Fastify();
  app.all('/*', srv.toFastify(handler) as never);
  await app.ready();
  const wss = new WebSocketServer({ server: app.server });
  attachWebSocket(wss, handler);
  const url = await listening(app.server);
  return { url, close: closer(app.server, wss, () => app.close()) };
};

const hono: Framework = async (options) => {
  const handler = bothHandlers(options);
  const app = new Hono();
  app.use('*', srv.toHono(handler) as never);
  const server = http.createServer(getRequestListener(app.fetch));
  const wss = new WebSocketServer({ server });
  attachWebSocket(wss, handler);
  const url = await listening(server);
  return { url, close: closer(server, wss, () => new Promise<void>((r) => server.close(() => r()))) };
};

async function run(name: string, start: Framework): Promise<void> {
  const plain = await start({ validateResponses: false });
  const checked = await start({ validateResponses: true });
  try {
    await sseTests(plain.url);
    await validatedSseTests(checked.url);
    await wsTests(plain.url);
    await authTests(plain.url);
    console.log(`${name}: sse, validated sse, ws and auth ok`);
  } finally {
    await plain.close();
    await checked.close();
  }
}

async function main(): Promise<void> {
  await run('fastify', fastify);
  await run('hono', hono);
  console.log('stream frameworks ok');
}

main().then(
  () => process.exit(0),
  (err: unknown) => {
    console.error(err);
    process.exit(1);
  },
);
