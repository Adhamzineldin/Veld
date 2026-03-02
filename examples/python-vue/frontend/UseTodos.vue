<script setup lang="ts">
import { ref, onMounted } from "vue";
import { api } from "../../generated/client/api";

// -- state ------------------------------------------------------------------
const todos  = ref<Awaited<ReturnType<typeof api.todos.listTodos>>>([]);
const users  = ref<Awaited<ReturnType<typeof api.users.listUsers>>>([]);
const newTitle  = ref("");
const newName   = ref("");
const newEmail  = ref("");

// -- data loading -----------------------------------------------------------
async function loadAll() {
  [todos.value, users.value] = await Promise.all([
    api.todos.listTodos(),
    api.users.listUsers(),
  ]);
}

onMounted(loadAll);

// -- mutations --------------------------------------------------------------
async function addUser() {
  if (!newName.value || !newEmail.value) return;
  await api.users.createUser({ name: newName.value, email: newEmail.value });
  newName.value = "";
  newEmail.value = "";
  await loadAll();
}

async function removeUser(id: string) {
  await api.users.deleteUser(id);
  await loadAll();
}

async function addTodo(userId: string) {
  if (!newTitle.value) return;
  await api.todos.createTodo({ title: newTitle.value, userId });
  newTitle.value = "";
  await loadAll();
}

async function toggleTodo(id: string, completed: boolean) {
  await api.todos.updateTodo(id, { completed: !completed });
  await loadAll();
}

async function removeTodo(id: string) {
  await api.todos.deleteTodo(id);
  await loadAll();
}
</script>

<template>
  <div style="font-family: sans-serif; max-width: 640px; margin: 2rem auto">
    <h1>Veld — python-vue example</h1>

    <section>
      <h2>Users</h2>
      <ul>
        <li v-for="u in users" :key="u.id">
          {{ u.name }} &lt;{{ u.email }}&gt;
          <button @click="removeUser(u.id)" style="margin-left: 8px">delete</button>
          <button @click="addTodo(u.id)" :disabled="!newTitle" style="margin-left: 4px">
            add todo
          </button>
        </li>
      </ul>
      <input v-model="newName"  placeholder="Name" />
      <input v-model="newEmail" placeholder="Email" style="margin-left: 4px" />
      <button @click="addUser" :disabled="!newName || !newEmail" style="margin-left: 4px">
        Add user
      </button>
    </section>

    <section style="margin-top: 2rem">
      <h2>Todos</h2>
      <input v-model="newTitle" placeholder="Todo title" />
      <ul>
        <li
          v-for="t in todos"
          :key="t.id"
          :style="{ textDecoration: t.completed ? 'line-through' : 'none' }"
        >
          <input type="checkbox" :checked="t.completed" @change="toggleTodo(t.id, t.completed)" />
          {{ t.title }}
          <span style="color: #888; font-size: 0.8em">(user: {{ t.userId }})</span>
          <button @click="removeTodo(t.id)" style="margin-left: 8px">delete</button>
        </li>
      </ul>
    </section>
  </div>
</template>
