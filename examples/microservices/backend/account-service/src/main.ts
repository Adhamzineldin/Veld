// account-service: accounts of a user, in memory. It asks `iam` who a token belongs to and asks
// `transactions` (a Java service) for the history shown in a statement, while `transactions`
// calls this service back to check accounts: the two consume each other.
// Everything is imported from the generated package `@veld/accounts-api`, including the clients
// of the consumed services, which the package re-exports as the namespaces `iam` and `transactions`.
import { randomUUID } from 'node:crypto';
import express from 'express';
import {
  AccountNotActive,
  Forbidden,
  NotFound,
  createAccountsHandler,
  iam,
  toExpress,
  transactions,
} from '@veld/accounts-api';
import type { Account, AccountsService } from '@veld/accounts-api';

interface Principal {
  user: iam.User;
  token: string;
}

const accounts = new Map<string, Account>();
const principalOf = (ctx: { principal: unknown }): Principal => ctx.principal as Principal;

/** Asks the iam service who owns `token`; null when it is not valid. */
async function authenticate(token: string): Promise<Principal | null> {
  try {
    const user = await new iam.IamClient({ auth: { bearer: () => token } }).getProfile({});
    return { user, token };
  } catch (e) {
    if (iam.Unauthorized.matches(e)) return null;
    throw e;
  }
}

/** The transactions service, called with the caller's own token. */
const transactionsFor = (p: Principal) =>
  new transactions.TransactionsClient({ auth: { bearer: () => p.token } });

function nextIban(): string {
  const digits = Array.from({ length: 16 }, () => Math.floor(Math.random() * 10)).join('');
  return `NX00${digits}`;
}

/** The account if `p` owns it. Someone else's account is reported as missing, not forbidden. */
function owned(p: Principal, id: string): Account {
  const account = accounts.get(id);
  if (!account || account.userId !== p.user.id) throw new NotFound({ resource: 'account' });
  return account;
}

function mustBeActive(account: Account): void {
  if (account.status !== 'active') throw new AccountNotActive({ status: account.status });
}

const impl: AccountsService = {
  async listAccounts(_req, ctx) {
    const p = principalOf(ctx);
    return [...accounts.values()].filter((a) => a.userId === p.user.id);
  },

  async getAccount({ id }, ctx) {
    return owned(principalOf(ctx), id);
  },

  async createAccount({ body }, ctx) {
    const account: Account = {
      id: randomUUID(),
      userId: principalOf(ctx).user.id,
      type: body.type,
      status: 'active',
      name: body.name,
      currency: body.currency,
      // Demo money: a real system would credit accounts through a ledger.
      balance: '1000.00',
      iban: nextIban(),
      createdAt: new Date().toISOString(),
    };
    accounts.set(account.id, account);
    return account;
  },

  async getSummary(_req, ctx) {
    const mine = [...accounts.values()].filter((a) => a.userId === principalOf(ctx).user.id);
    const cents = mine.reduce((sum, a) => sum + Math.round(Number(a.balance) * 100), 0);
    return {
      totalBalance: (cents / 100).toFixed(2),
      accountCount: mine.length,
      primaryAccount: mine[0],
    };
  },

  async getStatement({ id }, ctx) {
    const p = principalOf(ctx);
    const account = owned(p, id);
    // A typed call to the Java transactions service; its errors come back as typed errors.
    const page = await transactionsFor(p).getHistory({ accountId: id, query: { page: 1, size: 5 } });
    return { account, recent: page.items };
  },

  async freezeAccount({ id }, ctx) {
    const account = owned(principalOf(ctx), id);
    mustBeActive(account);
    const frozen: Account = { ...account, status: 'frozen' };
    accounts.set(id, frozen);
    return frozen;
  },

  async closeAccount({ id }, ctx) {
    const account = owned(principalOf(ctx), id);
    mustBeActive(account);
    accounts.set(id, { ...account, status: 'closed' });
  },

  async banAccount({ id }, ctx) {
    const p = principalOf(ctx);
    if (!p.user.email.endsWith('@nexusbank.example')) {
      throw new Forbidden({ message: 'staff only' });
    }
    const account = accounts.get(id);
    if (!account) throw new NotFound({ resource: 'account' });
    const banned: Account = { ...account, status: 'closed' };
    accounts.set(id, banned);
    return banned;
  },
};

const app = express();
app.use(toExpress(createAccountsHandler(impl, { auth: { bearer: authenticate } })));

const port = Number(process.env.PORT ?? 3002);
app.listen(port, process.env.HOST ?? '127.0.0.1', () => {
  console.log(`account-service listening on port ${port}`);
});
