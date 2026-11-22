<script lang="ts">
  import { onMount } from 'svelte';
  import { createTodosStore } from '../generated/stores/todos.store';
  import { createUsersStore } from '../generated/stores/users.store';
  import type { Todo } from '../generated/types';

  const todos = createTodosStore();
  const users = createUsersStore();

  let items = $state<Todo[]>([]);
  let newTitle = $state('');
  let newUserId = $state('1');
  let errorMsg = $state<string | null>(null);

  // Subscribe to store state
  todos.loading.subscribe(() => {});
  todos.error.subscribe((e) => { errorMsg = e?.message ?? null; });

  async function load() {
    const result = await todos.ListTodos();
    if (result) items = result;
  }

  async function addTodo() {
    if (!newTitle.trim()) return;
    const created = await todos.CreateTodo({ title: newTitle.trim(), userId: newUserId });
    if (created) {
      items = [...items, created];
      newTitle = '';
    }
  }

  async function toggle(todo: Todo) {
    const updated = await todos.UpdateTodo(todo.id, { completed: !todo.completed });
    if (updated) items = items.map((t) => (t.id === updated.id ? updated : t));
  }

  async function remove(id: string) {
    await todos.DeleteTodo(id);
    items = items.filter((t) => t.id !== id);
  }

  onMount(load);
</script>

<main>
  <h1>Todo App</h1>

  {#if errorMsg}
    <p class="error">{errorMsg}</p>
  {/if}

  <form onsubmit={(e) => { e.preventDefault(); addTodo(); }}>
    <input bind:value={newTitle} placeholder="New todo title" />
    <input bind:value={newUserId} placeholder="User ID" style="width:80px" />
    <button type="submit">Add</button>
  </form>

  <ul>
    {#each items as todo (todo.id)}
      <li>
        <input type="checkbox" checked={todo.completed} onchange={() => toggle(todo)} />
        <span class:done={todo.completed}>{todo.title}</span>
        <button onclick={() => remove(todo.id)}>Delete</button>
      </li>
    {/each}
  </ul>
</main>

<style>
  main { max-width: 480px; margin: 2rem auto; font-family: sans-serif; }
  form { display: flex; gap: 0.5rem; margin-bottom: 1rem; }
  input[type="text"], input:not([type]) { flex: 1; padding: 0.4rem; }
  ul { list-style: none; padding: 0; }
  li { display: flex; align-items: center; gap: 0.5rem; padding: 0.3rem 0; }
  .done { text-decoration: line-through; color: #888; }
  .error { color: red; }
</style>
