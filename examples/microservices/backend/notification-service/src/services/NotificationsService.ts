import { INotificationsService } from '@veld/generated/interfaces/INotificationsService';
import {
  Notification, NotificationPreferences, UpdatePreferencesInput,
} from '@veld/generated/types/notifications';
import { notificationsErrors } from '@veld/generated/errors/notifications.errors';
import { randomUUID } from 'crypto';

const notifications: Notification[] = [
  {
    id: 'notif-001', userId: 'user-001', type: 'transaction',
    title: 'Payment received', body: 'EUR 2,500 salary credited to Main Checking',
    read: false, createdAt: '2024-04-01T08:00:00Z',
  },
  {
    id: 'notif-002', userId: 'user-001', type: 'security',
    title: 'New login detected', body: 'Sign-in from Chrome on Windows 11',
    read: false, createdAt: '2024-04-03T10:15:00Z',
  },
];

const preferences: NotificationPreferences[] = [
  { userId: 'user-001', transaction: true, security: true, marketing: false },
];

export class NotificationsService implements INotificationsService {
  async listNotifications(req: any): Promise<Notification[]> {
    return notifications.filter(n => n.userId === req.userId);
  }

  async markAsRead(req: any, id: string): Promise<Notification> {
    const n = notifications.find(n => n.id === id && n.userId === req.userId);
    if (!n) throw notificationsErrors.markAsRead.notFound(`Notification ${id} not found`);
    n.read = true;
    return n;
  }

  async getPreferences(req: any): Promise<NotificationPreferences> {
    return (
      preferences.find(p => p.userId === req.userId) ??
      { userId: req.userId, transaction: true, security: true, marketing: false }
    );
  }

  async updatePreferences(req: any, input: UpdatePreferencesInput): Promise<NotificationPreferences> {
    let prefs = preferences.find(p => p.userId === req.userId);
    if (!prefs) {
      prefs = { userId: req.userId, transaction: true, security: true, marketing: false };
      preferences.push(prefs);
    }
    if (input.transaction !== undefined) prefs.transaction = input.transaction;
    if (input.security    !== undefined) prefs.security    = input.security;
    if (input.marketing   !== undefined) prefs.marketing   = input.marketing;
    return prefs;
  }
}
