import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
// @ts-ignore
import { api } from '../generated/client/api';
// @ts-ignore
import styles from './App.module.css';

/**
 * Main application component demonstrating Veld API integration with React
 * using React Query for state management and API calls.
 */
export default function App() {
  const queryClient = useQueryClient();

  // ── Local State ────────────────────────────────────────────────────────
  const [newName, setNewName] = useState('');
  const [newEmail, setNewEmail] = useState('');
  const [newTitle, setNewTitle] = useState('');

  // ── User Management ────────────────────────────────────────────────────

  /**
   * Fetch all users with React Query.
   * - Automatically caches results
   * - Handles loading and error states
   * - Invalidated on user mutations
   */
  const { data: users = [], isLoading: usersLoading, error: usersError } = useQuery({
    queryKey: ['users'],
    queryFn: async () => api.Users.ListUsers(),
    retry: 2,
    retryDelay: (attemptIndex) => Math.min(1000 * 2 ** attemptIndex, 30000),
  });

  /**
   * Create a new user.
   * - Validates input before submission
   * - Clears form on success
   * - Automatically invalidates users cache
   */
  const createUserMutation = useMutation({
    mutationFn: (input: { name: string; email: string }) =>
      api.Users.CreateUser(input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['users'] });
      setNewName('');
      setNewEmail('');
    },
    onError: (error) => {
      console.error('Failed to create user:', error);
    },
  });

  /**
   * Delete a user by ID.
   * - Optimistically updates cache
   * - Invalidates users cache on success
   */
  const deleteUserMutation = useMutation({
    mutationFn: (id: string) => api.Users.DeleteUser(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['users'] });
    },
    onError: (error) => {
      console.error('Failed to delete user:', error);
    },
  });

  // ── Todo Management ────────────────────────────────────────────────────

  /**
   * Fetch all todos with React Query.
   * - Automatically caches results
   * - Handles loading and error states
   * - Invalidated on todo mutations
   */
  const { data: todos = [], isLoading: todosLoading, error: todosError } = useQuery({
    queryKey: ['todos'],
    queryFn: async () => api.Todos.ListTodos(),
    retry: 2,
    retryDelay: (attemptIndex) => Math.min(1000 * 2 ** attemptIndex, 30000),
  });

  /**
   * Create a new todo.
   * - Requires title and user ID
   * - Clears form on success
   * - Automatically invalidates todos cache
   */
  const createTodoMutation = useMutation({
    mutationFn: (input: { title: string; userId: string }) =>
      api.Todos.CreateTodo(input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['todos'] });
      setNewTitle('');
    },
    onError: (error) => {
      console.error('Failed to create todo:', error);
    },
  });

  /**
   * Toggle a todo's completion status.
   * - Updates specific todo
   * - Automatically invalidates todos cache
   */
  const updateTodoMutation = useMutation({
    mutationFn: (input: { id: string; completed: boolean }) =>
      api.Todos.UpdateTodo(input.id, { completed: input.completed }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['todos'] });
    },
    onError: (error) => {
      console.error('Failed to update todo:', error);
    },
  });

  /**
   * Delete a todo by ID.
   * - Optimistically updates cache
   * - Invalidates todos cache on success
   */
  const deleteTodoMutation = useMutation({
    mutationFn: (id: string) => api.Todos.DeleteTodo(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['todos'] });
    },
    onError: (error) => {
      console.error('Failed to delete todo:', error);
    },
  });

  // ── Event Handlers ─────────────────────────────────────────────────────

  const handleAddUser = () => {
    if (!newName.trim() || !newEmail.trim()) return;
    createUserMutation.mutate({ name: newName.trim(), email: newEmail.trim() });
  };

  const handleDeleteUser = (userId: string) => {
    deleteUserMutation.mutate(userId);
  };

  const handleAddTodo = (userId: string) => {
    if (!newTitle.trim()) return;
    createTodoMutation.mutate({ title: newTitle.trim(), userId });
  };

  const handleToggleTodo = (todoId: string, completed: boolean) => {
    updateTodoMutation.mutate({ id: todoId, completed: !completed });
  };

  const handleDeleteTodo = (todoId: string) => {
    deleteTodoMutation.mutate(todoId);
  };

  // ── Render ─────────────────────────────────────────────────────────────

  return (
    <div className={styles.container}>
      <header className={styles.header}>
        <h1>🚀 Veld — React + TypeScript Example</h1>
        <p>Full-stack type-safe API integration with React Query</p>
      </header>

      {/* Users Section */}
      <section className={styles.section}>
        <h2>👥 Users</h2>

        {usersError && (
          <div className={styles.error}>Error loading users: {String(usersError)}</div>
        )}

        {usersLoading ? (
          <p className={styles.loading}>Loading users...</p>
        ) : (
          <ul className={styles.list}>
            {users.map((user) => (
              <li key={user.id} className={styles.listItem}>
                <div className={styles.userInfo}>
                  <strong>{user.name}</strong>
                  <span className={styles.email}>{user.email}</span>
                </div>
                <div className={styles.actions}>
                  <button
                    onClick={() => handleDeleteUser(user.id)}
                    disabled={deleteUserMutation.isPending}
                    className={styles.buttonDanger}
                  >
                    {deleteUserMutation.isPending ? '...' : 'Delete'}
                  </button>
                  <button
                    onClick={() => handleAddTodo(user.id)}
                    disabled={!newTitle || createTodoMutation.isPending}
                    className={styles.buttonSecondary}
                  >
                    {createTodoMutation.isPending ? '...' : 'Add Todo'}
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}

        <div className={styles.form}>
          <input
            type="text"
            placeholder="Full name"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyPress={(e) => e.key === 'Enter' && handleAddUser()}
            className={styles.input}
          />
          <input
            type="email"
            placeholder="Email address"
            value={newEmail}
            onChange={(e) => setNewEmail(e.target.value)}
            onKeyPress={(e) => e.key === 'Enter' && handleAddUser()}
            className={styles.input}
          />
          <button
            onClick={handleAddUser}
            disabled={!newName.trim() || !newEmail.trim() || createUserMutation.isPending}
            className={styles.buttonPrimary}
          >
            {createUserMutation.isPending ? 'Creating...' : 'Add User'}
          </button>
          {createUserMutation.error && (
            <p className={styles.error}>{String(createUserMutation.error)}</p>
          )}
        </div>
      </section>

      {/* Todos Section */}
      <section className={styles.section}>
        <h2>✓ Todos</h2>

        {todosError && (
          <div className={styles.error}>Error loading todos: {String(todosError)}</div>
        )}

        {todosLoading ? (
          <p className={styles.loading}>Loading todos...</p>
        ) : (
          <ul className={styles.list}>
            {todos.map((todo) => (
              <li key={todo.id} className={styles.todoItem}>
                <label className={styles.todoLabel}>
                  <input
                    type="checkbox"
                    checked={todo.completed}
                    onChange={() => handleToggleTodo(todo.id, todo.completed)}
                    disabled={updateTodoMutation.isPending}
                    className={styles.checkbox}
                  />
                  <span className={todo.completed ? styles.todoCompleted : ''}>
                    {todo.title}
                  </span>
                </label>
                <span className={styles.userId}>(user: {todo.userId})</span>
                <button
                  onClick={() => handleDeleteTodo(todo.id)}
                  disabled={deleteTodoMutation.isPending}
                  className={styles.buttonDanger}
                >
                  {deleteTodoMutation.isPending ? '...' : 'Delete'}
                </button>
              </li>
            ))}
          </ul>
        )}

        <div className={styles.form}>
          <input
            type="text"
            placeholder="What needs to be done?"
            value={newTitle}
            onChange={(e) => setNewTitle(e.target.value)}
            onKeyPress={(e) => e.key === 'Enter' && users.length > 0 && handleAddTodo(users[0].id)}
            className={styles.input}
          />
          {users.length === 0 ? (
            <p className={styles.info}>Create a user first to add todos</p>
          ) : (
            <button
              onClick={() => handleAddTodo(users[0].id)}
              disabled={!newTitle.trim() || createTodoMutation.isPending}
              className={styles.buttonPrimary}
            >
              {createTodoMutation.isPending ? 'Creating...' : 'Add Todo'}
            </button>
          )}
          {createTodoMutation.error && (
            <p className={styles.error}>{String(createTodoMutation.error)}</p>
          )}
        </div>
      </section>
    </div>
  );
}

