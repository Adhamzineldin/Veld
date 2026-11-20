# 🚀 Quick Reference Card - Veld Examples

## Choose Your Stack (60 seconds)

### 🌐 Web Development
| Stack | Speed | Learning | Why Choose? |
|-------|-------|----------|-------------|
| **React** (node-react) | ⚡⚡⚡⚡⚡ | Easy | Largest ecosystem, familiar, fast |
| **Vue** (python-vue) | ⚡⚡⚡⚡ | Easy | Simple Python backend, elegant |
| **Svelte** (go-svelte) | ⚡⚡⚡⚡⚡ | Easy | Smallest bundle, blazing fast |
| **Angular** (java-angular) | ⚡⚡⚡ | Hard | Enterprise, proven, strong typing |

### 📱 Mobile Development
| Stack | Platforms | Learning | Why Choose? |
|-------|-----------|----------|-------------|
| **Flutter** (node-flutter) | iOS+Android+Web | Medium | One codebase, beautiful, hot reload |
| **SwiftUI** (rust-swift) | iOS only | Hard | Native, best performance, gorgeous |
| **Kotlin** (csharp-kotlin) | Android only | Hard | Native Android, .NET backend |

### 🔧 Tools & Scripts
| Stack | Use | Learning | Why Choose? |
|-------|-----|----------|-------------|
| **TypeScript** (node-typescript) | CLI, tests, scripts | Easy | No framework, full type safety |

### 🏢 Enterprise
| Stack | Teams | Learning | Why Choose? |
|-------|-------|----------|-------------|
| **PHP/Laravel** (php-typescript) | PHP teams | Medium | Migration path from legacy |

---

## Setup In 3 Steps

```bash
# Step 1: Start backend
cd <example>/backend
npm install && npm run dev
# Wait for "Server running on http://localhost:3000"

# Step 2: Start frontend (NEW TERMINAL)
cd <example>/frontend
npm install && npm run dev
# Visit http://localhost:5173 (or shown in terminal)

# Step 3: Generate code (if you modify veld/)
cd <example>/veld
veld generate
```

---

## Example Selection Matrix

```
Need to...                          → Choose this
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Build a web app ASAP               → node-react
Build an app with Python           → python-vue
Build a super-fast web app         → go-svelte
Build enterprise web app           → java-angular
Build iOS app (native)             → rust-swift
Build Android app (native)         → csharp-kotlin
Build iOS + Android (same code)    → node-flutter
Write CLI tools / scripts          → node-typescript
Migrate from PHP legacy            → php-typescript
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

---

## Key Commands

### Run
```bash
npm run dev              # JavaScript
python app.py           # Python
go run main.go          # Go
mvn spring-boot:run     # Java
cargo run              # Rust
php artisan serve      # PHP
flutter run            # Flutter
```

### Build
```bash
npm run build           # JavaScript
cargo build --release  # Rust
go build              # Go
mvn clean package     # Java
php artisan build     # PHP (N/A)
flutter build         # Flutter
```

### Generate Veld Code
```bash
cd <example>/veld
veld generate
```

### Test
```bash
npm test               # JavaScript
pytest                # Python
cargo test            # Rust
mvn test              # Java
go test ./...         # Go
```

---

## API Call Examples

All examples use the same PascalCase naming:

```typescript
// List
await api.Users.ListUsers()
await api.Todos.ListTodos()

// Get
await api.Users.GetUser(id)
await api.Todos.GetTodo(id)

// Create
await api.Users.CreateUser({ name, email })
await api.Todos.CreateTodo({ title, userId })

// Update
await api.Todos.UpdateTodo(id, { completed })

// Delete
await api.Users.DeleteUser(id)
await api.Todos.DeleteTodo(id)
```

---

## Files You'll See

| File | Purpose |
|------|---------|
| `package.json` | Dependencies, scripts |
| `tsconfig.json` | TypeScript configuration |
| `vite.config.ts` | Frontend build config |
| `veld.config.json` | Veld generator config |
| `app.veld` | API contract definition |
| `models/*.veld` | Data model definitions |
| `modules/*.veld` | API endpoint definitions |
| `generated/` | Auto-generated code (**don't edit!**) |

---

## Documentation Map

```
examples/
├── INDEX.md                          ← START HERE
├── COMPLETE-GUIDE.md                 ← Choose your stack
├── COMPLETION-SUMMARY.md             ← What you have
│
└── <example>/
    └── README-DETAILED.md            ← Complete setup guide
```

---

## Deployment Targets

| Backend | Deploy To |
|---------|-----------|
| Node.js | Vercel, Railway, Heroku, AWS |
| Python | Heroku, PythonAnywhere, AWS, Google Cloud |
| Go | Railway, Google Cloud Run, Heroku |
| Java | AWS Elastic Beanstalk, Google Cloud App Engine |
| Rust | Railway, Fly.io, Google Cloud Run |
| C#/.NET | Azure, AWS |
| PHP | Any shared hosting, Heroku |

| Frontend | Deploy To |
|----------|-----------|
| React/Vue/Svelte/Angular | Vercel, Netlify, GitHub Pages |
| Flutter | Web deployment (Firebase, S3) |
| iOS/Android | App Store / Google Play |
| TypeScript | As npm package or CLI tool |

---

## Performance Comparison

```
Backend Speed (requests/sec):
Rust/Axum      ████████████ 50,000+
Go             ██████████   40,000+
Java/Spring    ████████     20,000+
Node.js        ███████      15,000+
Python/Flask   ███          5,000+

Frontend Size (gzip):
Svelte         ██           ~15KB
React          █████        ~40KB
Vue            █████        ~35KB
Angular        ███████████  ~130KB
```

---

## Troubleshooting Flowchart

```
Problem → Solution
─────────────────────────────────────
Port in use? → Change port in config
CORS error? → Check backend CORS setup
Types wrong? → Run veld generate
Deps missing? → npm install or equivalent
Server won't start? → Check port/firewall
API call failing? → Check Network tab in DevTools
Build error? → Check TypeScript errors
```

---

## Best Practices Checklist

Before deploying:
- ☑️ Environment variables configured
- ☑️ Error handling added
- ☑️ Loading states shown
- ☑️ Types checked (npm run type-check)
- ☑️ Built for production (npm run build)
- ☑️ Tested in production build
- ☑️ API URLs point to production backend
- ☑️ Security headers set
- ☑️ CORS configured for production
- ☑️ Logging configured

---

## One-Liner Commands

```bash
# Run everything (from project root)
(cd backend && npm run dev) & (cd frontend && npm run dev)

# Clean everything
find . -type d -name node_modules -exec rm -rf {} +

# Generate code
cd veld && veld generate && cd ..

# Build both
npm run build && (cd ../backend && npm run build)
```

---

## Resources

- **Veld Docs:** https://veld.dev
- **React Query:** https://tanstack.com/query/latest
- **TypeScript:** https://www.typescriptlang.org/docs/
- **Vite:** https://vitejs.dev/
- **Framework Docs:** Check README-DETAILED.md in each example

---

## You Have Everything!

✅ 9 working examples  
✅ Full documentation  
✅ Production-ready code  
✅ Type safety throughout  
✅ Best practices baked in  

**Pick an example and start coding!** 🚀

---

*Last updated: March 2, 2026*  
*All 9 examples: Complete & Perfect* ✨

