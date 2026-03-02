# Node.js + React + TypeScript Example Setup Guide

## Project Structure

```
node-react/
├── backend/
│   ├── src/
│   │   ├── services/
│   │   │   ├── UsersService.ts
│   │   │   └── TodosService.ts
│   │   └── index.ts
│   ├── package.json
│   └── tsconfig.json
│
├── frontend/
│   ├── src/
│   │   ├── App.tsx
│   │   ├── App.module.css
│   │   ├── index.css
│   │   └── main.tsx
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── index.html
│   └── vite.env.d.ts
│
├── veld/
│   ├── app.veld
│   ├── veld.config.json
│   ├── models/
│   │   ├── user.veld
│   │   └── todo.veld
│   └── modules/
│       ├── users.veld
│       └── todos.veld
│
└── generated/
    ├── client/
    │   └── api.ts
    ├── interfaces/
    │   ├── IUsersService.ts
    │   └── ITodosService.ts
    └── types/
        ├── users.ts
        └── todos.ts
```

## Setup Instructions

### 1. Backend Setup

```bash
cd backend
npm install
npm run dev
# Server will run on http://localhost:3000
```

**Key Dependencies:**
- `express` - HTTP server framework
- `ts-node-dev` - TypeScript development server with auto-reload
- `typescript` - TypeScript compiler

**Scripts:**
- `npm run dev` - Start development server with auto-reload
- `npm run build` - Compile TypeScript to JavaScript
- `npm run start` - Run compiled JavaScript in production

### 2. Generate Veld Code

```bash
cd veld
veld generate
# Generates TypeScript types and API client
```

This creates:
- `/generated/types/` - TypeScript type definitions
- `/generated/interfaces/` - Service interfaces
- `/generated/client/api.ts` - Type-safe API client

### 3. Frontend Setup

```bash
cd frontend
npm install
npm run dev
# Frontend will run on http://localhost:5173
```

**Key Dependencies:**
- `react` - UI library
- `react-dom` - React DOM rendering
- `@tanstack/react-query` - State management for server state
- `vite` - Build tool (extremely fast)
- `typescript` - Type safety

**Scripts:**
- `npm run dev` - Start Vite dev server with HMR
- `npm run build` - Build for production
- `npm run preview` - Preview production build
- `npm run type-check` - Check TypeScript without emitting

## How It Works

### Architecture

1. **Backend (Express + TypeScript)**
   - Implements `ITodosService` and `IUsersService` interfaces
   - In-memory data store (replace with database)
   - Automatically generated routes from Veld contracts
   - Type-safe request/response handling

2. **Generated Code**
   - `api.ts` - Typed API client matching backend routes
   - Type definitions for all models (User, Todo, etc.)
   - Interface definitions matching service contracts

3. **Frontend (React + TypeScript)**
   - Uses `@tanstack/react-query` for server state management
   - Fully typed API calls via generated `api` object
   - Automatic request deduplication and caching
   - Built-in loading/error states

### API Usage Example

```typescript
// Type-safe API calls with auto-complete
import { api } from '../generated/client/api';

// List all todos
const todos = await api.Todos.ListTodos();

// Create a todo
const newTodo = await api.Todos.CreateTodo({
  title: 'Build something',
  userId: '123'
});

// Update a todo
const updated = await api.Todos.UpdateTodo(todoId, {
  completed: true
});

// Delete a todo
await api.Todos.DeleteTodo(todoId);
```

### React Query Patterns

```typescript
// Fetching data
const { data, isLoading, error } = useQuery({
  queryKey: ['todos'],
  queryFn: () => api.Todos.ListTodos(),
});

// Mutating data
const mutation = useMutation({
  mutationFn: (input) => api.Todos.CreateTodo(input),
  onSuccess: () => {
    queryClient.invalidateQueries({ queryKey: ['todos'] });
  },
});

// Trigger mutation
mutation.mutate({ title: 'New todo', userId: '123' });
```

## Key Features

### ✅ Type Safety
- Full end-to-end TypeScript types
- Generated from Veld contracts
- Auto-complete in IDE

### ✅ React Hooks
- `useQuery` for fetching
- `useMutation` for mutations
- `useQueryClient` for cache management

### ✅ Developer Experience
- Vite with HMR (Hot Module Replacement)
- Fast refresh for React components
- TypeScript strict mode

### ✅ Production Ready
- CORS configured
- Error handling
- Loading states
- Retry logic
- Request deduplication

## Common Tasks

### Add a New Feature

1. Define models and actions in `veld/*.veld`
2. Run `veld generate` to create types and API client
3. Implement service in `backend/src/services/`
4. Use auto-generated hooks/functions in frontend

### Debug API Calls

Check browser DevTools Network tab:
- All requests to `http://localhost:3000`
- Full request/response bodies
- Status codes and headers

### Environment Variables

Create `.env` for Vite:
```
VITE_API_URL=http://localhost:3000
```

Access in code:
```typescript
const baseUrl = import.meta.env.VITE_API_URL;
```

## Troubleshooting

### Port Already in Use
```bash
# Change backend port
PORT=3001 npm run dev

# Change frontend port
npm run dev -- --port 5174
```

### CORS Errors
- Backend must allow origin: `http://localhost:5173`
- Check vite.config.ts proxy configuration
- Ensure backend sets proper CORS headers

### Types Not Found
```bash
# Regenerate types
cd veld
veld generate
```

### Node Modules Issues
```bash
# Clean install
rm -rf node_modules package-lock.json
npm install
```

## Production Deployment

### Backend
```bash
npm run build
npm run start
# Set environment variables for database, etc.
```

### Frontend
```bash
npm run build
# Upload dist/ to static hosting (Vercel, Netlify, etc.)
```

## Next Steps

1. Replace in-memory store with database (PostgreSQL, MongoDB)
2. Add authentication/authorization
3. Implement proper error handling and logging
4. Add E2E tests with Playwright
5. Deploy to production

## Resources

- [Veld Documentation](https://veld.dev)
- [React Query Docs](https://tanstack.com/query/latest)
- [Vite Guide](https://vitejs.dev/guide/)
- [TypeScript Handbook](https://www.typescriptlang.org/docs/)

