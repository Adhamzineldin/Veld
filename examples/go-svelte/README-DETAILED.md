# Go + Svelte Example Setup Guide

## Quick Start

```bash
# Terminal 1 - Backend
cd go-svelte/backend
go run main.go
# Runs on http://localhost:3000

# Terminal 2 - Frontend
cd go-svelte/frontend
npm install
npm run dev
# Runs on http://localhost:5173
```

## Backend Setup (Go)

### Prerequisites
- Go 1.19+

### Installation

```bash
cd backend
go mod download    # Download dependencies
go run main.go     # Run development server
```

### Key Go Features Used

**Goroutines & Channels:**
```go
// Concurrent request handling
go handleRequest(req)

// Channel communication
results := make(chan Todo)
go func() {
    results <- fetchTodo(id)
}()
```

**Interfaces:**
```go
type TodosService interface {
    ListTodos(ctx context.Context) ([]Todo, error)
    CreateTodo(ctx context.Context, input *CreateTodoInput) (*Todo, error)
}
```

**Error Handling:**
```go
todo, err := service.GetTodo(ctx, id)
if err != nil {
    return nil, fmt.Errorf("failed to get todo: %w", err)
}
```

## Frontend Setup (Svelte)

### Prerequisites
- Node.js 16+

### Installation

```bash
cd frontend
npm install
npm run dev     # Start development server
npm run build   # Build for production
```

### Key Svelte Features

**Reactive Variables:**
```svelte
<script>
    let count = 0;
    
    // Reactive update without hooks!
    function increment() {
        count += 1;
    }
</script>

<button on:click={increment}>
    Count: {count}
</button>
```

**Stores:**
```typescript
// stores/todos.ts
import { writable } from 'svelte/store'

export const todos = writable([])

// In component:
import { todos } from './stores/todos'
$todos // Auto-unsubscribe
```

**Two-way Binding:**
```svelte
<script>
    let name = '';
</script>

<!-- Updates 'name' automatically -->
<input bind:value={name} />
<p>Hello {name}!</p>
```

## Architecture Comparison

### Go vs Node.js
- **Go:** Compiled, single binary, built-in concurrency
- **Node.js:** Interpreted, npm ecosystem, familiar to JS devs

### Svelte vs React
- **Svelte:** Less code, no virtual DOM, smaller bundle
- **React:** Larger ecosystem, more community, hooks pattern

## Running Together

```bash
# Backend
go run main.go        # Port 3000

# Frontend (new terminal)
npm run dev          # Port 5173, auto-proxies to :3000
```

Visit `http://localhost:5173` and enjoy blazing-fast development!

## Production

### Backend
```bash
go build -o app
./app
```

Deploy to: Heroku, AWS Lambda, Google Cloud Run, Railway, etc.

### Frontend
```bash
npm run build  # Creates dist/
```

Deploy `dist/` to: Vercel, Netlify, static hosting, etc.

## Key Files

- `backend/main.go` - HTTP server setup
- `backend/services/todos.go` - Todo business logic
- `backend/services/users.go` - User business logic
- `frontend/src/App.svelte` - Main component
- `frontend/src/stores/` - Shared state with Svelte stores

## Type Safety in Go

```go
// Compile-time type checking
type Todo struct {
    ID        string `json:"id"`
    Title     string `json:"title"`
    Completed bool   `json:"completed"`
    UserId    string `json:"userId"`
}

// Return types are enforced
func (s *TodosService) ListTodos(ctx context.Context) ([]Todo, error) {
    // Must return []Todo and error
}
```

## API Calls in Svelte

```svelte
<script lang="ts">
    import { api } from '../generated/client/api'
    
    let todos = []
    let loading = true
    
    onMount(async () => {
        try {
            todos = await api.Todos.ListTodos()
        } finally {
            loading = false
        }
    })
    
    async function addTodo(title: string) {
        const todo = await api.Todos.CreateTodo({
            title,
            userId: '1'
        })
        todos = [...todos, todo]
    }
</script>
```

## Troubleshooting

### Go Dependencies
```bash
go mod tidy      # Clean up unused dependencies
go mod download  # Download all dependencies
```

### Svelte Build Issues
```bash
# Clear cache
rm -rf node_modules/.vite
npm run dev
```

### Port Conflicts
```bash
# Go: Change port in main.go
// PORT := ":3001"

# Svelte: Change port
npm run dev -- --port 5174
```

## Resources

- [Go Documentation](https://golang.org/doc)
- [Svelte Tutorial](https://svelte.dev/tutorial)
- [Veld Documentation](https://veld.dev)

---

Perfect for developers wanting **simplicity** and **performance**! 🚀

