import express from 'express';
import { notificationsRouter } from '@veld/generated/routes/notifications.routes';
import { NotificationsService } from './services/NotificationsService';
import { AuthMiddleware } from './middleware/AuthMiddleware';

const app = express();
app.use(express.json());
app.use((_req, res, next) => {
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'GET,PATCH,OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type,Authorization');
  next();
});

const router = express.Router();
notificationsRouter(router, new NotificationsService(), new AuthMiddleware());
app.use(router);

app.listen(process.env.PORT ?? 3006, () =>
  console.log(`[notification-service] http://localhost:${process.env.PORT ?? 3006}`));
