import { useState } from 'react';
import { useCreateAccount, useGetStatement, useListAccounts } from '@veld/web/accounts/react';
import { Transfer } from './Transfer';

/**
 * Screen one: the user's accounts and, for the selected one, its statement. The statement is
 * assembled by the accounts service from the Java transactions service.
 */
export function Accounts() {
  const accounts = useListAccounts();
  const create = useCreateAccount();
  const [selected, setSelected] = useState<string | null>(null);
  const statement = useGetStatement(selected ? { id: selected } : null);

  async function open(): Promise<void> {
    const account = await create.mutate({ body: { type: 'checking', name: 'Everyday', currency: 'EUR' } });
    if (account) {
      accounts.refetch();
      setSelected(account.id);
    }
  }

  return (
    <section>
      <h2>Accounts</h2>
      <button data-testid="open-account" onClick={() => void open()}>
        Open a checking account
      </button>
      <ul data-testid="accounts">
        {(accounts.data ?? []).map((a) => (
          <li key={a.id}>
            <button data-testid={`select-${a.id}`} onClick={() => setSelected(a.id)}>
              {a.name}
            </button>{' '}
            {a.balance} {a.currency} ({a.iban})
          </li>
        ))}
      </ul>
      {statement.data && (
        <div data-testid="statement">
          <h3>Statement: {statement.data.account.name}</h3>
          <ul>
            {statement.data.recent.map((t) => (
              <li key={t.id}>
                {t.type} {t.amount} {t.currency} {t.description ?? ''}
              </li>
            ))}
          </ul>
          <Transfer
            accountId={statement.data.account.id}
            onDone={() => {
              statement.refetch();
              accounts.refetch();
            }}
          />
        </div>
      )}
    </section>
  );
}
