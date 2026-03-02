# Python + Vue.js 3 Example Setup Guide

## Project Structure

```
python-vue/
├── backend/
│   ├── app.py
│   ├── requirements.txt
│   ├── services/
│   │   ├── todos_service.py
│   │   ├── users_service.py
│   │   └── __init__.py
│   └── venv/
│
├── frontend/
│   ├── src/
│   │   ├── UseTodos.vue
│   │   ├── main.ts
│   │   └── App.vue
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.json
│   └── index.html
│
├── veld/
│   ├── app.veld
│   ├── veld.config.json
│   ├── models/
│   └── modules/
│
└── generated/
    ├── client/
    │   └── api.ts
    ├── interfaces/
    └── types/
```

## Backend Setup (Python + Flask)

### Prerequisites
- Python 3.9+
- pip (Python package manager)

### Installation

```bash
cd backend

# Create virtual environment
python -m venv venv

# Activate virtual environment
# On macOS/Linux:
source venv/bin/activate
# On Windows:
venv\Scripts\activate

# Install dependencies
pip install -r requirements.txt

# Run development server
python app.py
# Server runs on http://localhost:5000
```

### Backend Dependencies (requirements.txt)

```txt
Flask==3.0.0
Flask-CORS==4.0.0
Flask-RESTful==0.3.10
python-dotenv==1.0.0
```

### Key Backend Files

**app.py** - Main Flask application:
```python
from flask import Flask
from flask_cors import CORS
from services.todos_service import TodosService
from services.users_service import UsersService

app = Flask(__name__)
CORS(app, resources={r"/api/*": {"origins": "*"}})

todos_service = TodosService()
users_service = UsersService()

# Routes registered here
```

**services/todos_service.py** - Todo operations:
```python
class TodosService:
    def list_todos(self):
        """Returns all todos"""
        
    def get_todo(self, id: str):
        """Returns single todo by ID"""
        
    def create_todo(self, input):
        """Creates new todo"""
        
    def update_todo(self, id: str, input):
        """Updates todo"""
        
    def delete_todo(self, id: str):
        """Deletes todo"""
```

**services/users_service.py** - User operations:
```python
class UsersService:
    def list_users(self):
        """Returns all users"""
        
    def get_user(self, id: str):
        """Returns single user by ID"""
        
    def create_user(self, input):
        """Creates new user"""
        
    def delete_user(self, id: str):
        """Deletes user"""
```

## Code Generation

```bash
cd veld
veld generate
```

This generates:
- `/generated/client/api.ts` - TypeScript API client
- `/generated/interfaces/` - Service interfaces
- `/generated/types/` - Type definitions

## Frontend Setup (Vue.js 3)

### Prerequisites
- Node.js 16+
- npm or yarn

### Installation

```bash
cd frontend

# Install dependencies
npm install

# Start development server
npm run dev
# Frontend runs on http://localhost:5173

# Build for production
npm run build

# Preview production build
npm run preview
```

### Frontend Dependencies

**Key packages:**
- `vue` - Vue.js 3 framework
- `typescript` - TypeScript support
- `vite` - Lightning-fast build tool
- `axios` or `fetch` - HTTP client

### Key Frontend Files

**src/main.ts** - Application entry point:
```typescript
import { createApp } from 'vue'
import App from './App.vue'

createApp(App).mount('#app')
```

**src/UseTodos.vue** - Main component with API integration:
```vue
<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { api } from '../../generated/client/api'

// State
const todos = ref([])
const users = ref([])
const newTitle = ref('')

// Load data on mount
onMounted(async () => {
  todos.value = await api.Todos.ListTodos()
  users.value = await api.Users.ListUsers()
})

// Create todo
async function addTodo(userId: string) {
  const todo = await api.Todos.CreateTodo({
    title: newTitle.value,
    userId
  })
  todos.value.push(todo)
  newTitle.value = ''
}

// Update todo
async function toggleTodo(id: string, completed: boolean) {
  const updated = await api.Todos.UpdateTodo(id, {
    completed: !completed
  })
  const index = todos.value.findIndex(t => t.id === id)
  todos.value[index] = updated
}

// Delete todo
async function removeTodo(id: string) {
  await api.Todos.DeleteTodo(id)
  todos.value = todos.value.filter(t => t.id !== id)
}
</script>

<template>
  <div>
    <!-- UI here -->
  </div>
</template>

<style scoped>
/* Styles here */
</style>
```

## Running Both Backend and Frontend

### Terminal 1 - Backend
```bash
cd python-vue/backend
source venv/bin/activate  # or venv\Scripts\activate on Windows
python app.py
```

### Terminal 2 - Frontend
```bash
cd python-vue/frontend
npm run dev
```

### Expected Output

**Backend:**
```
 * Running on http://127.0.0.1:5000
 * Debug mode: on
```

**Frontend:**
```
  VITE v5.0.0  ready in 0 ms

  ➜  Local:   http://localhost:5173/
  ➜  press h to show help
```

Visit `http://localhost:5173` in your browser to see the app!

## Vue.js 3 Features Used

### Composition API (`<script setup>`)
Modern, concise Vue syntax:
```vue
<script setup lang="ts">
// Variables automatically exposed to template
const message = ref('Hello!')

// No need for data(), computed(), methods
const count = computed(() => items.value.length)

function increment() { count.value++ }
</script>

<template>
  <p>{{ message }}</p>
  <button @click="increment">Count: {{ count }}</button>
</template>
```

### Reactivity
- `ref()` - Reactive value wrapper
- `computed()` - Derived reactive values
- `watch()` - React to changes

### Lifecycle Hooks
- `onMounted()` - After component renders
- `onUnmounted()` - Before component unmounts
- `onUpdated()` - After update
- `onBeforeMount()` / `onBeforeUnmount()`

