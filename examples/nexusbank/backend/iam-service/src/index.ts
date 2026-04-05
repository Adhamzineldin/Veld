import express from 'express';
import { iamRouter } from '@veld/generated/routes/iam.routes';
import { IamService } from './services/IamService';
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
iamRouter(router, new IamService(), new AuthMiddleware());
app.use(router);

const PORT = process.env.PORT ?? 3001;
app.listen(PORT, () => console.log(`[iam-service] http://localhost:${PORT}`));
