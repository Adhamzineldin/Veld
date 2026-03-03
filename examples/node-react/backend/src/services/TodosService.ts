import { ITodosService } from "../../../generated/interfaces/ITodosService";
import { Todo, CreateTodoInput, UpdateTodoInput } from "../../../generated/types/todos";
import { todosErrors } from "../../../generated/errors/todos.errors";
import { randomUUID } from "crypto";

const store: Todo[] = [
  { id: "1", title: "Buy groceries", completed: false, userId: "1" },
  { id: "2", title: "Read docs",     completed: true,  userId: "2" },
];

export class TodosService implements ITodosService {
  async listTodos(): Promise<Todo[]> {
    return store;
  }

  async getTodo(id: string): Promise<Todo> {
    const todo = store.find((t) => t.id === id);
    if (!todo) throw todosErrors.getTodo.notFound(`Todo ${id} not found`);
    return todo;
  }

  async createTodo(input: CreateTodoInput): Promise<Todo> {
    if (!input.title) throw todosErrors.createTodo.badRequest("Title is required");
    const todo: Todo = { id: randomUUID(), completed: false, ...input };
    store.push(todo);
    return todo;
  }

  async updateTodo(id: string, input: UpdateTodoInput): Promise<Todo> {
    const todo = store.find((t) => t.id === id);
    if (!todo) throw todosErrors.updateTodo.notFound(`Todo ${id} not found`);
    if (!input.title && input.completed === undefined) {
      throw todosErrors.updateTodo.badRequest("At least one field must be provided");
    }
    if (input.title     !== undefined) todo.title     = input.title;
    if (input.completed !== undefined) todo.completed = input.completed;
    return todo;
  }

  async deleteTodo(id: string): Promise<void> {
    const idx = store.findIndex((t) => t.id === id);
    if (idx === -1) throw todosErrors.deleteTodo.notFound(`Todo ${id} not found`);
    store.splice(idx, 1);
  }
}
