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

// What the server saw of the request it bound: its identity, checked inside the implementation.
const bound: { same?: boolean; foreign?: boolean; plainThrows?: boolean } = {};

async function main(): Promise<void> {
  const alpha = await listen(
    alphaSrv.toNodeHttp(
      alphaSrv.createAlphaHandler({
        async fetch(req) {
          throw new alphaSrv.Gone({ id: req.id });
        },
        async item(req) {
          return { id: req.id };
        },
        async thing(req) {
          return { id: req.id };
        },
        async echo(req) {
          // A model bound from the request carries its declaration: equal to `Item.of`, never to a `Thing`.
          bound.same = alphaSrv.same(req.body, alphaSrv.Item.of({ id: req.body.id }));
          bound.foreign = alphaSrv.same(req.body, alphaSrv.Thing.of({ id: req.body.id }));
          try {
            alphaSrv.same({ id: req.body.id }, req.body);
          } catch (e) {
            bound.plainThrows = e instanceof TypeError;
          }
          return { id: req.body.id };
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
    await models(a1, a2);
    process.stdout.write('http equality ok\n');
  } finally {
    alpha.server.close();
    beta.server.close();
  }
}

/** Models: identity is the declaration, not the data. */
async function models(a1: cli1.AlphaClient, a2: cli2.AlphaClient): Promise<void> {
  const item1 = await a1.item({ id: 'x' });
  const item2 = await a2.item({ id: 'x' });
  const thing1 = await a1.thing({ id: 'x' });
  const thing2 = await a2.thing({ id: 'x' });
  const other = await a1.item({ id: 'y' });

  // Same fields, same canonical bytes ...
  assert.equal(cli1.Item.canonical(item1), cli1.Thing.canonical(thing1));
  // ... but two declarations: never the same, in either order, across packages too.
  assert.notEqual(cli1.Item.sig, cli1.Thing.sig);
  assert.ok(!cli1.same(item1, thing1) && !cli1.same(thing1, item1), 'Item vs Thing');
  assert.ok(!cli1.same(item1, thing2) && !cli2.same(thing2, item1), 'Item vs Thing, across packages');
  assert.ok(!cli1.Item.same(item1, thing1 as unknown as cli1.Item), 'the companion refuses a foreign declaration too');

  // The same declaration with equal data is the same, across separately generated packages.
  assert.ok(cli1.same(item1, item2) && cli2.same(item2, item1), 'Item across client packages');
  assert.ok(cli1.same(thing1, thing2), 'Thing across client packages');
  assert.ok(!cli1.same(item1, other), 'different data');

  // A decoded value against a constructed one (and a package-2 constructed one).
  assert.ok(cli1.same(item1, cli1.Item.of({ id: 'x' })), 'decoded vs constructed');
  assert.ok(cli1.same(cli2.Item.of({ id: 'x' }), item1), 'constructed in package 2 vs decoded in package 1');
  assert.ok(!cli1.same(cli1.Item.of({ id: 'x' }), cli1.Thing.of({ id: 'x' })), 'constructed Item vs Thing');
  // The companion still works for plain objects: the declaration is implied.
  assert.ok(cli1.Item.same({ id: 'x' }, item1) && !cli1.Item.same({ id: 'z' }, item1));

  // An error against a model with identical data.
  const gone = await a1.fetch({ id: 'x' }).catch((e: unknown) => e as cli1.VeldError);
  assert.ok(gone instanceof cli1.Gone);
  assert.equal(cli1.canonical(gone.data), cli1.Item.canonical(item1), 'identical data');
  assert.ok(!cli1.same(gone, item1) && !cli1.same(item1, gone), 'error vs model');

  // Identity is invisible: not serialised, not enumerable, gone from a spread (which `same` then refuses).
  assert.equal(JSON.stringify(item1), '{"id":"x"}');
  assert.deepEqual(Object.keys(item1), ['id']);
  assert.equal(Object.getOwnPropertySymbols(item1).length, 2, 'two non-enumerable symbols');
  assert.throws(() => cli1.same({ ...item1 }, item1), TypeError, 'a spread has no identity');
  assert.throws(() => cli1.same({ id: 'x' }, item1), /Model\.same/, 'plain objects: the error names the companion');
  assert.throws(() => cli1.same('x', 'x'), TypeError, 'primitives have no identity');
  // Lists compare element by element.
  assert.ok(cli1.same([item1, thing1], [item2, thing2]) && !cli1.same([item1], [thing1]));
  assert.ok(!cli1.same([item1], [item1, item1]));

  // Server-side binding: the implementation saw a bound request body with its identity.
  const echoed = await a2.echo({ body: cli2.Item.of({ id: 'x' }) });
  assert.deepEqual(bound, { same: true, foreign: false, plainThrows: true });
  assert.ok(cli2.same(echoed, thing2), 'a response decodes as the declared Thing');
  assert.ok(!cli2.same(echoed, item2));
}

main().catch((e: unknown) => {
  process.stderr.write(`${String(e instanceof Error ? (e.stack ?? e.message) : e)}\n`);
  process.exit(1);
});
