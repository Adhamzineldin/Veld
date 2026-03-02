import { useState } from "react";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../generated/client/api";

export default function App() {
  const qc = useQueryClient();
  const [newName,  setNewName]  = useState("");
  const [newEmail, setNewEmail] = useState("");
  const [newTitle, setNewTitle] = useState("");

  const { data: users = [] } = useQuery({
    queryKey: ["users"],
    queryFn: () => api.users.listUsers(),
  });

  const { data: todos = [] } = useQuery({
    queryKey: ["todos"],
    queryFn: () => api.todos.listTodos(),
  });

  const addUser = useMutation({
    mutationFn: () => api.users.createUser({ name: newName, email: newEmail }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["users"] });
      setNewName(""); setNewEmail("");
    },
  });

  const deleteUser = useMutation({
    mutationFn: (id: string) => api.users.deleteUser(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["users"] }),
  });

  const addTodo = useMutation({
    mutationFn: (userId: string) =>
      api.todos.createTodo({ title: newTitle, userId }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["todos"] });
      setNewTitle("");
    },
  });

  const toggleTodo = useMutation({
    mutationFn: ({ id, completed }: { id: string; completed: boolean }) =>
      api.todos.updateTodo(id, { completed }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["todos"] }),
  });

  return (
    <div style={{ fontFamily: "sans-serif", maxWidth: 640, margin: "2rem auto" }}>
      <h1>Veld — node-react example</h1>

      <section>
        <h2>Users</h2>
        <ul>
          {users.map((u) => (
            <li key={u.id}>
              {u.name} &lt;{u.email}&gt;
              <button onClick={() => deleteUser.mutate(u.id)} style={{ marginLeft: 8 }}>
                delete
              </button>
              <button
                onClick={() => addTodo.mutate(u.id)}
                disabled={!newTitle}
                style={{ marginLeft: 4 }}
              >
                add todo
              </button>
            </li>
          ))}
        </ul>
        <input placeholder="Name"  value={newName}  onChange={(e) => setNewName(e.target.value)} />
        <input placeholder="Email" value={newEmail} onChange={(e) => setNewEmail(e.target.value)} style={{ marginLeft: 4 }} />
        <button onClick={() => addUser.mutate()} disabled={!newName || !newEmail} style={{ marginLeft: 4 }}>
          Add user
        </button>
      </section>

      <section style={{ marginTop: "2rem" }}>
        <h2>Todos</h2>
        <input
          placeholder="Todo title"
          value={newTitle}
          onChange={(e) => setNewTitle(e.target.value)}
        />
        <ul>
          {todos.map((t) => (
            <li key={t.id} style={{ textDecoration: t.completed ? "line-through" : "none" }}>
              <input
                type="checkbox"
                checked={t.completed}
                onChange={() => toggleTodo.mutate({ id: t.id, completed: !t.completed })}
              />
              {t.title} <span style={{ color: "#888", fontSize: "0.8em" }}>(user: {t.userId})</span>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
