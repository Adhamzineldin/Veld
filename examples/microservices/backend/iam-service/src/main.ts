// iam-service: registration, login and profiles, with in-memory storage.
// `@veld/iam-api` is the generated package (see `veld setup`).
import { randomBytes, randomUUID, scryptSync, timingSafeEqual } from 'node:crypto';
import express from 'express';
import { EmailTaken, Unauthorized, NotFound, createIamHandler, toExpress } from '@veld/iam-api';
import type { IamService, User } from '@veld/iam-api';

interface Account {
  user: User;
  salt: string;
  hash: Buffer;
}

interface Session {
  userId: string;
  expiresAt: number;
}

/** What the bearer hook hands to the implementation as `ctx.principal`. */
interface Principal {
  user: User;
  token: string;
}

const ACCESS_SECONDS = 3600;
const users = new Map<string, Account>(); // by id
const byEmail = new Map<string, string>(); // email -> id
const access = new Map<string, Session>();
const refresh = new Map<string, string>(); // refresh token -> user id

const hashPassword = (password: string, salt: string): Buffer => scryptSync(password, salt, 32);

function issueTokens(userId: string) {
  const accessToken = randomBytes(24).toString('hex');
  const refreshToken = randomBytes(24).toString('hex');
  access.set(accessToken, { userId, expiresAt: Date.now() + ACCESS_SECONDS * 1000 });
  refresh.set(refreshToken, userId);
  return { accessToken, refreshToken, expiresIn: ACCESS_SECONDS };
}

const principalOf = (ctx: { principal: unknown }): Principal => ctx.principal as Principal;

const iam: IamService = {
  async register({ body }) {
    const email = body.email.toLowerCase();
    if (byEmail.has(email)) throw new EmailTaken({ email });
    const salt = randomBytes(16).toString('hex');
    const user: User = {
      id: randomUUID(),
      email,
      firstName: body.firstName,
      lastName: body.lastName,
      phone: body.phone,
      createdAt: new Date().toISOString(),
    };
    users.set(user.id, { user, salt, hash: hashPassword(body.password, salt) });
    byEmail.set(email, user.id);
    return user;
  },

  async login({ body }) {
    const id = byEmail.get(body.email.toLowerCase());
    const account = id ? users.get(id) : undefined;
    const attempt = account ? hashPassword(body.password, account.salt) : null;
    if (!account || !attempt || !timingSafeEqual(attempt, account.hash)) {
      throw new Unauthorized({ message: 'wrong email or password' });
    }
    return issueTokens(account.user.id);
  },

  async refreshToken({ body }) {
    const id = refresh.get(body.refreshToken);
    if (!id) throw new Unauthorized({ message: 'unknown refresh token' });
    refresh.delete(body.refreshToken); // refresh tokens are single use
    return issueTokens(id);
  },

  async getProfile(_req, ctx) {
    return principalOf(ctx).user;
  },

  async updateProfile({ body }, ctx) {
    const account = users.get(principalOf(ctx).user.id);
    if (!account) throw new NotFound({ resource: 'user' });
    account.user = {
      ...account.user,
      firstName: body.firstName ?? account.user.firstName,
      lastName: body.lastName ?? account.user.lastName,
      phone: body.phone ?? account.user.phone,
    };
    return account.user;
  },

  async logout(_req, ctx) {
    access.delete(principalOf(ctx).token);
  },
};

const handler = createIamHandler(iam, {
  auth: {
    // Resolves a bearer token to the principal. Other services call `getProfile` to use it.
    bearer: async (token): Promise<Principal | null> => {
      const session = access.get(token);
      if (!session || session.expiresAt < Date.now()) return null;
      const account = users.get(session.userId);
      return account ? { user: account.user, token } : null;
    },
  },
});

const app = express();
app.use(toExpress(handler));

const port = Number(process.env.PORT ?? 3001);
app.listen(port, process.env.HOST ?? '127.0.0.1', () => {
  console.log(`iam-service listening on port ${port}`);
});
