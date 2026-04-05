import express from 'express';
import { lendingRouter } from '@veld/generated/routes/lending.routes';
import { LendingService } from './services/LendingService';
import { AuthMiddleware } from './middleware/AuthMiddleware';

const app = express();
app.use(express.json());
app.use((_req, res, next) => {
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'GET,POST,OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type,Authorization');
  next();
});

const router = express.Router();
lendingRouter(router, new LendingService(), new AuthMiddleware());
app.use(router);

app.listen(process.env.PORT ?? 3005, () =>
  console.log(`[lending-service] http://localhost:${process.env.PORT ?? 3005}`));
