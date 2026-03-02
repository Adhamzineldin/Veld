import { ITodosService } from "../../../generated/interfaces/ITodosService";
import { Todo, CreateTodoInput, UpdateTodoInput } from "../../../generated/types/todos";
import { randomUUID } from "crypto";

const store: Todo[] = [
  { id: "1", title: "Buy groceries", completed: false, userId: "1" },
  { id: "2", title: "Read docs",     completed: true,  userId: "2" },
];

export class TodosService implements ITodosService {
  async ListTodos(): Promise<Todo[]> {
    return store;
  }

  async GetTodo(id: string): Promise<Todo> {
    const todo = store.find((t) => t.id === id);
    if (!todo) throw new Error(`Todo ${id} not found`);
    return todo;
  }

  async CreateTodo(input: CreateTodoInput): Promise<Todo> {
    const todo: Todo = { id: randomUUID(), completed: false, ...input };
    store.push(todo);
    return todo;
  }

  async UpdateTodo(id: string, input: UpdateTodoInput): Promise<Todo> {
    const todo = store.find((t) => t.id === id);
    if (!todo) throw new Error(`Todo ${id} not found`);
    if (input.title     !== undefined) todo.title     = input.title;
    if (input.completed !== undefined) todo.completed = input.completed;
    return todo;
  }

  async DeleteTodo(id: string): Promise<void> {
    const idx = store.findIndex((t) => t.id === id);
    if (idx === -1) throw new Error(`Todo ${id} not found`);
    store.splice(idx, 1);
  }
}
