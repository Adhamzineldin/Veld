// IIAMService is Veld-generated from iam.veld — implement every method.
// The compiler will tell you if the contract and implementation ever drift.
import { IIAMService } from '@veld/generated/interfaces/IIAMService';
import {
  User, RegisterInput, LoginInput,
  TokenPair, RefreshInput, UpdateProfileInput,
} from '@veld/generated/types/iam';
import { iamErrors } from '@veld/generated/errors/iam.errors';
import { randomUUID } from 'crypto';
import bcrypt from 'bcrypt';
import jwt from 'jsonwebtoken';

const SECRET = process.env.JWT_SECRET ?? 'nexusbank-dev-secret';

// In-memory store — swap for Postgres/Redis in production.
const users: (User & { passwordHash: string })[] = [];

export class IamService implements IIAMService {
  async register(input: RegisterInput): Promise<User> {
    if (users.find(u => u.email === input.email))
      throw iamErrors.register.conflict('Email already registered');
    const passwordHash = await bcrypt.hash(input.password, 10);
    const user: User = {
      id: randomUUID(), email: input.email,
      firstName: input.firstName, lastName: input.lastName,
      phone: input.phone, createdAt: new Date().toISOString(),
    };
    users.push({ ...user, passwordHash });
    return user;
  }

  async login(input: LoginInput): Promise<TokenPair> {
    const found = users.find(u => u.email === input.email);
    if (!found || !await bcrypt.compare(input.password, found.passwordHash))
      throw iamErrors.login.unauthorized('Invalid credentials');
    return this.issueTokens(found.id);
  }

  async refreshToken(input: RefreshInput): Promise<TokenPair> {
    try {
      const { sub } = jwt.verify(input.refreshToken, SECRET) as { sub: string };
      return this.issueTokens(sub);
    } catch {
      throw iamErrors.refreshToken.unauthorized('Refresh token expired or invalid');
    }
  }

  async getProfile(req: any): Promise<User> {
    const found = users.find(u => u.id === req.userId);
    if (!found) throw iamErrors.getProfile.notFound('User not found');
    const { passwordHash: _, ...user } = found;
    return user;
  }

  async updateProfile(req: any, input: UpdateProfileInput): Promise<User> {
    const idx = users.findIndex(u => u.id === req.userId);
    if (idx === -1) throw iamErrors.updateProfile.notFound('User not found');
    Object.assign(users[idx], input);
    const { passwordHash: _, ...user } = users[idx];
    return user;
  }

  async logout(): Promise<void> {
    // Add jti to a Redis deny-list in production (short TTL = access token lifetime).
  }

  private issueTokens(userId: string): TokenPair {
    return {
      accessToken:  jwt.sign({ sub: userId }, SECRET, { expiresIn: '15m' }),
      refreshToken: jwt.sign({ sub: userId }, SECRET, { expiresIn: '7d' }),
      expiresIn: 900,
    };
  }
}
