import { IAccountsService } from '@veld/generated/interfaces/IAccountsService';
import { Account, CreateAccountInput, AccountSummary } from '@veld/generated/types/accounts';
import { accountsErrors } from '@veld/generated/errors/accounts.errors';
import { randomUUID } from 'crypto';

// ── Veld SDK: typed client for the IAM service ────────────────────────────
// Auto-generated from iam.veld — used to verify user identity before writes.
import { IamClient } from '@veld/generated/sdk/iam';

/** Create a per-request IAM client that forwards the caller's auth token. */
function iamFor(req: any): IamClient {
  const token = req.headers?.authorization ?? '';
  return new IamClient(undefined, { Authorization: token });
}

const store: Account[] = [
  {
    id: 'acc-001', userId: 'user-001', type: 'checking', status: 'active',
    name: 'Main Checking', currency: 'EUR', balance: 4250.00,
    iban: 'DE89370400440532013000', createdAt: '2024-01-15T09:00:00Z',
  },
  {
    id: 'acc-002', userId: 'user-001', type: 'savings', status: 'active',
    name: 'Emergency Fund', currency: 'EUR', balance: 12000.00,
    iban: 'DE27200400600532013001', createdAt: '2024-02-01T10:30:00Z',
  },
];

export class AccountsService implements IAccountsService {
  async listAccounts(req: any): Promise<Account[]> {
    return store.filter(a => a.userId === req.userId);
  }

  async getAccount(req: any, id: string): Promise<Account> {
    const a = store.find(a => a.id === id && a.userId === req.userId);
    if (!a) throw accountsErrors.getAccount.notFound(`Account ${id} not found`);
    return a;
  }

  async createAccount(req: any, input: CreateAccountInput): Promise<Account> {
    if (!['checking', 'savings', 'investment'].includes(input.type))
      throw accountsErrors.createAccount.badRequest('Invalid account type');

    // ── SDK call: verify the user exists in the IAM service ───────────
    // Passes the auth token so IAM can identify the caller.
    try {
      await iamFor(req).getProfile();
    } catch {
      throw accountsErrors.createAccount.badRequest('Could not verify user identity via IAM service');
    }

    const account: Account = {
      id: randomUUID(), userId: req.userId, ...input,
      status: 'active', balance: 0,
      iban: `DE${Math.floor(Math.random() * 1e20).toString().padStart(20, '0').slice(0, 20)}`,
      createdAt: new Date().toISOString(),
    };
    store.push(account);
    return account;
  }

  async getSummary(req: any): Promise<AccountSummary> {
    const mine = store.filter(a => a.userId === req.userId);
    return {
      totalBalance: mine.reduce((s, a) => s + a.balance, 0),
      accountCount: mine.length,
      primaryAccount: mine.find(a => a.type === 'checking') ?? mine[0],
    };
  }

  async freezeAccount(req: any, id: string): Promise<Account> {
    const a = store.find(a => a.id === id && a.userId === req.userId);
    if (!a) throw accountsErrors.freezeAccount.notFound(`Account ${id} not found`);
    a.status = 'frozen';
    return a;
  }

  async closeAccount(req: any, id: string): Promise<void> {
    const idx = store.findIndex(a => a.id === id && a.userId === req.userId);
    if (idx === -1) throw accountsErrors.closeAccount.notFound(`Account ${id} not found`);
    store.splice(idx, 1);
  }
}
