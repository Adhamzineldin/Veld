import { IMiddleware } from '@veld/generated/middleware/IMiddleware';
import jwt from 'jsonwebtoken';

const SECRET = process.env.JWT_SECRET ?? 'nexusbank-dev-secret';

// Implements the Veld-generated IMiddleware contract.
// Any action with middleware: [AuthGuard] in the contract
// will have this injected into the generated route handler.
export class AuthMiddleware implements IMiddleware {
  AuthGuard(req: any, res: any, next: () => void): void {
    const header = req.headers['authorization'] ?? '';
    const token = header.startsWith('Bearer ') ? header.slice(7) : '';
    try {
      const payload = jwt.verify(token, SECRET) as { sub: string };
      req.userId = payload.sub;
      next();
    } catch {
      res.status(401).json({ error: 'Unauthorized', code: 'UNAUTHORIZED' });
    }
  }
}
