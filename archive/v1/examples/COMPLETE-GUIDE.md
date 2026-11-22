# 🚀 Veld Examples - Complete Production-Ready Guide

This is your **complete reference guide** for all 9 Veld example implementations. Each example is production-ready with full setup instructions, best practices, and detailed documentation.

## 📊 Quick Comparison Matrix

| Stack | Backend | Frontend | Use Case | Difficulty | Performance |
|-------|---------|----------|----------|------------|-------------|
| **node-react** | Node/Express | React 18 | Modern web apps | ⭐⭐ | ⭐⭐⭐⭐ |
| **python-vue** | Python/Flask | Vue 3 | Data-driven apps | ⭐⭐⭐ | ⭐⭐⭐ |
| **go-svelte** | Go | Svelte | Lightweight, fast | ⭐⭐ | ⭐⭐⭐⭐⭐ |
| **java-angular** | Java/Spring | Angular | Enterprise apps | ⭐⭐⭐⭐ | ⭐⭐⭐ |
| **rust-swift** | Rust/Axum | SwiftUI | iOS + perf | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| **node-flutter** | Node/Express | Flutter | Cross-platform | ⭐⭐⭐ | ⭐⭐⭐⭐ |
| **csharp-kotlin** | C#/.NET | Kotlin | Android + .NET | ⭐⭐⭐ | ⭐⭐⭐⭐ |
| **node-typescript** | Node/Express | TypeScript | CLI/Tests/Scripts | ⭐ | ⭐⭐⭐⭐ |
| **php-typescript** | PHP/Laravel | TypeScript | Legacy + Modern | ⭐⭐⭐ | ⭐⭐⭐ |

## 🎯 Choose Your Stack

### I want to build **web applications** 🌐

#### Best Choice: **node-react**
```bash
cd node-react/
# Backend: Express.js (Node.js)
# Frontend: React 18 with React Query
# Why: Largest ecosystem, easiest hiring, fast development
```

**Setup:** 5 minutes  
**Files:** Backend [8 files], Frontend [12 files]  
**Detailed Guide:** `node-react/README-DETAILED.md`

#### Alternative 1: **python-vue**
```bash
cd python-vue/
# Backend: Flask (Python)
# Frontend: Vue 3 Composition API
# Why: Great for data science integration, simpler Python
```

#### Alternative 2: **go-svelte**
```bash
cd go-svelte/
# Backend: Go with Axum-like patterns
# Frontend: Svelte (smallest JS footprint)
# Why: Lightning-fast performance, minimal JavaScript
```

#### Enterprise Choice: **java-angular**
```bash
cd java-angular/
# Backend: Spring Boot (Java)
# Frontend: Angular with RxJS
# Why: Enterprise patterns, strong typing, proven stability
```

---

### I want to build **mobile applications** 📱

#### Best Cross-Platform: **node-flutter**
```bash
cd node-flutter/
# Backend: Express.js (Node.js)
# Frontend: Flutter (iOS + Android + Web)
# Why: Single codebase, beautiful UI, hot reload
```

**Setup:** 10 minutes  
**Supports:** iOS, Android, Web, macOS, Windows, Linux  
**Detailed Guide:** `node-flutter/README-DETAILED.md`

#### Native iOS: **rust-swift**
```bash
cd rust-swift/
# Backend: Rust/Axum
# Frontend: SwiftUI (native iOS)
# Why: Best iOS performance, memory-safe backend
```

#### Native Android: **csharp-kotlin**
```bash
cd csharp-kotlin/
# Backend: C#/.NET
# Frontend: Kotlin (native Android)
# Why: .NET teams + native Android
```

---

### I want **scripts and CLI tools** 📝

#### Best: **node-typescript**
```bash
cd node-typescript/
# Backend: Express.js
# Frontend: Vanilla TypeScript (no framework)
# Why: Use Veld types in scripts, CLI tools, integration
```

**Perfect for:**
- Testing backend APIs
- Batch processing
- Data migration scripts
- Integration testing
- Command-line tools

**Detailed Guide:** `node-typescript/README-DETAILED.md`

---

### I want to use **my existing technology** 🛠️

#### Already using **Python?** 
→ **python-vue** - Flask + Vue 3

#### Already using **PHP?**
→ **php-typescript** - Laravel + Modern TypeScript frontend

#### Already using **Java?**
→ **java-angular** - Spring Boot + Angular

#### Already using **C#/.NET?**
→ **csharp-kotlin** - ASP.NET Core + Kotlin

#### Already using **Rust?**
→ **rust-swift** - Axum + SwiftUI

---

## 📋 Universal Setup Flow

### Every example follows this pattern:

```
1. Read README-DETAILED.md in that example
2. Install backend dependencies
3. Run veld generate
4. Install frontend dependencies
5. Start both servers
6. Test in browser/device
7. Modify and see types update automatically
```

### Terminal Cheat Sheet

**React Example (quickest):**
```bash
# Terminal 1
cd node-react/backend && npm install && npm run dev

# Terminal 2
cd node-react/frontend && npm install && npm run dev
# Visit http://localhost:5173
```

