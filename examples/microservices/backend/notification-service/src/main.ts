// notification-service: notifications and preferences, in memory, on Hono. Tokens are checked
// with the iam service through its generated client.
// Imported from the generated package `@veld/notifications-api`.
import { randomUUID } from 'node:crypto';
import { serve } from '@hono/node-server';
import { Hono } from 'hono';
import { NotFound, createNotificationsHandler, iam, toHono } from '@veld/notifications-api';
import type { Notification, NotificationPreferences, NotificationsService } from '@veld/notifications-api';

interface Principal {
  user: iam.User;
  token: string;
}

const notifications = new Map<string, Notification[]>(); // by user id
const preferences = new Map<string, NotificationPreferences>();
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

/** A user's notifications. The first time we see a user they get a welcome message. */
function inbox(userId: string): Notification[] {
  let list = notifications.get(userId);
  if (!list) {
    list = [
      {
        id: randomUUID(),
        userId,
        type: 'system',
        title: 'Welcome to NexusBank',
        body: 'Your notification centre is ready.',
        read: false,
        createdAt: new Date().toISOString(),
      },
    ];
    notifications.set(userId, list);
  }
  return list;
}

function prefsOf(userId: string): NotificationPreferences {
  return preferences.get(userId) ?? { userId, transaction: true, security: true, marketing: false };
}

const impl: NotificationsService = {
  async listNotifications(_req, ctx) {
    return inbox(principalOf(ctx).user.id);
  },

  async markAsRead({ id }, ctx) {
    const list = inbox(principalOf(ctx).user.id);
    const at = list.findIndex((n) => n.id === id);
    if (at < 0) throw new NotFound({ resource: 'notification' });
    const read: Notification = { ...list[at]!, read: true };
    list[at] = read;
    return read;
  },

  async getPreferences(_req, ctx) {
    return prefsOf(principalOf(ctx).user.id);
  },

  async updatePreferences({ body }, ctx) {
    const userId = principalOf(ctx).user.id;
    const current = prefsOf(userId);
    const next: NotificationPreferences = {
      userId,
      transaction: body.transaction ?? current.transaction,
      security: body.security ?? current.security,
      marketing: body.marketing ?? current.marketing,
    };
    preferences.set(userId, next);
    return next;
  },
};

const app = new Hono();
app.use('*', toHono(createNotificationsHandler(impl, { auth: { bearer: authenticate } })));

const port = Number(process.env.PORT ?? 3005);
serve({ fetch: app.fetch, port, hostname: process.env.HOST ?? '127.0.0.1' }, () => {
  console.log(`notification-service listening on port ${port}`);
});
