# 📚 Veld Examples - Complete Index & Navigation

Welcome! This directory contains **9 complete, production-ready examples** of Veld integration with different technology stacks. Each example is a fully functional full-stack application demonstrating the same Todo/User management system.

## 🗂️ Directory Structure & Files

```
examples/
├── README.md                          # (This file) Overview
├── COMPLETE-GUIDE.md                  # ✨ START HERE - Master guide for all examples
├── SETUP-ALL-EXAMPLES.md              # Quick setup overview for all stacks
│
├── node-react/
│   ├── README.md                      # Quick overview
│   ├── README-DETAILED.md             # ⭐ Complete setup guide
│   ├── backend/                       # Express.js + TypeScript
│   ├── frontend/                      # React 18 + React Query
│   └── veld/                          # Veld contracts
│
├── python-vue/
│   ├── README.md
│   ├── README-DETAILED.md             # ⭐ Complete setup guide
│   ├── backend/                       # Flask + Python
│   ├── frontend/                      # Vue 3 + Composition API
│   └── veld/
│
├── go-svelte/
│   ├── README.md
│   ├── README-DETAILED.md             # ⭐ Complete setup guide
│   ├── backend/                       # Go
│   ├── frontend/                      # Svelte
│   └── veld/
│
├── java-angular/
│   ├── README.md
│   ├── README-DETAILED.md             # ⭐ Complete setup guide
│   ├── backend/                       # Spring Boot + Java
│   ├── frontend/                      # Angular
│   └── veld/
│
├── rust-swift/
│   ├── README.md
│   ├── README-DETAILED.md             # ⭐ Complete setup guide
│   ├── backend/                       # Rust + Axum
│   ├── frontend/                      # SwiftUI (iOS)
│   └── veld/
│
├── node-flutter/
│   ├── README.md
│   ├── README-DETAILED.md             # ⭐ Complete setup guide
│   ├── backend/                       # Express.js + TypeScript
│   ├── frontend/                      # Flutter (iOS/Android/Web)
│   └── veld/
│
├── csharp-kotlin/
│   ├── README.md
│   ├── README-DETAILED.md             # ⭐ Complete setup guide
│   ├── backend/                       # C#/.NET
│   ├── frontend/                      # Kotlin (Android)
│   └── veld/
│
├── node-typescript/
│   ├── README.md
│   ├── README-DETAILED.md             # ⭐ Complete setup guide
│   ├── backend/                       # Express.js + TypeScript
│   ├── frontend/                      # Vanilla TypeScript (no framework)
│   └── veld/
│
└── php-typescript/
    ├── README.md
    ├── README-DETAILED.md             # ⭐ Complete setup guide
    ├── backend/                       # PHP + Laravel
    ├── frontend/                      # TypeScript
    └── veld/
```

## 🚀 Quick Start

### For Complete Beginners
**Start Here:** `COMPLETE-GUIDE.md`
- Explains every example
- Shows comparison matrix
- Helps you choose the right stack
- Provides learning path

### For Experienced Developers
**Jump To Your Stack:**

| Stack | Location | Read This | Time |
|-------|----------|-----------|------|
| Web (React) | `node-react/` | `README-DETAILED.md` | 5 min |
| Web (Python) | `python-vue/` | `README-DETAILED.md` | 8 min |
| Web (Go) | `go-svelte/` | `README-DETAILED.md` | 5 min |
| Web (Java) | `java-angular/` | `README-DETAILED.md` | 10 min |
| Mobile (iOS) | `rust-swift/` | `README-DETAILED.md` | 15 min |
| Mobile (Cross) | `node-flutter/` | `README-DETAILED.md` | 10 min |
| Mobile (Android) | `csharp-kotlin/` | `README-DETAILED.md` | 12 min |
| Scripts | `node-typescript/` | `README-DETAILED.md` | 5 min |
| Legacy | `php-typescript/` | `README-DETAILED.md` | 10 min |

## 📖 How to Use This Repository

### Step 1: Choose Your Stack
```
1. Read COMPLETE-GUIDE.md
2. Decide based on your tech preferences
3. Navigate to that example folder
```

### Step 2: Read the Detailed Guide
```
4. Open README-DETAILED.md in that folder
5. Follow setup instructions step-by-step
6. Copy commands exactly as shown
```

### Step 3: Run Both Servers
```
7. Terminal 1: Start backend
8. Terminal 2: Start frontend
9. Open in browser/device
```

### Step 4: Explore
```
10. Modify API in veld/ files
11. Run veld generate
12. See types update automatically
13. Implement in backend
14. Use in frontend
```

## ✨ What Makes These Examples Perfect

### ✅ Complete & Production-Ready
- Full backend implementation
- Full frontend implementation
- Proper error handling
- Loading states
- Styling included

### ✅ Thoroughly Documented
- README-DETAILED.md in each example
- Inline code comments
- Architecture explanation
- Best practices guide
- Troubleshooting section

### ✅ Type-Safe Throughout
- TypeScript where applicable
- Generated types from Veld
- IDE auto-complete
- Compile-time error checking
- Full type inference

### ✅ Modern Best Practices
- State management patterns
- Async/await everywhere
- Error handling
- Loading indicators
- User feedback

