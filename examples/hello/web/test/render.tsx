// Renders the real <Greeter> against a running server (VELD_HELLO_URL) and checks the output.
import assert from 'node:assert/strict';
import { act, create } from 'react-test-renderer';
import { HelloClient } from '@veld/web';
import { VeldProvider } from '@veld/web/hello/react';
import { Greeter } from '../src/Greeter';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const base = process.env.VELD_HELLO_URL;
assert.ok(base, 'set VELD_HELLO_URL to the running server');
const client = new HelloClient({ baseUrl: base });

async function until(label: string, cond: () => boolean): Promise<void> {
  for (let i = 0; i < 300; i += 1) {
    if (cond()) return;
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });
  }
  assert.fail(`timed out waiting for: ${label}`);
}

async function main(): Promise<void> {
  let tree!: ReturnType<typeof create>;
  await act(async () => {
    tree = create(
      <VeldProvider clients={{ hello: client }}>
        <Greeter />
      </VeldProvider>,
    );
  });
  const flat = (n: unknown): string =>
    typeof n === 'string'
      ? n
      : Array.isArray(n)
        ? n.map(flat).join('')
        : n && typeof n === 'object' && 'children' in n
          ? flat((n as { children: unknown }).children)
          : '';
  const text = () => flat(tree.toJSON());
  const type = async (value: string) =>
    act(async () => {
      tree.root.findByType('input').props.onChange({ target: { value } });
    });

  assert.ok(!text().includes('Hello,'), 'nothing is requested before there is a name');

  await type('Ada');
  await until('the greeting', () => text().includes('Hello, Ada!'));

  await type('nobody');
  await until('the typed NotFound', () => text().includes('Nobody called nobody was found.'));
  assert.ok(!text().includes('Hello, Ada!'), 'stale data is not shown for another request');

  await act(async () => tree.unmount());
  console.log('render ok');
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
