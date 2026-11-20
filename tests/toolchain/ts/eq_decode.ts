// Calls the Node servers through the generated TypeScript client and prints what decoding the
// declared error yields, one line per call: `service id class sig op canonical` (tab separated).
// tests/toolchain/java_eq/EqDecode.java prints the same lines; the test requires them to match.
// Usage: eq_decode.js <alpha url> <beta url>
import * as cli from './cli_1/index';

const [alphaUrl, betaUrl] = process.argv.slice(2);
if (!alphaUrl || !betaUrl) throw new Error('usage: eq_decode.js <alpha url> <beta url>');

async function line(service: string, id: string, call: Promise<unknown>): Promise<void> {
  try {
    await call;
  } catch (e) {
    if (!(e instanceof cli.VeldError)) throw e;
    const cls = e instanceof cli.Gone ? 'Gone' : e.constructor.name;
    process.stdout.write(`${[service, id, cls, e.sig, String(e.op), cli.canonicalError(e)].join('\t')}\n`);
    return;
  }
  throw new Error(`${service}.fetch must fail`);
}

async function main(): Promise<void> {
  const alpha = new cli.AlphaClient({ baseUrl: alphaUrl });
  const beta = new cli.BetaClient({ baseUrl: betaUrl });
  for (const id of ['x', 'y']) {
    await line('Alpha', id, alpha.fetch({ id }));
    await line('Beta', id, beta.fetch({ id }));
  }
}

main().catch((e: unknown) => {
  process.stderr.write(`${String(e instanceof Error ? (e.stack ?? e.message) : e)}\n`);
  process.exit(1);
});
