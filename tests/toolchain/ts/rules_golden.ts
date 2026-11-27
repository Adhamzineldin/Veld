// Answers the fixed request of tests/wire/rules/request.json with the generated Node server and prints
// `{ status, contentType, body }` as one JSON line. The Rust test canonicalises `body` (RFC 8785) and compares
// it byte for byte with tests/wire/rules/problem.json. Expects `./server` (node) generated from
// tests/toolchain/contract/rules and the environment variable `WIRE_DIR`.
import * as fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import type { AddressInfo } from 'node:net';
import * as srv from './server/index';

interface Fixed {
  method: string;
  path: string;
  headers: Record<string, string>;
  body: unknown;
}

const impl: srv.BookingsService = {
  async create(req) {
    return req.body;
  },
  async search() {
    return 0;
  },
  async report() {
    return { parts: [], total: 0 };
  },
};

async function main(): Promise<void> {
  const dir = process.env['WIRE_DIR'];
  if (dir === undefined) throw new Error('WIRE_DIR is not set');
  const fixed = JSON.parse(fs.readFileSync(path.join(dir, 'rules', 'request.json'), 'utf8')) as Fixed;
  const server = http.createServer(srv.toNodeHttp(srv.createBookingsHandler(impl, {})));
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const port = (server.address() as AddressInfo).port;
  const res = await fetch(`http://127.0.0.1:${String(port)}${fixed.path}`, {
    method: fixed.method,
    headers: fixed.headers,
    body: JSON.stringify(fixed.body),
  });
  const body = await res.text();
  console.log(JSON.stringify({ status: res.status, contentType: res.headers.get('content-type'), body }));
  server.close();
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
