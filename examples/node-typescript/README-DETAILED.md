# Node.js + Vanilla TypeScript Example Setup Guide

## Quick Start

```bash
# Backend
cd node-typescript/backend && npm install && npm run dev
# Runs on http://localhost:3000

# Frontend (Run via tsx)
cd node-typescript/frontend && npm install && npx tsx src/index.ts
# Or build it as a module
```

## Backend Setup (Same as React)

```bash
cd backend
npm install
npm run dev
# localhost:3000
```

**Key Difference from React:**
- No UI framework
- Pure TypeScript with Express
- Fully typed API client

## Frontend Setup (Vanilla TypeScript)

### Installation

```bash
cd frontend
npm install
npx tsx src/index.ts    # Run directly with tsx
npm start              # Alternative script
```

### Project Structure

```
frontend/
├── src/
│   ├── index.ts        # Main entry point
│   ├── api/
│   │   └── client.ts   # API wrapper
│   └── utils/
│       └── logger.ts
├── package.json
├── tsconfig.json
└── README.md
```

## Usage Patterns

### Direct API Calls

```typescript
import { api } from '../generated/client/api'

// Fetch all todos
const todos = await api.Todos.ListTodos()
console.log(todos)

// Create a todo
const newTodo = await api.Todos.CreateTodo({
    title: 'Learn Veld',
    userId: '1'
})
console.log(`Created: ${newTodo.title}`)

// Update a todo
const updated = await api.Todos.UpdateTodo(newTodo.id, {
    completed: true
})

// Delete a todo
await api.Todos.DeleteTodo(newTodo.id)
```

### Error Handling

```typescript
try {
    const todo = await api.Todos.GetTodo('invalid-id')
} catch (error) {
    if (error instanceof Error) {
        console.error(`Error: ${error.message}`)
    }
}
```

### Type Safety

```typescript
// All types are inferred from generated code
const users: User[] = await api.Users.ListUsers()
const user: User = await api.Users.GetUser('1')

// Compile-time checking
const input: CreateTodoInput = {
    title: 'Buy milk',
    userId: '1'
    // Missing property? TypeScript error!
}
```

## Use Cases

### CLI Tool

```typescript
#!/usr/bin/env node
import { api } from './generated/client/api'

async function main() {
    const todos = await api.Todos.ListTodos()
    todos.forEach((t, i) => {
        const status = t.completed ? '✓' : ' '
        console.log(`${i+1}. [${status}] ${t.title}`)
    })
}

main().catch(console.error)
```

### Test Runner

```typescript
import { api } from './generated/client/api'

async function testAPI() {
    console.log('Testing API...')
    
    // Test user creation
    const user = await api.Users.CreateUser({
        name: 'Test User',
        email: 'test@example.com'
    })
    console.log('✓ User created:', user.id)
    
    // Test todo creation
    const todo = await api.Todos.CreateTodo({
        title: 'Test todo',
        userId: user.id
    })
    console.log('✓ Todo created:', todo.id)
    
    // Cleanup
    await api.Todos.DeleteTodo(todo.id)
    await api.Users.DeleteUser(user.id)
    console.log('✓ Cleanup successful')
}

testAPI().catch(console.error)
```

### Integration Test

```typescript
import assert from 'assert'
import { api } from './generated/client/api'

async function runTests() {
    console.log('Running integration tests...')
    
    // Test 1: List todos
    const todos = await api.Todos.ListTodos()
    assert(Array.isArray(todos), 'ListTodos should return array')
    console.log('✓ Test 1: ListTodos')
    
    // Test 2: Create and delete
    const newTodo = await api.Todos.CreateTodo({
        title: 'Integration test',
        userId: '1'
    })
    assert(newTodo.id, 'Should have ID')
    
    await api.Todos.DeleteTodo(newTodo.id)
    console.log('✓ Test 2: Create & Delete')
    
    console.log('\n✓ All tests passed!')
}

runTests().catch(err => {
    console.error('Test failed:', err.message)
    process.exit(1)
})
```

## Running the Examples

### Example 1: List All Todos

```bash
npx tsx example.ts
```

Output:
```
--- List todos ---
Todo {
  id: '1',
  title: 'Buy groceries',
  completed: false,
  userId: '1'
}
```

### Example 2: Create, Update, Delete

```typescript
async function crudDemo() {
    // Create
    const todo = await api.Todos.CreateTodo({
        title: 'New task',
        userId: '1'
    })
    console.log('Created:', todo)
    
    // Update
    const updated = await api.Todos.UpdateTodo(todo.id, {
        completed: true
    })
    console.log('Updated:', updated)
    
    // Delete
    await api.Todos.DeleteTodo(todo.id)
    console.log('Deleted:', todo.id)
}

crudDemo().catch(console.error)
```

## TypeScript Configuration

**tsconfig.json:**
```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext",
    "lib": ["ES2020"],
    "resolveJsonModule": true,
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true
  }
}
```

## Scripts

**package.json:**
```json
{
  "scripts": {
    "start": "npx tsx src/index.ts",
    "build": "tsc",
    "test": "npx tsx src/__tests__/api.test.ts",
    "dev": "npx tsx watch src/index.ts"
  }
}
```

## Environment Variables

**.env:**
```bash
API_URL=http://localhost:3000
API_TIMEOUT=5000
```

**Usage:**
```typescript
import dotenv from 'dotenv'
dotenv.config()

const apiUrl = process.env.API_URL || 'http://localhost:3000'
```

## Advanced Patterns

### Async Iterator

```typescript
async function* getAllTodos() {
    const todos = await api.Todos.ListTodos()
    for (const todo of todos) {
        yield todo
    }
}

for await (const todo of getAllTodos()) {
    console.log(todo.title)
}
```

### Retry Logic

```typescript
async function withRetry<T>(
    fn: () => Promise<T>,
    maxRetries = 3
): Promise<T> {
    for (let i = 0; i < maxRetries; i++) {
        try {
            return await fn()
        } catch (error) {
            if (i === maxRetries - 1) throw error
            await new Promise(r => setTimeout(r, 1000 * (i + 1)))
        }
    }
    throw new Error('Should not reach here')
}

const todos = await withRetry(() => api.Todos.ListTodos())
```

### Batch Operations

```typescript
async function createMultipleTodos(titles: string[], userId: string) {
    const promises = titles.map(title =>
        api.Todos.CreateTodo({ title, userId })
    )
    return Promise.all(promises)
}

const newTodos = await createMultipleTodos(
    ['Task 1', 'Task 2', 'Task 3'],
    'user-123'
)
```

## Bundling for Distribution

### Esbuild

```bash
npm install --save-dev esbuild

# Build standalone executable
npx esbuild src/index.ts --bundle --platform=node --outfile=dist/app.js

# Run
node dist/app.js
```

### Webpack

```bash
npm install --save-dev webpack webpack-cli ts-loader

# Build
npx webpack
```

## Advantages

✅ **No Framework Overhead** - Pure TypeScript
✅ **Small Bundle Size** - Only what you need
✅ **Type Safe** - Full TypeScript checking
✅ **Flexible** - Works anywhere JavaScript runs
✅ **Testing** - Easy to test pure functions
✅ **Learning** - Understand how everything works

## Resources

- [TypeScript Handbook](https://www.typescriptlang.org/docs/)
- [tsx - TypeScript Runner](https://tsx.is/)
- [Node.js APIs](https://nodejs.org/api/)
- [Veld Documentation](https://veld.dev)

---

Perfect for **TypeScript purists** and **simple scripts**! 📝