**Flutter Example (cross-platform):**
```bash
# Terminal 1
cd node-flutter/backend && npm install && npm run dev

# Terminal 2
cd node-flutter/frontend && flutter pub get && flutter run
```

**Python + Vue (data-focused):**
```bash
# Terminal 1
cd python-vue/backend && pip install -r requirements.txt && python app.py

# Terminal 2
cd python-vue/frontend && npm install && npm run dev
```

---

## 🔥 Key Features of Each Stack

### node-react
```
✅ React 18 with hooks
✅ React Query for state management
✅ Vite for lightning-fast HMR
✅ CSS Modules for styling
✅ Full TypeScript support
✅ Express backend with CORS
✅ Production-ready setup
✅ Error handling + loading states
✅ 5-minute quick start
```
📚 **Guide:** `node-react/README-DETAILED.md`

### python-vue
```
✅ Python type hints
✅ Vue 3 Composition API
✅ Reactive data binding
✅ Flask/FastAPI patterns
✅ Easy to learn
✅ Great for data science
✅ Built-in devtools
✅ <script setup> syntax
✅ Minimal boilerplate
```
📚 **Guide:** `python-vue/README-DETAILED.md`

### go-svelte
```
✅ Go simplicity and performance
✅ Svelte compiler magic
✅ No virtual DOM overhead
✅ Reactive stores
✅ Fastest development
✅ Smallest bundle size
✅ Built-in dev server
✅ Easy to understand
✅ Lightning-fast HMR
```
📚 **Guide:** `go-svelte/README-DETAILED.md`

### java-angular
```
✅ Spring Boot framework
✅ Angular modern syntax
✅ Dependency injection
✅ Observable patterns
✅ Enterprise-grade
✅ Strong typing
✅ Maven/Gradle builds
✅ Mature ecosystem
✅ Proven at scale
```
📚 **Guide:** `java-angular/README-DETAILED.md`

### rust-swift
```
✅ Rust memory safety
✅ Axum async runtime
✅ SwiftUI native iOS
✅ Type system benefits
✅ Zero-cost abstractions
✅ Concurrent handling
✅ App Store ready
✅ High performance
✅ Beautiful iOS UI
```
📚 **Guide:** `rust-swift/README-DETAILED.md`

### node-flutter
```
✅ Single codebase (5 platforms)
✅ Hot reload development
✅ Material Design UI
✅ Native performance
✅ Easy state management
✅ Rich widget library
✅ Code sharing
✅ Beautiful animations
✅ Cross-platform testing
```
📚 **Guide:** `node-flutter/README-DETAILED.md`

### csharp-kotlin
```
✅ C# modern features
✅ .NET ecosystem
✅ Kotlin expressiveness
✅ Android native
✅ Coroutines
✅ LINQ queries
✅ ViewModel pattern
✅ Record types
✅ Smart casts
```
📚 **Guide:** `csharp-kotlin/README-DETAILED.md`

### node-typescript
```
✅ Zero framework overhead
✅ Pure TypeScript
✅ Veld types in scripts
✅ CLI tools
✅ Testing scripts
✅ Small bundle size
✅ No dependencies needed
✅ Run with tsx
✅ Build with esbuild
```
📚 **Guide:** `node-typescript/README-DETAILED.md`

### php-typescript
```
✅ Laravel framework
✅ Eloquent ORM
✅ PHP 8.2 features
✅ Modern TypeScript frontend
✅ Database migrations
✅ Blade templates
✅ Artisan CLI
✅ Queue system
✅ Shared hosting support
```
📚 **Guide:** `php-typescript/README-DETAILED.md`

---

## 🎓 Learning Path

### Beginner (Start Here)
1. **node-react** - Understand full-stack TypeScript
2. **python-vue** - Learn different backend language
3. **node-typescript** - Understand API without UI framework

### Intermediate
1. **go-svelte** - Learn performance-first approach
2. **node-flutter** - Build cross-platform mobile
3. **java-angular** - Enterprise patterns

### Advanced
1. **rust-swift** - Memory safety + native iOS
2. **csharp-kotlin** - .NET ecosystem + Android
3. **php-typescript** - Legacy + modern integration

---

## 🔧 Common Tasks Across All Examples

### 1. Run Backend & Frontend

**The universal pattern:**
```bash
# Terminal 1 - Backend
cd <example>/backend
<install> && <run>

# Terminal 2 - Frontend
cd <example>/frontend
<install> && <run>
```

### 2. Generate Code from Veld Contract

```bash
cd veld/
veld generate
# Creates:
# - /generated/client/api.*
# - /generated/types/
# - /generated/interfaces/
```

### 3. Add a New Endpoint

1. Edit `veld/modules/todos.veld` or `veld/modules/users.veld`
2. Run `veld generate`
3. Implement in backend service
4. Use auto-generated types in frontend

### 4. Deploy to Production

**Backend:**
- Compile/build for your language
- Set environment variables
- Deploy to hosting platform

