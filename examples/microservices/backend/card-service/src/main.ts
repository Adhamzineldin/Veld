// card-service: payment cards, in memory, on Fastify. A card can only be requested for an account
// that the accounts service confirms, through the generated `accounts` client.
// Imported from the generated package `@veld/cards-api`.
import { randomInt, randomUUID } from 'node:crypto';
import Fastify from 'fastify';
import {
  CardCancelled,
  NotFound,
  accounts,
  createCardsHandler,
  iam,
  toFastify,
} from '@veld/cards-api';
import type { Card, CardsService } from '@veld/cards-api';

interface Principal {
  user: iam.User;
  token: string;
}

const cards = new Map<string, Card>();
const principalOf = (ctx: { principal: unknown }): Principal => ctx.principal as Principal;

async function authenticate(token: string): Promise<Principal | null> {
  try {
    const user = await new iam.IamClient({ auth: { bearer: () => token } }).getProfile({});
    return { user, token };
  } catch (e) {
    if (iam.Unauthorized.matches(e)) return null;
    throw e;
  }
}

function owned(p: Principal, id: string): Card {
  const card = cards.get(id);
  if (!card || card.userId !== p.user.id) throw new NotFound({ resource: 'card' });
  return card;
}

function mustNotBeCancelled(card: Card): void {
  if (card.status === 'cancelled') throw new CardCancelled({});
}

const impl: CardsService = {
  async listCards(_req, ctx) {
    const p = principalOf(ctx);
    return [...cards.values()].filter((c) => c.userId === p.user.id);
  },

  async getCard({ id }, ctx) {
    return owned(principalOf(ctx), id);
  },

  async requestCard({ body }, ctx) {
    const p = principalOf(ctx);
    try {
      const account = await new accounts.AccountsClient({ auth: { bearer: () => p.token } }).getAccount({
        id: body.accountId,
      });
      if (account.status !== 'active') throw new NotFound({ resource: 'active account' });
    } catch (e) {
      // The accounts service's own NotFound becomes this service's NotFound.
      if (accounts.NotFound.matches(e)) throw new NotFound({ resource: 'account' });
      throw e;
    }
    const expires = new Date();
    expires.setFullYear(expires.getFullYear() + 4);
    const card: Card = {
      id: randomUUID(),
      accountId: body.accountId,
      userId: p.user.id,
      type: body.type,
      status: 'active',
      network: body.network,
      last4: String(randomInt(0, 10000)).padStart(4, '0'),
      expiresAt: expires.toISOString().slice(0, 10),
      createdAt: new Date().toISOString(),
    };
    cards.set(card.id, card);
    return card;
  },

  async freezeCard({ id }, ctx) {
    const card = owned(principalOf(ctx), id);
    mustNotBeCancelled(card);
    const frozen: Card = { ...card, status: 'frozen' };
    cards.set(id, frozen);
    return frozen;
  },

  async cancelCard({ id }, ctx) {
    const card = owned(principalOf(ctx), id);
    mustNotBeCancelled(card);
    cards.set(id, { ...card, status: 'cancelled' });
  },
};

const app = Fastify();
app.all('/*', toFastify(createCardsHandler(impl, { auth: { bearer: authenticate } })));

const port = Number(process.env.PORT ?? 3004);
app.listen({ port, host: process.env.HOST ?? '127.0.0.1' }).then(
  () => console.log(`card-service listening on port ${port}`),
  (err: unknown) => {
    console.error(err);
    process.exit(1);
  },
);