### Template Features
- `{{ variable }}` - Text interpolation
- `v-bind:attr="value"` or `:attr="value"` - Bind attributes
- `v-on:click="handler"` or `@click="handler"` - Event binding
- `v-for="item in items"` - List rendering
- `v-if` / `v-else` / `v-show` - Conditional rendering
- `v-model` - Two-way binding

## Type Safety

Every API call is fully typed:

```typescript
// TypeScript knows return type
const users = await api.Users.ListUsers()
// users is User[]

// TypeScript knows parameter types
const newTodo = await api.Todos.CreateTodo({
  title: 'Buy milk',
  userId: '123'
})
// newTodo is Todo

// Errors caught at compile time
await api.Todos.CreateTodo({
  title: 'Buy milk',
  // userId: REQUIRED - Error if missing!
})
```

## Environment Variables

Create `frontend/.env.local`:
```
VITE_API_URL=http://localhost:5000/api
```

Access in code:
```typescript
const apiUrl = import.meta.env.VITE_API_URL
```

## Development Workflow

1. **Start backend:**
   ```bash
   cd backend
   python app.py
   ```

2. **Start frontend (another terminal):**
   ```bash
   cd frontend
   npm run dev
   ```

3. **Make changes:**
   - Edit `.py` files → Flask auto-reloads
   - Edit `.vue` files → Vite hot-reloads
   - Edit Veld contracts → run `veld generate`

4. **Test in browser:**
   - Open `http://localhost:5173`
   - Check Network tab for API calls
   - Check Console for errors

## Common Tasks

### Add a New Endpoint

1. Update `veld/modules/todos.veld`:
   ```
   action SearchTodos {
     method: POST
     path: /search
     input: SearchInput
     output: Todo[]
   }
   ```

2. Generate code:
   ```bash
   cd veld && veld generate
   ```

3. Implement in `backend/services/todos_service.py`:
   ```python
   def search_todos(self, input):
       # Implementation
   ```

4. Use in `frontend/src/UseTodos.vue`:
   ```typescript
   const results = await api.Todos.SearchTodos(searchInput)
   ```

### Debug API Calls

Check browser DevTools Network tab:
- Filter by XHR
- Click each request to see:
  - Request headers and body
  - Response status and body
  - Timing information

### Handle Errors

```typescript
try {
  const user = await api.Users.GetUser('invalid-id')
} catch (error) {
  console.error('Failed to get user:', error)
  // Show error message to user
}
```

## Troubleshooting

### Port Already in Use

**Backend:**
```bash
# Use different port
PORT=5001 python app.py
```

**Frontend:**
```bash
# Use different port
npm run dev -- --port 5174
```

### CORS Errors

**Backend should have:**
```python
from flask_cors import CORS
CORS(app, resources={r"/api/*": {"origins": "*"}})
```

### Virtual Environment Not Activating

```bash
# On Windows:
venv\Scripts\activate

# On macOS/Linux:
source venv/bin/activate

# Verify:
which python  # should show path in venv/
```

### npm Packages Not Found

```bash
cd frontend
rm -rf node_modules package-lock.json
npm install
```

### Types Not Updating

```bash
cd veld
rm -rf generated/
veld generate
```

## Production Deployment

### Backend (Python)

Install production dependencies:
```bash
pip install gunicorn python-dotenv
```

Run with Gunicorn:
```bash
gunicorn -w 4 -b 0.0.0.0:5000 app:app
```

Deploy to:
- Heroku
- Railway
- PythonAnywhere
- AWS Elastic Beanstalk
- DigitalOcean App Platform

### Frontend (Vue.js)

Build for production:
```bash
npm run build
# Creates dist/ folder
```

Deploy `dist/` to:
- Vercel
- Netlify
- GitHub Pages
- Any static hosting

## Python Best Practices

### Type Hints
```python
from typing import List, Dict, Optional

def get_todos(self) -> List[Dict[str, str]]:
    """Return list of todos"""
    pass

def get_todo(self, id: str) -> Optional[Dict[str, str]]:
    """Return single todo or None"""
    pass
```

### Error Handling
```python
try:
    todo = self.todos[id]
except KeyError:
    raise ValueError(f"Todo {id} not found")
except Exception as e:
    raise RuntimeError(f"Database error: {str(e)}")
```

### Service Pattern
```python
class TodosService:
    def __init__(self):
        self.todos = {}
    
    def list_todos(self) -> List:
        return list(self.todos.values())
```

## Vue.js Best Practices

### Keep Components Small
```vue
<!-- Too complex? Split into smaller components -->
<template>
  <TodoForm @submit="handleSubmit" />
  <TodoList :todos="todos" @delete="removeTodo" />
</template>
```

### Use Computed for Derived State
```typescript
const filteredTodos = computed(() => {
  return todos.value.filter(t => !t.completed)
})
```

### Extract Composables
```typescript
// composables/useTodos.ts
export function useTodos() {
  const todos = ref([])
  
  const addTodo = async (input) => {
    const todo = await api.Todos.CreateTodo(input)
    todos.value.push(todo)
  }
  
  return { todos, addTodo }
}

// In component:
const { todos, addTodo } = useTodos()
```

## Resources

- [Vue.js 3 Documentation](https://vuejs.org/)
- [Vue Composition API Guide](https://vuejs.org/guide/extras/composition-api-faq.html)
- [Vite Documentation](https://vitejs.dev/)
- [TypeScript Handbook](https://www.typescriptlang.org/docs/)
- [Flask Documentation](https://flask.palletsprojects.com/)
- [Veld Documentation](https://veld.dev)

---

**Now you have a complete Python + Vue.js 3 example ready to go!** 🎉

