// The typed runtime config of the generated Node server (`Orders`, which consumes `Iam`) and of its
// consumed clients (spec/runtime-config.md sections 3.1 to 5): the entry takes the runtime config as
// its last argument, checks it first and fails with every problem, `createClients` gives each client
// its URL and the options, a client without a URL is refused, and nothing reads the environment on
// its own. Expects `./server` (node, `Orders` consuming `Iam`) generated from tests/toolchain/contract.
import assert from 'node:assert/strict';
import * as srv from './server/index';
import { hooks, impl, runtime } from './roundtrip';

type Call = { url: string; headers: Record<string, string> };

function spy(calls: Call[]): srv.veld.FetchLike {
  return (async (url: string, init?: { headers?: Record<string, string> }) => {
    calls.push({ url, headers: init?.headers ?? {} });
    throw new Error('offline');
  }) as unknown as srv.veld.FetchLike;
}

function failure(run: () => unknown): srv.VeldConfigError {
  try {
    run();
  } catch (e) {
    assert.ok(e instanceof srv.VeldConfigError, String(e));
    return e;
  }
  return assert.fail('expected a VeldConfigError');
}

async function main(): Promise<void> {
  // 1. The entry: the runtime config is the argument after the hooks, and the handler works with it.
  assert.equal(typeof srv.createOrdersHandler(impl, hooks, runtime), 'function');

  // 2. A bad runtime config fails when the handler is created, with the whole list, in path order.
  const bad = failure(() =>
    srv.createOrdersHandler(impl, hooks, { services: { iam: { url: 'ftp://iam' }, extra: { url: 'http://x' } }, extra: 1 } as never),
  );
  assert.deepEqual(
    bad.problems.map((p) => p.message),
    ['extra is not a setting', 'services.extra is not a setting', 'services.iam.url must be an http or https URL'],
  );
  assert.equal(
    bad.message,
    'invalid runtime config: extra is not a setting; services.extra is not a setting; services.iam.url must be an http or https URL',
  );
  assert.equal(bad.name, 'VeldConfigError');

  // 3. The server consumes `Iam`, so the argument is required: leaving it out fails with the path.
  const left = failure(() => (srv.createOrdersHandler as (...args: unknown[]) => unknown)(impl, hooks));
  assert.equal(left.message, 'invalid runtime config: services.iam.url is missing');

  // 4. The check runs first: a runtime config and hooks that are both wrong report the runtime config.
  const both = failure(() => srv.createOrdersHandler(impl, {} as srv.OrdersHooks, {} as srv.ApiRuntime));
  assert.equal(both.message, 'invalid runtime config: services.iam.url is missing');
  assert.throws(() => srv.createOrdersHandler(impl, {} as srv.OrdersHooks, runtime), /hooks\.auth\.bearer is required/);

  // 5. `createClients`: every client gets its URL (a trailing slash is dropped) and the same options.
  const calls: Call[] = [];
  const api = srv.createClients(
    { services: { iam: { url: 'http://iam.test///' } } },
    { fetch: spy(calls), headers: { 'x-trace': 'on' }, baseUrl: 'http://elsewhere.test' } as never,
  );
  assert.deepEqual(Object.keys(api), ['iam']);
  assert.ok(api.iam instanceof srv.iam.IamClient);
  await assert.rejects(api.iam.getUser({ id: '11111111-1111-4111-8111-111111111111' }), srv.VeldTransportError);
  assert.equal(calls.length, 1);
  assert.ok(calls[0]?.url.startsWith('http://iam.test/iam/users/'), String(calls[0]?.url));
  assert.equal(calls[0]?.headers['x-trace'], 'on', 'the options reach the client');
  // `createClients` checks the runtime config first.
  assert.equal(
    failure(() => srv.createClients({ services: { iam: { url: 'iam' } } })).message,
    'invalid runtime config: services.iam.url must be an http or https URL',
  );

  // 6. `fromEnv`: the names are the application's, the check is the same, and `process.env` is the default.
  const env = { IAM_URL: 'http://iam:3001/' };
  assert.deepEqual(srv.fromEnv({ 'services.iam.url': 'IAM_URL' }, env), { services: { iam: { url: 'http://iam:3001' } } });
  assert.equal(
    failure(() => srv.fromEnv({ 'services.iam.url': 'IAM_URL' }, {})).message,
    'invalid runtime config: services.iam.url is missing (environment variable IAM_URL is not set)',
  );
  process.env['VELD_RUNTIME_TEST_IAM'] = 'http://from-process-env';
  assert.equal(srv.fromEnv({ 'services.iam.url': 'VELD_RUNTIME_TEST_IAM' }).services.iam.url, 'http://from-process-env');
  delete process.env['VELD_RUNTIME_TEST_IAM'];

  // 7. Removed: the environment fallback and the compiled-in URL. A client needs `baseUrl`.
  process.env['VELD_IAM_URL'] = 'http://from-the-environment';
  const noUrl = () => new srv.iam.IamClient({ fetch: spy([]) } as never);
  assert.throws(noUrl, (e: unknown) => e instanceof srv.VeldTransportError && e.message === 'no base URL: pass baseUrl');
  delete process.env['VELD_IAM_URL'];

  process.stdout.write('runtime config ok\n');
}

main().catch((e: unknown) => {
  process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
  process.exit(1);
});
