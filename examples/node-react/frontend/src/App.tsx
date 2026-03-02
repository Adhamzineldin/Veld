import {useState} from 'react';
import {
    useUsersListUsers,
    useUsersCreateUser,
    useUsersDeleteUser,
    useTodosListTodos,
    useTodosCreateTodo,
    useTodosUpdateTodo,
    useTodosDeleteTodo,
} from '@veld/hooks';
import type {User, Todo} from '@veld/types';
// @ts-ignore
import styles from './App.module.css';


export default function App() {
    const [newName, setNewName] = useState('');
    const [newEmail, setNewEmail] = useState('');
    const [newTitle, setNewTitle] = useState('');

    // Use generated hooks for queries
    const {data: users = [], isLoading: usersLoading, error: usersError} = useUsersListUsers();
    const {data: todos = [], isLoading: todosLoading, error: todosError} = useTodosListTodos();

    // Use generated hooks for mutations
    const createUserMutation = useUsersCreateUser({
        onSuccess: () => {
            setNewName('');
            setNewEmail('');
        },
    });

    const deleteUserMutation = useUsersDeleteUser();

    const createTodoMutation = useTodosCreateTodo({
        onSuccess: () => {
            setNewTitle('');
        },
    });

    const updateTodoMutation = useTodosUpdateTodo();

    const deleteTodoMutation = useTodosDeleteTodo();
    const handleAddUser = () => {
        if (!newName.trim() || !newEmail.trim()) return;
        createUserMutation.mutate({input: {name: newName.trim(), email: newEmail.trim()}});
    };

    const handleDeleteUser = (userId: string) => deleteUserMutation.mutate({id: userId});

    const handleAddTodo = (userId: string) => {
        if (!newTitle.trim()) return;
        createTodoMutation.mutate({input: {title: newTitle.trim(), userId}});
    };

    const handleToggleTodo = (todoId: string, completed: boolean) =>
        updateTodoMutation.mutate({id: todoId, input: {completed: !completed}});

    const handleDeleteTodo = (todoId: string) => deleteTodoMutation.mutate({id: todoId});
    return (
        <div className={styles.container}>
            <header className={styles.header}>
                <h1>🚀 Veld — React + TypeScript Example</h1>
                <p>Full-stack type-safe API integration with React Query</p>
            </header>
            <section className={styles.section}>
                <h2>👥 Users</h2>
                {usersError && <div className={styles.error}>Error loading users</div>}
                {usersLoading ? <p>Loading users...</p> : (
                    <ul className={styles.list}>
                        {users.map((user: User) => (
                            <li key={user.id} className={styles.listItem}>
                                <div className={styles.userInfo}>
                                    <strong>{user.name}</strong>
                                    <span className={styles.email}>{user.email}</span>
                                </div>
                                <div className={styles.actions}>
                                    <button onClick={() => handleDeleteUser(user.id)}
                                            className={styles.buttonDanger}>Delete
                                    </button>
                                    <button onClick={() => handleAddTodo(user.id)}
                                            className={styles.buttonSecondary}>Add Todo
                                    </button>
                                </div>
                            </li>
                        ))}
                    </ul>
                )}
                <div className={styles.form}>
                    <input type="text" placeholder="Name" value={newName} onChange={(e) => setNewName(e.target.value)}/>
                    <input type="email" placeholder="Email" value={newEmail}
                           onChange={(e) => setNewEmail(e.target.value)}/>
                    <button onClick={handleAddUser} className={styles.buttonPrimary}>Add User</button>
                </div>
            </section>
            <section className={styles.section}>
                <h2>✓ Todos</h2>
                {todosError && <div className={styles.error}>Error loading todos</div>}
                {todosLoading ? <p>Loading todos...</p> : (
                    <ul className={styles.list}>
                        {todos.map((todo: Todo) => (
                            <li key={todo.id} className={styles.todoItem}>
                                <label><input type="checkbox" checked={todo.completed}
                                              onChange={() => handleToggleTodo(todo.id, todo.completed)}/><span>{todo.title}</span></label>
                                <button onClick={() => handleDeleteTodo(todo.id)}
                                        className={styles.buttonDanger}>Delete
                                </button>
                            </li>
                        ))}
                    </ul>
                )}
                <div className={styles.form}>
                    <input type="text" placeholder="New todo" value={newTitle}
                           onChange={(e) => setNewTitle(e.target.value)}/>
                    <button onClick={() => users[0] && handleAddTodo(users[0].id)} className={styles.buttonPrimary}>Add
                        Todo
                    </button>
                </div>
            </section>
        </div>
    );
}
