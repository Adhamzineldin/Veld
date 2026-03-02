import { IUsersService } from "../../../generated/interfaces/IUsersService";
import { User, CreateUserInput } from "../../../generated/types/users";
import { randomUUID } from "crypto";

const store: User[] = [
  { id: "1", name: "Alice", email: "alice@example.com" },
  { id: "2", name: "Bob",   email: "bob@example.com" },
];

export class UsersService implements IUsersService {
  async ListUsers(): Promise<User[]> {
    return store;
  }

  async GetUser(id: string): Promise<User> {
    const user = store.find((u) => u.id === id);
    if (!user) throw new Error(`User ${id} not found`);
    return user;
  }

  async CreateUser(input: CreateUserInput): Promise<User> {
    const user: User = { id: randomUUID(), ...input };
    store.push(user);
    return user;
  }

  async DeleteUser(id: string): Promise<void> {
    const idx = store.findIndex((u) => u.id === id);
    if (idx === -1) throw new Error(`User ${id} not found`);
    store.splice(idx, 1);
  }
}