### ✅ Quick to Run
- 5-10 minute setup
- No database setup needed (in-memory)
- Auto-reload during development
- Hot reload for frontend
- Fully functional immediately

## 🎯 Example Features

### What Each Example Demonstrates

**Backend Patterns:**
- Service/Repository pattern
- Error handling
- CORS configuration
- Type-safe request handling
- In-memory data store

**Frontend Patterns:**
- Component architecture
- State management
- Loading/error states
- Form handling
- API integration

**Veld Integration:**
- Contract definition in `.veld` files
- Code generation from contracts
- Type-safe API calls
- End-to-end type safety
- Automatic API client generation

## 📋 Common Operations

### Starting an Example
```bash
cd <example>/backend
npm/python/go install  # or appropriate package manager
npm/python/go run      # or appropriate run command
```

### Generating Code
```bash
cd <example>/veld
veld generate
```

### Running Tests
Each example includes testing setup:
```bash
cd <example>
npm test              # JavaScript/TypeScript
pytest               # Python
cargo test           # Rust
```

### Building for Production
```bash
cd <example>/backend
npm run build         # JavaScript/TypeScript
cargo build --release # Rust
go build             # Go
```

## 🔍 Finding What You Need

### By Technology
- **React** → `node-react/`
- **Vue** → `python-vue/`
- **Svelte** → `go-svelte/`
- **Angular** → `java-angular/`
- **SwiftUI** → `rust-swift/`
- **Flutter** → `node-flutter/`
- **Kotlin** → `csharp-kotlin/`
- **PHP/Laravel** → `php-typescript/`
- **Vanilla TypeScript** → `node-typescript/`

### By Backend Language
- **Node.js** → `node-react/`, `node-flutter/`, `node-typescript/`
- **Python** → `python-vue/`
- **Go** → `go-svelte/`
- **Java** → `java-angular/`
- **Rust** → `rust-swift/`
- **C#** → `csharp-kotlin/`
- **PHP** → `php-typescript/`

### By Frontend Type
- **Web** → `node-react/`, `python-vue/`, `go-svelte/`, `java-angular/`, `node-typescript/`, `php-typescript/`
- **iOS** → `rust-swift/`, `csharp-kotlin/` (via Kotlin)
- **Android** → `node-flutter/`, `csharp-kotlin/`
- **Cross-Platform** → `node-flutter/`
- **CLI/Scripts** → `node-typescript/`

### By Use Case
- **Fastest Development** → `node-react/`, `python-vue/`
- **Best Performance** → `rust-swift/`, `go-svelte/`
- **Enterprise** → `java-angular/`, `csharp-kotlin/`, `php-typescript/`
- **Mobile First** → `node-flutter/`, `rust-swift/`, `csharp-kotlin/`
- **Lightweight** → `go-svelte/`, `node-typescript/`

## 💡 Learning Resources

### Within This Repository
- Each example includes inline code comments
- README-DETAILED.md explains every concept
- Troubleshooting section in each guide
- Best practices for that tech stack

### External Resources
- [Veld Official Docs](https://veld.dev)
- Framework-specific docs (links in each README)
- Type system explanations
- API pattern descriptions

## 🎓 Recommended Learning Order

### Beginners
1. Start with `COMPLETE-GUIDE.md`
2. Pick `node-react/` - most familiar stack
3. Follow README-DETAILED.md
4. Modify and experiment
5. Try another backend (python-vue)
6. Try another frontend (flutter)

### Intermediate
1. Try `go-svelte/` - new languages
2. Understand performance differences
3. Try `java-angular/` - enterprise patterns
4. Learn state management strategies
5. Explore mobile (flutter)

### Advanced
1. `rust-swift/` - Memory safety + iOS
2. `csharp-kotlin/` - .NET ecosystem
3. `php-typescript/` - Legacy integration
4. Compare performance characteristics
5. Production deployment strategies

## 🚨 Troubleshooting

**Can't find a file?**
→ Check the directory structure above

**Which README do I read?**
→ Read `README-DETAILED.md` for setup instructions

**How do I choose a stack?**
→ Read `COMPLETE-GUIDE.md` section "Choose Your Stack"

**What if my language isn't here?**
→ Any language can use Veld - these are examples

**How do I deploy?**
→ Each `README-DETAILED.md` has production section

## 🎯 Next Steps

### Immediate
1. Pick a stack from `COMPLETE-GUIDE.md`
2. Go to that folder
3. Open `README-DETAILED.md`
4. Follow setup instructions

### Short Term
5. Run backend + frontend
6. Play with the app
7. Modify veld contracts
8. Add new endpoints

### Medium Term
9. Replace in-memory store with database
10. Add authentication
11. Deploy to production
12. Build your own project

## 📞 Support

### For Each Example
See `README-DETAILED.md` → Troubleshooting section

### General Help
1. Read the guides thoroughly
2. Check browser DevTools
3. Review error messages
4. Search online for framework-specific issues
5. Visit Veld official docs

---

## 🎉 You're Ready!

This is everything you need to:
- ✅ Understand Veld
- ✅ Choose the right stack
- ✅ Run working examples
- ✅ Build your own project
- ✅ Deploy to production

**Let's get started! Pick an example and follow its README-DETAILED.md** 🚀

---

**Pro Tip:** Open multiple examples and compare them. See how the same contracts are implemented differently in different languages!

