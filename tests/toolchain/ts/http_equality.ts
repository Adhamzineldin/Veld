// Equality over real HTTP. Two services (Alpha, Beta) of one contract run as two separately
// generated servers (`srv_alpha`, `srv_beta`, each with its own runtime copy). Two separately
// generated client packages (`cli_1`, `cli_2`) call them and decode the errors that arrive.
import assert from 'node:assert/strict';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import * as alphaSrv from './srv_alpha/index';
import * as betaSrv from './srv_beta/index';
import * as cli1 from './cli_1/index';
import * as cli2 from './cli_2/index';

function listen(handler: http.RequestListener): Promise<{ server: http.Server; url: string }> {
  return new Promise((resolve) => {
    const server = http.createServer(handler);
    server.listen(0, '127.0.0.1', () => {
      resolve({ server, url: `http://127.0.0.1:${String((server.address() as AddressInfo).port)}` });
    });
  });
}

async function caught(call: Promise<unknown>): Promise<cli1.VeldError> {
  try {
    await call;
  } catch (e) {
    return e as cli1.VeldError;
  }
  throw new Error('expected the call to fail');
}

async function main(): Promise<void> {
  const alpha = await listen(
    alphaSrv.toNodeHttp(
      alphaSrv.createAlphaHandler({
        async fetch(req) {
          throw new alphaSrv.Gone({ id: req.id });
        },
      }),
    ),
  );
  const beta = await listen(
    betaSrv.toNodeHttp(
      betaSrv.createBetaHandler({
        async fetch(req) {
          throw new betaSrv.Gone({ id: req.id });
        },
      }),
    ),
  );
  try {
    // Four clients: {package 1, package 2} x {Alpha, Beta}.
    const a1 = new cli1.AlphaClient({ baseUrl: alpha.url });
    const a2 = new cli2.AlphaClient({ baseUrl: alpha.url });
    const b1 = new cli1.BetaClient({ baseUrl: beta.url });
    const b2 = new cli2.BetaClient({ baseUrl: beta.url });
    assert.notEqual(cli1.VeldError, cli2.VeldError, 'the client packages are separate copies');
    assert.notEqual(alphaSrv.VeldError, betaSrv.VeldError, 'the server packages are separate copies');

    const ex1 = await caught(a1.fetch({ id: 'x' }));
    const ex2 = await caught(a2.fetch({ id: 'x' }));
    const ey2 = await caught(a2.fetch({ id: 'y' }));
    const bx1 = await caught(b1.fetch({ id: 'x' }));
    const bx2 = await caught(b2.fetch({ id: 'x' }));

    // Each client decodes into its own package's class.
    assert.ok(ex1 instanceof cli1.Gone && !(ex1 instanceof cli2.Gone));
    assert.ok(ex2 instanceof cli2.Gone && !(ex2 instanceof cli1.Gone));
    assert.equal(ex1.status, 410);

    // Same service, op and payload: equal, in both directions, across client packages.
    assert.ok(cli1.same(ex1, ex2), 'client 1 vs client 2');
    assert.ok(cli2.same(ex2, ex1), 'client 2 vs client 1');
    // ... and against an error built by the server package with the same op.
    const local = new alphaSrv.Gone({ id: 'x' }, { op: ex1.op });
    assert.ok(alphaSrv.same(local, ex1) && cli1.same(ex1, local), 'server-side copy vs decoded');

    // Different payload.
    assert.ok(!cli1.same(ex1, ey2) && !cli2.same(ey2, ex1), 'different payload');

    // The same error from the OTHER service: only the service differs.
    assert.ok(ex1.op && bx1.op && ex1.op !== bx1.op, 'ops differ');
    assert.equal(ex1.sig, bx1.sig, 'same error declaration');
    assert.equal(cli1.canonicalError(ex1), cli1.canonicalError(bx1), 'same payload');
    assert.match(ex1.op, /^veld:shop\/Alpha\.fetch@/);
    assert.match(bx1.op, /^veld:shop\/Beta\.fetch@/);
    assert.ok(!cli1.same(ex1, bx1) && !cli2.same(bx2, ex2) && !cli1.same(bx2, ex2), 'other service is not the same');
    assert.ok(cli1.same(bx1, bx2) && cli2.same(bx2, bx1), 'Beta decoded in both packages');
    process.stdout.write('http equality ok\n');
  } finally {
    alpha.server.close();
    beta.server.close();
  }
}

main().catch((e: unknown) => {
  process.stderr.write(`${String(e instanceof Error ? (e.stack ?? e.message) : e)}\n`);
  process.exit(1);
});
