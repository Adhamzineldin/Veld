import { IUsersService } from "@veld/generated/interfaces/IUsersService";
import { User, CreateUserInput, SuccessResponse } from "@veld/generated/types/users";
import { usersErrors } from "@veld/generated/errors/users.errors";
import { randomUUID } from "crypto";

const store: User[] = [
  { id: "1", name: "Alice", email: "alice@example.com" },
  { id: "2", name: "Bob",   email: "bob@example.com" },
];

export class UsersService implements IUsersService {
  async listUsers(): Promise<User[]> {
    return store;
  }

  async getUser(id: string): Promise<User> {
    const user = store.find((u) => u.id === id);

    if (!user) throw usersErrors.getUser.notFound(`User ${id} not found`);
    return user;
  }

  async createUser(input: CreateUserInput): Promise<User> {
    const exists = store.find((u) => u.email === input.email);
    if (exists) throw usersErrors.createUser.userExists(`Email ${input.email} already exists`);
    if (!input.name || !input.email) throw usersErrors.createUser.conflict("Name and email required");
    const user: User = { id: randomUUID(), ...input };
    store.push(user);
    return user;
  }

  async deleteUser(id: string): Promise<SuccessResponse> {
    const idx = store.findIndex((u) => u.id === id);
    if (idx === -1) throw usersErrors.deleteUser.notFound(`User ${id} not found`);
    store.splice(idx, 1);
    return { status: true };
  }
}
