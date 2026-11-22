import { useMemo, useRef, useState } from 'react';
import { VeldProvider as AccountsProvider } from '@veld/web/accounts/react';
import { VeldProvider as IamProvider } from '@veld/web/iam/react';
import { VeldProvider as TransactionsProvider } from '@veld/web/transactions/react';
import { Accounts } from './Accounts';
import { SignIn } from './SignIn';
import { createClients } from './clients';
import type { Urls } from './clients';

/** The app: provides the generated clients to the hooks, then shows sign-in or the accounts. */
export function Bank({ urls }: { urls: Urls }) {
  const token = useRef<string | null>(null);
  const [signedIn, setSignedIn] = useState(false);
  const clients = useMemo(() => createClients(urls, () => token.current), [urls]);
  return (
    <IamProvider clients={{ iam: clients.iam }}>
      <AccountsProvider clients={{ accounts: clients.accounts }}>
        <TransactionsProvider clients={{ transactions: clients.transactions }}>
          <h1>NexusBank</h1>
          {signedIn ? (
            <Accounts />
          ) : (
            <SignIn
              onToken={(t) => {
                token.current = t;
                setSignedIn(true);
              }}
            />
          )}
        </TransactionsProvider>
      </AccountsProvider>
    </IamProvider>
  );
}
