import { AccountsClient, IamClient, TransactionsClient } from '@veld/web';

/** Where each service lives. In the browser they share one origin (a reverse proxy routes by path). */
export interface Urls {
  iam: string;
  accounts: string;
  transactions: string;
}

/** The same-origin default: nginx (docker compose) or the Vite dev proxy routes `/api/...`. */
export const sameOrigin = (): Urls => {
  const origin = window.location.origin;
  return { iam: origin, accounts: origin, transactions: origin };
};

/** The three generated clients. Each call reads the current token, so signing in needs no rebuild. */
export function createClients(urls: Urls, token: () => string | null) {
  const auth = { bearer: () => token() ?? '' };
  return {
    iam: new IamClient({ baseUrl: urls.iam, auth }),
    accounts: new AccountsClient({ baseUrl: urls.accounts, auth }),
    transactions: new TransactionsClient({ baseUrl: urls.transactions, auth }),
  };
}
