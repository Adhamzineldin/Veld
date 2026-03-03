import { useState } from 'react';
import type { User, Todo } from '@veld/types';

// @ts-ignore
import styles from './App.module.css';

// ── Generated Hooks (TanStack Query) ────────────────────────────────
import { useListUsers, useCreateUser, useDeleteUser } from '@veld/hooks/usersHooks';
import { useListTodos, useCreateTodo, useUpdateTodo, useDeleteTodo } from '@veld/hooks/todosHooks';

// ── Generated API Client (for error code constants) ─────────────────
import { usersApi } from '@veld/client/usersApi';
import { todosApi } from '@veld/client/todosApi';

// ── Generated Error Type (for typed error handling) ─────────────────
import type { VeldApiError } from '@veld/client/_internal';

export default function App() {
  const [newName, setNewName] = useState('');
  const [newEmail, setNewEmail] = useState('');
  const [newTitle, setNewTitle] = useState('');
  const [mutationError, setMutationError] = useState<string | null>(null);

  // ── Queries — error is typed as VeldApiError ────────────────────
  const { data: users = [], isLoading: usersLoading, error: usersError } = useListUsers();
  const { data: todos = [], isLoading: todosLoading, error: todosError } = useListTodos();

  // ── Mutations — onError receives VeldApiError with .code & .status ──

  const createUserMutation = useCreateUser({
    onSuccess: () => {
      setNewName('');
      setNewEmail('');
      setMutationError(null);
    },
    onError: (error: VeldApiError) => {
      // Type-safe error code comparison using generated constants
      if (error.code === usersApi.errors.createUser.conflict) {
        setMutationError('A user with this email already exists.');
      } else if (error.code === usersApi.errors.createUser.userExists) {
        setMutationError('Invalid user data. Please check your input.');
      } else {
        setMutationError(`Failed to create user: ${error.message}`);
      }
    },
  });

  const deleteUserMutation = useDeleteUser({
    onError: (error: VeldApiError) => {
      if (error.code === usersApi.errors.deleteUser.notFound) {
        setMutationError('User not found — they may have been deleted already.');
      } else {
        setMutationError(`Failed to delete user (${error.status}): ${error.message}`);
      }
    },
  });

  const createTodoMutation = useCreateTodo({
    onSuccess: () => {
      setNewTitle('');
      setMutationError(null);
    },
    onError: (error: VeldApiError) => {
      if (error.code === todosApi.errors.createTodo.badRequest) {
        setMutationError('Invalid todo data.');
      } else {
        setMutationError(`Failed to create todo: ${error.message}`);
      }
    },
  });

  const updateTodoMutation = useUpdateTodo({
    onError: (error: VeldApiError) => {
      if (error.code === todosApi.errors.updateTodo.notFound) {
        setMutationError('Todo not found.');
      } else if (error.code === todosApi.errors.updateTodo.badRequest) {
        setMutationError('Invalid update data.');
      } else {
        setMutationError(`Failed to update todo: ${error.message}`);
      }
    },
  });

  const deleteTodoMutation = useDeleteTodo({
    onError: (error: VeldApiError) => {
      if (error.code === todosApi.errors.deleteTodo.notFound) {
        setMutationError('Todo already deleted.');
      } else {
        setMutationError(`Failed to delete todo: ${error.message}`);
      }
    },
  });

  // ── Handlers ────────────────────────────────────────────────────
  const handleAddUser = () => {
    // if (!newName.trim() || !newEmail.trim()) return;
    setMutationError(null);
    createUserMutation.mutate({ input: { name: newName.trim(), email: newEmail.trim() } });
  };

  const handleDeleteUser = (userId: string) => {
    setMutationError(null);
    deleteUserMutation.mutate({ id: userId });
  };

  const handleAddTodo = (userId: string) => {
    if (!newTitle.trim()) return;
    setMutationError(null);
    createTodoMutation.mutate({ input: { title: newTitle.trim(), userId } });
  };

  const handleToggleTodo = (todoId: string, completed: boolean) => {
    setMutationError(null);
    updateTodoMutation.mutate({ id: todoId, input: { completed: !completed } });
  };

  const handleDeleteTodo = (todoId: string) => {
    setMutationError(null);
    deleteTodoMutation.mutate({ id: todoId });
  };

  return (
    <div className={styles.container}>
      <header className={styles.header}>
        <h1>🚀 Veld — React + TypeScript Example</h1>
        <p>Full-stack type-safe API integration with React Query</p>
      </header>

      {/* ── Mutation Error Banner ───────────────────────────────── */}
      {mutationError && (
        <div className={styles.error} onClick={() => setMutationError(null)}>
          ⚠️ {mutationError} <span style={{ cursor: 'pointer', marginLeft: 8 }}>✕</span>
        </div>
      )}

      {/* ── Users Section ───────────────────────────────────────── */}
      <section className={styles.section}>
        <h2>👥 Users</h2>

        {/* Query error — usersError is VeldApiError with .code & .status */}
        {usersError && (
          <div className={styles.error}>
            Error loading users (HTTP {usersError.status}): {usersError.message}
          </div>
        )}

        {usersLoading ? (
          <p>Loading users...</p>
        ) : (
          <ul className={styles.list}>
            {users.map((user: User) => (
              <li key={user.id} className={styles.listItem}>
                <div className={styles.userInfo}>
                  <strong>{user.name}</strong>
                  <span className={styles.email}>{user.email}</span>
                </div>
                <div className={styles.actions}>
                  <button
                    onClick={() => handleDeleteUser(user.id)}
                    className={styles.buttonDanger}
                    disabled={deleteUserMutation.isPending}
                  >
                    {deleteUserMutation.isPending ? 'Deleting...' : 'Delete'}
                  </button>
                  <button
                    onClick={() => handleAddTodo(user.id)}
                    className={styles.buttonSecondary}
                  >
                    Add Todo
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}

        <div className={styles.form}>
          <input
            type="text"
            placeholder="Name"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
          />
          <input
            type="email"
            placeholder="Email"
            value={newEmail}
            onChange={(e) => setNewEmail(e.target.value)}
          />
          <button
            onClick={handleAddUser}
            className={styles.buttonPrimary}
            disabled={createUserMutation.isPending}
          >
            {createUserMutation.isPending ? 'Creating...' : 'Add User'}
          </button>
        </div>
      </section>

      {/* ── Todos Section ───────────────────────────────────────── */}
      <section className={styles.section}>
        <h2>✓ Todos</h2>

        {/* Query error — todosError is VeldApiError with .code & .status */}
        {todosError && (
          <div className={styles.error}>
            Error loading todos (HTTP {todosError.status}): {todosError.message}
          </div>
        )}

        {todosLoading ? (
          <p>Loading todos...</p>
        ) : (
          <ul className={styles.list}>
            {todos.map((todo: Todo) => (
              <li key={todo.id} className={styles.todoItem}>
                <label>
                  <input
                    type="checkbox"
                    checked={todo.completed}
                    onChange={() => handleToggleTodo(todo.id, todo.completed)}
                    disabled={updateTodoMutation.isPending}
                  />
                  <span>{todo.title}</span>
                </label>
                <button
                  onClick={() => handleDeleteTodo(todo.id)}
                  className={styles.buttonDanger}
                  disabled={deleteTodoMutation.isPending}
                >
                  Delete
                </button>
              </li>
            ))}
          </ul>
        )}

        <div className={styles.form}>
          <input
            type="text"
            placeholder="New todo"
            value={newTitle}
            onChange={(e) => setNewTitle(e.target.value)}
          />
          <button
            onClick={() => users[0] && handleAddTodo(users[0].id)}
            className={styles.buttonPrimary}
            disabled={createTodoMutation.isPending}
          >
            {createTodoMutation.isPending ? 'Creating...' : 'Add Todo'}
          </button>
        </div>
      </section>
    </div>
  );
}
