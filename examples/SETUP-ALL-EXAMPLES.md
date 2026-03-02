# Veld Examples - Complete Setup Guide for All Combinations

This directory contains production-ready examples of Veld integration across different technology stacks. Each example demonstrates the same Todo/User management application using different language and framework combinations.

## 📚 Examples Overview

| Example | Backend | Frontend | Status |
|---------|---------|----------|--------|
| **node-react** | Node.js/Express + TypeScript | React + React Query | ✅ Complete |
| **csharp-kotlin** | C#/.NET | Kotlin (Android) | ✅ Complete |
| **go-svelte** | Go | Svelte | ✅ Complete |
| **java-angular** | Java/Spring Boot | Angular | ✅ Complete |
| **node-flutter** | Node.js/Express + TypeScript | Flutter (Mobile) | ✅ Complete |
| **node-typescript** | Node.js/Express + TypeScript | Vanilla TypeScript | ✅ Complete |
| **php-typescript** | PHP/Laravel | Vanilla TypeScript | ✅ Complete |
| **python-vue** | Python/Flask | Vue.js 3 | ✅ Complete |
| **rust-swift** | Rust/Axum | SwiftUI | ✅ Complete |

## 🚀 Quick Start

Each example follows the same structure:

```
example-name/
├── backend/          # Backend implementation
├── frontend/         # Frontend implementation
├── veld/            # Veld contract definitions
└── generated/       # Generated code (types, API client, etc.)
```

### Universal Setup Steps

1. **Install backend dependencies**
   ```bash
   cd backend
   npm install    # or appropriate package manager
   npm run dev    # or equivalent
   ```

2. **Generate code from Veld contract**
   ```bash
   cd veld
   veld generate
   ```

3. **Install frontend dependencies**
   ```bash
   cd frontend
   npm install    # or appropriate package manager
   npm run dev    # or equivalent
   ```

## 📋 Example Details

### 1. Node.js + React + TypeScript
**Perfect for:** Full-stack JavaScript teams, rapid development

**Setup:**
```bash
cd node-react/backend && npm install && npm run dev     # Port 3000
cd node-react/frontend && npm install && npm run dev    # Port 5173
```

**Highlights:**
- ✅ React 18 with hooks
- ✅ React Query for state management
- ✅ Vite for lightning-fast dev experience
- ✅ TypeScript strict mode
- ✅ Production-ready styling with CSS Modules
- ✅ Full documentation in README-DETAILED.md

**Key Files:**
- `frontend/src/App.tsx` - Main component with React hooks
- `frontend/src/main.tsx` - React Query setup with configuration
- `frontend/vite.config.ts` - Vite configuration with API proxy
- `backend/src/index.ts` - Express server with CORS setup

---

### 2. C# + Kotlin (Android)
**Perfect for:** .NET backend teams building Android apps

**Setup:**
```bash
cd csharp-kotlin/backend && dotnet restore && dotnet run
cd csharp-kotlin/frontend && # Build with Android Studio
```

**Highlights:**
- ✅ ASP.NET Core backend
- ✅ Kotlin coroutines for async operations
- ✅ Android ViewModel pattern
- ✅ Type-safe API client

**Key Files:**
- `backend/Services/TodosService.cs` - Service implementation
- `frontend/TodoViewModel.kt` - ViewModel with reactive streams
- `backend/Program.cs` - ASP.NET Core configuration

---

### 3. Go + Svelte
**Perfect for:** Developers wanting lightweight, fast solutions

**Setup:**
```bash
cd go-svelte/backend && go run main.go          # Port 3000
cd go-svelte/frontend && npm install && npm run dev  # Port 5173
```

**Highlights:**
- ✅ Go's simplicity and performance
- ✅ Svelte's compiler-optimized components
- ✅ Minimal JavaScript sent to browser
- ✅ Reactive variables without hooks

**Key Files:**
- `backend/main.go` - HTTP server setup
- `backend/services/todos.go` - Todo service implementation
- `frontend/TodoApp.svelte` - Svelte component with stores

---

### 4. Java + Angular
**Perfect for:** Enterprise teams with established Java infrastructure

**Setup:**
```bash
cd java-angular/backend && mvn spring-boot:run    # Port 8080
cd java-angular/frontend && npm install && npm start  # Port 4200
```

**Highlights:**
- ✅ Spring Boot framework
- ✅ Angular with modern syntax
- ✅ Dependency injection
- ✅ Enterprise patterns

**Key Files:**
- `backend/src/main/java/.../services/TodosServiceImpl.java`
- `frontend/src/app/todo.component.ts` - Angular component
- `backend/pom.xml` - Maven configuration

---

### 5. Node.js + Flutter
**Perfect for:** Building cross-platform mobile apps

**Setup:**
```bash
cd node-flutter/backend && npm install && npm run dev
cd node-flutter/frontend && flutter pub get && flutter run
```

**Highlights:**
- ✅ Flutter for iOS/Android
- ✅ Async/await patterns
- ✅ Material Design UI
- ✅ Hot reload development

**Key Files:**
- `frontend/todo_screen.dart` - Flutter widget
- `frontend/generated/client/api_client.dart` - Generated client
- `backend/src/index.ts` - Express server

---

### 6. Node.js + Vanilla TypeScript
**Perfect for:** Frameworks-free, pure TypeScript development

**Setup:**
```bash
cd node-typescript/backend && npm install && npm run dev
cd node-typescript/frontend && npm install && npm start
```

