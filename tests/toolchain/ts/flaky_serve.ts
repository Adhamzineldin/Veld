// The flaky server of flaky.ts on its own: prints `PORT=<n>` and serves until stdin closes.
// Used by the Java reconnect test.
import { serve } from './flaky';

void serve().then(({ base }) => {
  process.stdout.write(`PORT=${new URL(base).port}\n`);
});
process.stdin.on('end', () => process.exit(0));
process.stdin.resume();
