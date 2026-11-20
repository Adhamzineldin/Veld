// Renders the real <Bank> against the running services (VELD_IAM_URL, VELD_ACCOUNTS_URL,
// VELD_TRANSACTIONS_URL): sign up, open an account, read its statement (accounts -> Java
// transactions -> accounts), make a transfer and see a typed InsufficientFunds.
import assert from 'node:assert/strict';
import { act, create } from 'react-test-renderer';
import type { ReactTestInstance } from 'react-test-renderer';
import { Bank } from '../src/Bank';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const need = (name: string): string => {
  const v = process.env[name];
  assert.ok(v, `set ${name}`);
  return v;
};
const urls = {
  iam: need('VELD_IAM_URL'),
  accounts: need('VELD_ACCOUNTS_URL'),
  transactions: need('VELD_TRANSACTIONS_URL'),
};

const flat = (n: unknown): string =>
  typeof n === 'string'
    ? n
    : Array.isArray(n)
      ? n.map(flat).join('')
      : n && typeof n === 'object' && 'children' in n
        ? flat((n as { children: unknown }).children)
        : '';

async function main(): Promise<void> {
  let tree!: ReturnType<typeof create>;
  await act(async () => {
    tree = create(<Bank urls={urls} />);
  });
  const text = () => flat(tree.toJSON());
  const byId = (id: string): ReactTestInstance => tree.root.findByProps({ 'data-testid': id });
  const has = (id: string) => tree.root.findAllByProps({ 'data-testid': id }).length > 0;
  const type = (id: string, value: string) =>
    act(async () => {
      byId(id).props.onChange({ target: { value } });
    });
  const click = (id: string) =>
    act(async () => {
      byId(id).props.onClick();
    });
  async function until(label: string, cond: () => boolean): Promise<void> {
    for (let i = 0; i < 500; i += 1) {
      if (cond()) return;
      await act(async () => {
        await new Promise((r) => setTimeout(r, 10));
      });
    }
    assert.fail(`timed out waiting for: ${label}\n${text()}`);
  }

  const email = `render-${Date.now()}@example.com`;
  await type('email', email);
  await type('password', 'a-long-password-1');
  await click('register');
  await until('the accounts screen', () => text().includes('Accounts'));

  await click('open-account');
  await until('the new account with its statement', () => has('statement'));
  await until('the opening balance', () => text().includes('credit 1000.00 EUR Opening balance'));

  await type('to-iban', 'DE89370400440532013000');
  await type('amount', '5000.00');
  await click('send');
  await until('the typed InsufficientFunds', () => has('insufficient'));
  assert.ok(text().includes('1000.00 available, 5000.00 required'), text());

  await type('amount', '250.00');
  await click('send');
  await until('the receipt', () => has('receipt'));
  assert.ok(text().includes('New balance 750'), text());
  await until('the transfer in the statement', () => text().includes('transfer 250.00 EUR'));

  await act(async () => tree.unmount());
  console.log('render ok');
}

main().catch((e: unknown) => {
  console.error(e);
  process.exit(1);
});
