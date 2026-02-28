# Veld Future Implementation Roadmap

**Status:** Active Development  
**Last Updated:** February 28, 2026  
**Version:** 1.0

---

## 📋 Executive Summary

This roadmap outlines the implementation of the **Veld Future** features as outlined in `plan.md`. The initiative focuses on expanding Veld to support additional backend languages (Go, Rust, Java, C#, PHP), creating editor plugins (VS Code, IntelliJ), and establishing package manager wrappers (npm, pip, Homebrew).

**Guiding Principles:**
- ✅ SOLID principles: Single Responsibility, Open/Closed, Liskov Substitution, Interface Segregation, Dependency Inversion
- ✅ Clean code architecture: Modular, testable, extensible
- ✅ Zero breaking changes to existing emitters (TypeScript/Python)
- ✅ Backward compatible with current CLI and generated output

---

## 🏗️ Architecture Overview

### Emitter Registry Pattern (Existing)

```
┌─────────────────────┐
│   veld generate     │
│   --backend=node    │
│   --frontend=ts     │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────────────────────────────────┐
│         Emitter Registry (sync.RWMutex)        │
│                                                 │
│  backends:  {                                   │
│    "node" → NodeEmitter                         │
│    "python" → PythonEmitter                     │
│    "go" → GoEmitter (NEW)                      │
│    "rust" → RustEmitter (NEW)                  │
│    ...                                          │
│  }                                              │
└────────────┬────────────────────────────────────┘
             │
             ▼
┌─────────────────────────────────────────────────┐
│     Language Adapter Interface (NEW)            │
│                                                 │
│  LanguageAdapter {                              │
│    TypeMapper() → type mapping rules            │
│    Conventions() → naming, import patterns      │
│    ValidatorLib() → validation framework        │
│  }                                              │
└─────────────────────────────────────────────────┘
```

### New: Extensible Language Adapter System

```
internal/emitter/
├── emitter.go (core, with LanguageMetadata)
├── lang/ (NEW)
│   ├── lang.go (interfaces)
│   ├── golang.go (Go conventions)
│   ├── rust.go (Rust conventions)
│   ├── java.go (Java conventions)
│   └── ...
├── codegen/ (NEW, shared utilities)
│   ├── writer.go (buffered code writer)
│   ├── formatter.go (indentation, comments)
│   └── imports.go (dependency resolution)
└── backend/
    ├── node/ (existing)
    ├── python/ (existing)
    ├── go/ (NEW, Phase 2)
    │   ├── go.go (registration + orchestration)
    │   ├── types.go (struct generation)
    │   ├── routes.go (handler generation)
    │   └── validation.go (validation integration)
    ├── rust/ (NEW, Phase 3)
    ├── java/ (NEW, Phase 3)
    ├── csharp/ (NEW, Phase 3)
    └── php/ (NEW, Phase 3)
```

---

## 📅 Phase Breakdown

### Phase 1: Architecture & Refactoring (2–3 weeks, HIGH priority)
**Goal:** Establish extension framework; prepare foundation for new emitters

**Status:** ✅ **COMPLETE** (Feb 28, 2026)

#### Completed Tasks:
- ✅ Create `internal/emitter/lang/lang.go` with core interfaces:
  - `LanguageAdapter` — Central interface for language-specific logic
  - `TypeGenerator` — Generate models/types in target language
  - `RouteGenerator` — Generate HTTP routes/handlers
  - `SchemaValidator` — Generate validation schemas
  - Supporting types: `LanguageMetadata`, `NamingContext`, `CommentStyle`
- ✅ Create `internal/emitter/codegen/` package:
  - `Writer` — Buffered, indented code output for any language
  - `ImportManager` — Multi-language import statement generation
  - Helper functions: `FormatCode()`, `IndentCode()`
  - Support for Go, Rust, Java, Python, C#, PHP import syntax
- ✅ Implement `GoAdapter` (`internal/emitter/lang/golang.go`):
  - Type mapping (Veld → Go types)
  - Naming conventions (PascalCase, camelCase, SCREAMING_SNAKE_CASE)
  - Struct field tags, import statements, file extensions
  - Helper functions: `toSnakeCase()`, `toCamelCase()`, `toPascalCase()`
  - Support for nullable types, type checking
- ✅ Comprehensive unit tests:
  - 28 tests for `lang/golang.go` — all passing
  - 13 tests for `codegen/` — all passing
  - 100% backward compatibility with existing emitters

#### Deliverables Verification:
- ✅ `go test ./internal/emitter/lang/... -v` → **ALL PASS**
- ✅ `go test ./internal/emitter/codegen/... -v` → **ALL PASS**
- ✅ `go test ./internal/emitter/... -v` → **ALL PASS** (no breaking changes)
- ✅ Node/Python emitters still fully functional
- ✅ Clean architecture: SOLID principles applied throughout

#### Architecture Added:
```
internal/emitter/
├── lang/ (NEW)
│   ├── lang.go (interfaces)
│   ├── lang_test.go (tests)
│   ├── golang.go (Go adapter + helpers)
│   └── golang_test.go (28 tests)
└── codegen/ (NEW)
    ├── writer.go (buffered code writer)
    ├── writer_test.go (tests)
    ├── imports.go (multi-language import manager)
    └── [imports tested in writer_test.go]
```

#### Key Design Decisions:
1. **Interface Segregation** — Separate `TypeGenerator`, `RouteGenerator`, `SchemaValidator` interfaces
2. **Language Adapter Pattern** — All languages implement same `LanguageAdapter` interface
3. **Shared Code Generation** — `Writer` and `ImportManager` work for all languages
4. **Extensibility** — Adding new languages requires only implementing adapter interfaces
5. **Testing First** — 41 unit tests ensure correctness and prevent regressions

---

---

### Phase 2: Go Emitter (3–4 weeks, HIGH priority)
**Goal:** Full Go backend with Chi/Gin support; test with testapp

#### Tasks:
- [ ] Create `internal/emitter/backend/go/` package
- [ ] Implement Go language adapter (`internal/emitter/lang/golang.go`):
  - Type mapping (veld types → Go types)
  - Go naming conventions (CamelCase, exported fields)
  - Import path conventions (`github.com/user/module`)
- [ ] Generate types/models (`go/types.go`):
  - Structs for each model
  - Interface definitions for each service
  - Proper struct tags (json, xml, etc.)
- [ ] Generate routes/handlers (`go/routes.go`):
  - Chi router setup
  - Handler functions with proper signatures
  - Middleware (logging, error handling)
  - Panic recovery
- [ ] Validation integration (`go/validation.go`):
  - Optional: `go-validator/v10` helpers
  - Or: manual validation in handlers (zero-dependency approach)
- [ ] Error handling middleware (`go/middleware.go`):
  - Catch panics → JSON error responses
  - Consistent error response format
- [ ] CLI registration:
  - Add `--backend=go` flag support
  - Test: `veld generate --backend=go`
- [ ] Testapp integration:
  - Generate Go backend from testapp schema
  - Create `testapp/go-backend/` (parallel to Node/Python)
  - Verify generated code compiles and runs

#### Framework Decision:
**Recommendation:** Start with **Chi** (minimal, idiomatic Go, fast)
- Alternative: Gin (high-performance, larger ecosystem)
- Future: Support both via adapter pattern in Phase 3

#### Deliverables:
- ✅ `veld generate --backend=go` produces working Go backend
- ✅ Full type system, routes, error handling
- ✅ Testapp generates and runs without errors
- ✅ Generated code follows Go idioms (CamelCase exports, interfaces, no panics in lib code)

#### Validation:
- [ ] `go build` succeeds on generated code
- [ ] All route handlers tested manually
- [ ] Error scenarios return proper JSON + status codes

---

### Phase 3: Additional Backends (6–8 weeks, MEDIUM priority)
**Goal:** Rust, Java/Kotlin, C#, PHP emitters complete and tested

#### 3A. Rust Emitter (Axum/Actix, 2 weeks)
- [ ] Create `internal/emitter/backend/rust/`
- [ ] Implement Rust language adapter (`internal/emitter/lang/rust.go`):
  - Type mapping (Veld → Rust types, Option<T>, Result<T, E>)
  - Serde serialization/deserialization
  - Visibility rules (pub, pub(crate))
  - Trait definitions
- [ ] Generate types/models with Serde derives
- [ ] Generate routes/handlers for Axum router
- [ ] Error handling with custom error types
- [ ] Testapp integration: `testapp/rust-backend/`

#### 3B. Java/Kotlin Emitter (Spring Boot, 2 weeks)
- [ ] Create `internal/emitter/backend/java/`
- [ ] Implement Java language adapter (`internal/emitter/lang/java.go`):
  - Type mapping (Veld → Java/Kotlin types, Optional<T>, List<T>)
  - Spring Boot annotations (@RestController, @PostMapping, etc.)
  - Dependency injection patterns
  - Package naming conventions
- [ ] Generate DTOs (data classes in Kotlin, POJOs in Java)
- [ ] Generate Spring Boot controllers
- [ ] Validation using Spring Boot Validator or custom
- [ ] Testapp integration: `testapp/java-backend/`

#### 3C. C# Emitter (ASP.NET Core, 2 weeks)
- [ ] Create `internal/emitter/backend/csharp/`
- [ ] Implement C# language adapter (`internal/emitter/lang/csharp.go`):
  - Type mapping (Veld → C# types, Nullable<T>, List<T>)
  - ASP.NET Core attributes ([HttpPost], [FromBody], etc.)
  - Async/await patterns
  - Namespace conventions
- [ ] Generate models/DTOs with attributes
- [ ] Generate API controllers
- [ ] Error handling with problem details
- [ ] Testapp integration: `testapp/csharp-backend/`

#### 3D. PHP Emitter (Laravel, 2 weeks)
- [ ] Create `internal/emitter/backend/php/`
- [ ] Implement PHP language adapter (`internal/emitter/lang/php.go`):
  - Type mapping (Veld → PHP types with docblocks)
  - Laravel conventions (Models, Controllers, Routes)
  - Namespace and use statement patterns
  - PHPDoc annotations
- [ ] Generate Eloquent models
- [ ] Generate API controllers with proper routing
- [ ] Validation using Laravel validation rules
- [ ] Testapp integration: `testapp/php-backend/`

#### Deliverables:
- ✅ `veld generate --backend=rust|java|csharp|php` all functional
- ✅ Each backend tested with testapp examples
- ✅ Generated code follows language idioms
- ✅ Framework-specific best practices applied

#### Validation:
- [ ] Compilation/interpretation succeeds for all backends
- [ ] Testapp runs without errors
- [ ] Routes respond with correct status codes

---

### Phase 4: Editor Plugins (2–3 weeks per plugin, MEDIUM priority)

#### 4A. VS Code Plugin (2 weeks)
**Separate Repo:** `veld-dev/veld-vscode`

- [ ] Set up VS Code extension project:
  - TypeScript extension template
  - `package.json` with activation events
  - Contribution points for language, grammar, snippets
- [ ] Create TextMate grammar (`syntax/veld.tmLanguage.json`):
  - Tokens: keywords (model, module, action, field, type)
  - Scope names for syntax highlighting
  - Built-in types highlighting
  - Comment syntax
- [ ] Add code snippets (`snippets/veld.json`):
  - `model` declaration template
  - `module` with actions
  - `field` with types
- [ ] Implement basic language server protocol (LSP) features:
  - Symbol provider (models, modules, actions)
  - Hover provider (type info)
  - Error diagnostics (via `veld validate`)
- [ ] Test on example `.veld` files
- [ ] Publish to VS Code Marketplace

#### 4B. IntelliJ/WebStorm Plugin (3 weeks)
**Separate Repo:** `veld-dev/veld-intellij`

- [ ] Set up IntelliJ plugin project:
  - JetBrains plugin SDK
  - `plugin.xml` configuration
- [ ] Implement lexer and parser for `.veld`:
  - Option 1: Use TextMate grammar bundle (easier)
  - Option 2: Write custom `VeldLexer` extending `FlexAdapter` (more features)
- [ ] Create syntax highlighting scheme
- [ ] Implement code inspection (run `veld validate` on save)
- [ ] Add code completion:
  - Model names in type references
  - Field names in action parameters
  - Built-in types
- [ ] Test on example `.veld` files
- [ ] Publish to JetBrains Plugin Marketplace

#### Deliverables:
- ✅ VS Code extension with syntax highlighting + snippets
- ✅ IntelliJ/WebStorm extension with syntax + inspections
- ✅ Both published to respective marketplaces
- ✅ Documentation for installation

#### Validation:
- [ ] Syntax highlighting works correctly
- [ ] No false error reports
- [ ] Code completion helpful and accurate

---

### Phase 5: Package Manager Wrappers (2–3 weeks, LOW priority)

#### Tasks:
- [ ] **npm**: `npx veld generate` — wrapper script that downloads Go binary
  - [ ] Create `npm-package-wrapper/` directory
  - [ ] `package.json` with install script
  - [ ] Script fetches latest Go binary for platform
  - [ ] Publish to npm as `@veld/cli`
- [ ] **pip**: `pip install veld` — Python wrapper for Go binary
  - [ ] Create `pip-package-wrapper/` (setuptools)
  - [ ] Build wheels with embedded Go binary
  - [ ] Publish to PyPI
- [ ] **Homebrew**: `brew install veld`
  - [ ] Create Homebrew formula (Ruby)
  - [ ] Submit to Homebrew tap
- [ ] **Go**: Already works (`go install github.com/veld-dev/veld@latest`)
  - [ ] Verify tags and releases set up correctly

#### Deliverables:
- ✅ Multi-platform package installations available
- ✅ Users can run `veld generate` from any directory
- ✅ Auto-updates via package managers

#### Validation:
- [ ] Installation works on macOS, Linux, Windows
- [ ] `veld --version` works correctly

---

## 🏛️ SOLID Principles Application

### Single Responsibility Principle (SRP)
- **Violation:** Emitter does type generation + route generation + validation
- **Fix:** Split into `TypeGenerator`, `RouteGenerator`, `SchemaValidator` interfaces; emitter orchestrates
- **Example:** `internal/emitter/lang/lang.go` defines separate interfaces

### Open/Closed Principle (OCP)
- **Violation:** Adding new language requires modifying registry
- **Fix:** Registry pattern with `RegisterBackend()` — open for extension, closed for modification
- **Example:** New Rust emitter just calls `RegisterBackend("rust", ...)` in its `init()`

### Liskov Substitution Principle (LSP)
- **Violation:** Different emitters have different interfaces (Node has routes, Python has routes differently)
- **Fix:** All language adapters implement same `LanguageAdapter` interface
- **Example:** `GoAdapter`, `RustAdapter` both satisfy `LanguageAdapter`; code uses interface, not concrete type

### Interface Segregation Principle (ISP)
- **Violation:** Emitter interface has methods for all concerns (types, routes, validation)
- **Fix:** Smaller interfaces: `TypeGenerator`, `RouteGenerator`, `SchemaValidator`
- **Example:** Language only implements interfaces it needs (PHP may not need special streaming support)

### Dependency Inversion Principle (DIP)
- **Violation:** Main emitter code calls `node.Emit()`, `python.Emit()` directly
- **Fix:** Main code depends on `Emitter` interface, concrete emitters injected at init time
- **Example:** Registry stores `Emitter` interface, not concrete struct pointers

---

## 📁 File Organization Rules

### Code Ownership
- **`internal/emitter/lang/`** — Language adapters and conventions (platform-agnostic logic)
- **`internal/emitter/codegen/`** — Code generation utilities (shared by all backends)
- **`internal/emitter/backend/{lang}/`** — Language-specific emitter implementations
- **`internal/emitter/frontend/`** — Frontend SDK generators (existing pattern maintained)

### Naming Conventions
- **Emitter structs:** `GoEmitter`, `RustEmitter`, `JavaEmitter` (Language + Emitter)
- **Language adapters:** `GoAdapter`, `RustAdapter` (Language + Adapter)
- **Type generators:** `GoTypeGenerator`, `RustTypeGenerator` (Language + concern)
- **Files:** snake_case (types.go, routes.go, validation.go)

### Testing Organization
- **Unit tests:** `*_test.go` in same package (existing Go convention)
- **Integration tests:** `testapp/` with example schemas + generated code
- **Each Phase 2+ adds:** Full testapp backend in `testapp/{lang}-backend/`

---

## 🧪 Testing Strategy

### Phase 1 (Architecture)
```bash
go test ./internal/emitter/... -v
go test ./internal/emitter/lang/... -v
go test ./internal/emitter/codegen/... -v
```
- No breaking changes to Node/Python generation

### Phase 2 (Go Emitter)
```bash
go test ./internal/emitter/backend/go/... -v
cd testapp && veld generate --backend=go
cd testapp/go-backend && go build ./... && go test ./...
```

### Phase 3+ (Additional Backends)
- Same pattern for each language
- GitHub Actions matrix test: All backends on every PR

### Integration Tests (Continuous)
```bash
# testapp/ has go-backend, rust-backend, java-backend, etc.
# CI runs each:
cd testapp/{lang}-backend
make build  # or equivalent build command
make test
```

---

## ⚠️ Risk Assessment & Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **Breaking changes to Node/Python** | Medium | High | Comprehensive backward-compat tests; no changes to existing Emitter interface |
| **Go framework (Chi vs. Gin) choice conflict** | Medium | Medium | Implement adapter pattern early; support both via flags |
| **Type mapping inconsistencies across languages** | Medium | High | Central type mapping rules in `lang.go`; unit tests per language |
| **Framework-specific quirks** | Medium | Medium | Extensive comments in generated code; per-language tutorials |
| **Marketplace approval delays (VS Code, IntelliJ)** | Low | Low | Publish to GitHub releases first; marketplace submission parallel to development |
| **Package manager binary distribution** | Low | Medium | Use existing Go release infrastructure; cross-platform builds already working |
| **Scope creep (too many languages at once)** | High | High | Strict phase sequencing; complete Go before starting Rust; mark Phase 3+ as "optional" |

---

## 📊 Timeline Estimate

| Phase | Duration | Start Date | End Date | Status |
|-------|----------|-----------|----------|--------|
| **1. Architecture** | 2–3 weeks | Feb 28, 2026 | Feb 28, 2026 | ✅ **COMPLETE** |
| **2. Go Emitter** | 3–4 weeks | Mar 1, 2026 | Mar 28, 2026 | 🔵 Next (In Planning) |
| **3. Additional Backends** | 6–8 weeks | Mar 29, 2026 | May 17, 2026 | 🔵 Planned |
| **4. Editor Plugins** | 4–6 weeks | May 18, 2026 | Jun 29, 2026 | 🔵 Planned |
| **5. Package Managers** | 2–3 weeks | Jun 30, 2026 | Jul 18, 2026 | 🔵 Planned |

**Total Remaining Duration:** ~16 weeks (~4 months)  
**Phase 1 Completion:** Ahead of schedule ✅

---

## 📚 Documentation Plan

### For Each Phase:
1. **CLAUDE.md update** — New backend features, type mappings, examples
2. **Generated README** — Per-backend quickstart (update existing template)
3. **Tutorial** — Step-by-step guide (e.g., "Building a Go API with Veld")
4. **IMPLEMENTATION_ROADMAP.md** — This file, updated weekly

### Final Deliverable:
- Main `README.md` — Feature matrix showing all supported backends
- `docs/backends/` directory with per-language guides
- `docs/plugins/` directory with editor setup instructions

---

## 🎯 Success Criteria

By end of Phase 5:
- ✅ All 5 backend languages fully supported
- ✅ All generated code compiles/runs without modification
- ✅ Type system, routes, validation consistent across backends
- ✅ Editor plugins installed and functional in VS Code + IntelliJ
- ✅ Package managers (npm, pip, Homebrew) available
- ✅ Zero breaking changes to existing Veld workflows
- ✅ SOLID principles applied throughout
- ✅ Comprehensive documentation and tutorials
- ✅ Community can extend with new languages using adapter pattern

---

## 🚀 Getting Started

**Next Step:** Proceed to Phase 1 — Architecture Setup

```bash
# 1. Create language adapter interfaces
touch internal/emitter/lang/lang.go

# 2. Create codegen utilities
mkdir -p internal/emitter/codegen
touch internal/emitter/codegen/{writer,formatter,imports}.go

# 3. Enhance emitter registry
# (modify internal/emitter/emitter.go)

# 4. Run tests
go test ./internal/emitter/...
```

See `PHASE_1_SETUP.md` for detailed implementation steps.

---

**Questions?** Open an issue or PR with discussion tag.