**Frontend:**
- Build for production (`npm run build`, `flutter build`, etc.)
- Upload `dist/` or build artifacts
- Configure API URL for production backend

### 5. Add Authentication

1. Add auth endpoint to Veld contract
2. Generate code
3. Implement in backend service
4. Store token in frontend (localStorage, Keychain, etc.)
5. Send token in API requests

---

## 📊 Performance Benchmarks

**Backend Concurrency** (requests/second):
```
Rust/Axum:      ⭐⭐⭐⭐⭐ 50,000+
Go:             ⭐⭐⭐⭐⭐ 40,000+
Java/Spring:    ⭐⭐⭐⭐  20,000+
Node.js/Express: ⭐⭐⭐⭐  15,000+
Python/Flask:   ⭐⭐⭐    5,000+
PHP/Laravel:    ⭐⭐⭐    8,000+
```

**Frontend Bundle Size** (minified + gzipped):
```
Svelte:         ⭐⭐⭐⭐⭐ ~15KB
React:          ⭐⭐⭐⭐  ~40KB
Vue:            ⭐⭐⭐⭐  ~35KB
Angular:        ⭐⭐⭐    ~130KB
Flutter Web:    ⭐⭐⭐    ~200KB
```

---

## 🎯 Quick Feature Lookup

### Need **Type Safety Everywhere?**
→ **node-react**, **java-angular**, **rust-swift**

### Need **Smallest Bundle Size?**
→ **go-svelte**, **node-typescript**

### Need **Best DX (Developer Experience)?**
→ **node-react**, **python-vue**

### Need **Native Mobile Apps?**
→ **node-flutter**, **rust-swift**, **csharp-kotlin**

### Need **Enterprise Patterns?**
→ **java-angular**, **csharp-kotlin**, **php-typescript**

### Need **Fastest Backend?**
→ **rust-swift**, **go-svelte**

### Need **Python Data Science?**
→ **python-vue**

### Need **Cross-Platform Everything?**
→ **node-flutter**

---

## 📚 File Organization

Every example has identical structure:

```
<example>/
├── README.md                 # Quick overview
├── README-DETAILED.md        # Complete setup guide ✨
├── backend/                  # Backend implementation
│   ├── package.json / composer.json / Cargo.toml / pom.xml
│   ├── src/
│   └── services/
├── frontend/                 # Frontend implementation
│   ├── package.json / pubspec.yaml
│   ├── src/
│   └── public/
├── veld/                     # Type contracts
│   ├── app.veld
│   ├── veld.config.json
│   ├── models/
│   └── modules/
└── generated/                # Auto-generated code
    ├── client/
    ├── interfaces/
    └── types/
```

---

## ⚡ Pro Tips

1. **Each example is standalone** - Copy entire folder to start new project
2. **Detailed README-DETAILED.md** - Read it for production setup
3. **Generated code matches Veld contract** - Edit contract, regenerate
4. **Use environment variables** - For API URLs, keys, etc.
5. **Check browser DevTools** - Network tab shows all API calls
6. **Type errors = good sign** - TypeScript catching issues early
7. **Hot reload** - Supported by most frontend frameworks
8. **Skip package manager lock files** - for cleaner git
9. **Production builds** - Use `npm run build`, `cargo build --release`, etc.
10. **Docker ready** - All can be containerized for deployment

---

## 🚀 Next Steps

### Pick Your Stack
1. Choose from the 9 examples above
2. Read the detailed README for that example
3. Follow setup instructions step-by-step

### Run the Examples
```bash
# Backend
cd <example>/backend && <install> && <run>

# Frontend (new terminal)
cd <example>/frontend && <install> && <run>
```

### Modify & Extend
1. Change models in `veld/models/`
2. Add endpoints in `veld/modules/`
3. Run `veld generate`
4. Implement in backend
5. Use in frontend - types auto-complete!

### Deploy
1. Build backend for production
2. Build frontend for production
3. Deploy backend to your platform
4. Deploy frontend to static hosting
5. Update API URLs for production

---

## 📞 Common Issues & Solutions

| Issue | Solution |
|-------|----------|
| Port already in use | Change port in backend/frontend config |
| CORS errors | Check backend CORS configuration |
| Types not updating | Run `veld generate` again |
| Dependencies not installing | `rm -rf node_modules && npm install` |
| Vite/Dev server issues | `npm cache clean --force && npm install` |
| Database errors (PHP/Java) | Run migrations: `php artisan migrate` |
| Emulator can't connect | Use `10.0.2.2` instead of `localhost` |

---

## 🎉 You're Ready!

Pick an example, follow the guide, and start building! Each example includes:

✅ Complete backend implementation
✅ Full frontend implementation  
✅ Production-ready setup
✅ Detailed documentation
✅ Best practices for that stack
✅ Troubleshooting guide
✅ Type safety throughout

**Happy coding! 🚀**

---

**For detailed setup instructions:** Read the `README-DETAILED.md` in your chosen example directory.

**For more info on Veld:** Visit https://veld.dev