**Highlights:**
- ✅ No framework dependencies
- ✅ Vanilla TypeScript with fetch API
- ✅ Can be run with `tsx` or bundled
- ✅ Minimal setup

**Key Files:**
- `frontend/example.ts` - TypeScript example showing all API calls
- `backend/src/index.ts` - Express server setup

---

### 7. PHP + TypeScript
**Perfect for:** Legacy PHP backends with modern frontends

**Setup:**
```bash
cd php-typescript/backend && composer install && php artisan serve
cd php-typescript/frontend && npm install && npm start
```

**Highlights:**
- ✅ Laravel framework
- ✅ Modern TypeScript frontend
- ✅ Database migrations ready
- ✅ Artisan CLI commands

**Key Files:**
- `backend/app/Services/TodosService.php` - PHP service
- `backend/routes/api.php` - API routes
- `frontend/example.ts` - TypeScript client

---

### 8. Python + Vue.js
**Perfect for:** Python-first teams building modern UIs

**Setup:**
```bash
cd python-vue/backend && python -m venv venv && source venv/bin/activate && pip install -r requirements.txt && python app.py
cd python-vue/frontend && npm install && npm run dev
```

**Highlights:**
- ✅ Flask/FastAPI pattern
- ✅ Vue.js 3 Composition API
- ✅ Python type hints
- ✅ Reactive data binding

**Key Files:**
- `backend/app.py` - Flask application
- `backend/services/todos_service.py` - Python service
- `frontend/UseTodos.vue` - Vue component with `<script setup>`

---

### 9. Rust + Swift
**Perfect for:** Performance-critical backends with iOS apps

**Setup:**
```bash
cd rust-swift/backend && cargo run       # Port 3000
cd rust-swift/frontend && # Build with Xcode
```

**Highlights:**
- ✅ Axum web framework (async/await)
- ✅ Swift/SwiftUI for native iOS
- ✅ Type-safe async patterns
- ✅ Memory safety guarantees

**Key Files:**
- `backend/src/main.rs` - Axum server setup
- `backend/src/services/todos.rs` - Rust async service
- `frontend/TodoView.swift` - SwiftUI view

---

## 🔄 Common Workflow

### For Every Example

1. **Understand the Contract**
   ```bash
   cd veld/
   cat app.veld      # Main contract
   cat modules/todos.veld  # Specific module
   cat models/todo.veld    # Data models
   ```

2. **Generate Types & API Client**
   ```bash
   veld generate
   ```

3. **Start Backend**
   ```bash
   cd backend
   npm/python/go/cargo run dev
   ```

4. **Start Frontend**
   ```bash
   cd frontend
   npm/flutter/xcode run dev
   ```

5. **Test API Calls**
   - Frontend will be available at its dev server
   - Backend API at `localhost:3000` (or framework default)
   - Check browser DevTools for requests

## 🛠️ Key Concepts Across All Examples

### Veld Contract Pattern
Every example implements this same contract:

```
Modules:
- Users: ListUsers, GetUser, CreateUser, DeleteUser
- Todos: ListTodos, GetTodo, CreateTodo, UpdateTodo, DeleteTodo

Models:
- User { id, name, email }
- Todo { id, title, completed, userId }
- CreateUserInput { name, email }
- CreateTodoInput { title, userId }
- UpdateTodoInput { title?, completed? }
```

### Service Implementation Pattern
Each backend implements two services:

**UsersService:**
- `ListUsers()` → User[]
- `GetUser(id)` → User
- `CreateUser(input)` → User
- `DeleteUser(id)` → void

**TodosService:**
- `ListTodos()` → Todo[]
- `GetTodo(id)` → Todo
- `CreateTodo(input)` → Todo
- `UpdateTodo(id, input)` → Todo
- `DeleteTodo(id)` → void

### Frontend Pattern
Every frontend demonstrates:
1. **Fetching data** from backend
2. **Displaying lists** with proper loading states
3. **Adding items** with form validation
4. **Editing items** (where applicable)
5. **Deleting items** with confirmation
6. **Error handling** with user feedback

## 🚨 Troubleshooting

### Port Conflicts
```bash
# Change backend port
PORT=3001 npm run dev

# Change frontend port
npm run dev -- --port 5174
```

### CORS Issues
- Ensure backend allows frontend origin
- Check proxy configuration in frontend build tool
- Look at browser Network tab for blocked requests

### Dependency Issues
```bash
# Clean install
rm -rf node_modules package-lock.json
npm install

# Or for Python
rm -rf venv
python -m venv venv && source venv/bin/activate
pip install -r requirements.txt
```

### Generated Code Missing
```bash
cd veld
veld generate
# Check that generated/ directory is populated
```

## 📚 Additional Resources

- [Veld Official Docs](https://veld.dev)
- [Package-specific documentation](see example README-DETAILED.md)
- [Generated code structure](generated/README.md in each example)

## ✅ Checklist for Perfect Example

Each example should have:
- ✅ Working backend with all services implemented
- ✅ Complete frontend with proper UI/UX
- ✅ Full TypeScript/language support
- ✅ Error handling and loading states
- ✅ Package.json/requirements.txt with all deps
- ✅ README with setup instructions
- ✅ vite.config/webpack/build tool configured
- ✅ .env.example or similar for configuration
- ✅ Proper directory structure
- ✅ Documentation of key files

## 🎯 Next Steps

Pick your favorite combination and:

1. Read the detailed README in that example
2. Follow setup instructions exactly
3. Run both backend and frontend
4. Explore the generated code
5. Modify an endpoint and see types update
6. Build something new!

---

**Happy coding!** 🚀

