import express from 'express';
import { accountsRouter } from '@veld/generated/routes/accounts.routes';
import { AccountsService } from './services/AccountsService';
import { AuthMiddleware } from './middleware/AuthMiddleware';

const app = express();
app.use(express.json());
app.use((_req, res, next) => {
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'GET,POST,PUT,DELETE,PATCH,OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type,Authorization');
  next();
});

const router = express.Router();
accountsRouter(router, new AccountsService(), new AuthMiddleware());
app.use(router);

app.listen(process.env.PORT ?? 3002, () =>
  console.log(`[account-service] http://localhost:${process.env.PORT ?? 3002}`));
