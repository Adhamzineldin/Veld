# Phase 2: Go Backend Emitter — Implementation Guide

**Duration:** 3–4 weeks  
**Start Date:** Mar 1, 2026  
**Status:** 🔵 Next (Ready to Start)  
**Priority:** 🔴 HIGH (Go is Veld's native language)

---

## Overview

Phase 2 builds on Phase 1's architecture to create a **complete Go backend code generator**. The emitter will:

- Generate typed Go structs from Veld models
- Generate HTTP route handlers using **Chi router** (idiomatic, minimal, fast)
- Integrate input validation (optional validators or manual)
- Implement proper error handling with middleware
- Support all Veld features: enums, List/Map types, model inheritance, actions

**Philosophy:** Generated code should be **production-ready**, follow Go idioms, and require **zero modifications**.

---

## Architecture: Go Emitter Structure

```
internal/emitter/backend/go/
├── go.go                 # Emitter registration + orchestration
├── go_test.go            # Integration tests
├── emitter.go            # Main Emit() implementation
├── types.go              # Model/enum/interface generation
├── routes.go             # HTTP route generation
├── handlers.go           # Request/response handling
├── validation.go         # Input validation integration
├── middleware.go         # Common middleware (errors, logging)
├── templates.go          # Code templates (router setup, main.go, etc.)
└── helpers.go            # Go-specific utilities
```

### Package Registration

```go
// internal/emitter/backend/go/go.go
package go

import "github.com/veld-dev/veld/internal/emitter"

func init() {
	emitter.RegisterBackend("go", New())
}

type GoEmitter struct{}

func New() *GoEmitter { return &GoEmitter{} }

func (e *GoEmitter) IsBackend() {}
```

---

## Detailed Tasks

### Task 1: Skeleton & Registration (Day 1)

**File:** `internal/emitter/backend/go/go.go`

```go
package go

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/veld-dev/veld/internal/ast"
	"github.com/veld-dev/veld/internal/emitter"
)

func init() {
	emitter.RegisterBackend("go", New())
}

type GoEmitter struct{}

func New() *GoEmitter { return &GoEmitter{} }

func (e *GoEmitter) IsBackend() {}

func (e *GoEmitter) Emit(a ast.AST, outDir string, opts emitter.EmitOptions) error {
	if opts.DryRun {
		fmt.Println("[DRY RUN] Go backend would generate:")
		for _, m := range a.Modules {
			fmt.Printf("  - module: %s\n", m.Name)
		}
		return nil
	}

	// Create output directory structure
	if err := os.MkdirAll(outDir, 0755); err != nil {
		return fmt.Errorf("failed to create output directory: %w", err)
	}

	// Generate types
	if err := e.generateTypes(a, outDir); err != nil {
		return fmt.Errorf("failed to generate types: %w", err)
	}

	// Generate routes
	if err := e.generateRoutes(a, outDir); err != nil {
		return fmt.Errorf("failed to generate routes: %w", err)
	}

	// Generate main server setup
	if err := e.generateServer(a, outDir); err != nil {
		return fmt.Errorf("failed to generate server: %w", err)
	}

	return nil
}

func (e *GoEmitter) Summary(modules []string) []emitter.SummaryLine {
	var lines []emitter.SummaryLine

	// Types directory
	typeFiles := make([]string, 0, len(modules)+1)
	for _, m := range modules {
		typeFiles = append(typeFiles, strings.ToLower(m)+".go")
	}
	typeFiles = append(typeFiles, "types.go") // common types
	if len(typeFiles) > 0 {
		lines = append(lines, emitter.SummaryLine{
			Dir:   "internal/models/",
			Files: strings.Join(typeFiles, ", "),
		})
	}

	// Routes directory
	routeFiles := make([]string, 0, len(modules))
	for _, m := range modules {
		routeFiles = append(routeFiles, strings.ToLower(m)+".go")
	}
	routeFiles = append(routeFiles, "routes.go") // router setup
	if len(routeFiles) > 0 {
		lines = append(lines, emitter.SummaryLine{
			Dir:   "internal/routes/",
			Files: strings.Join(routeFiles, ", "),
		})
	}

	// Middleware
	lines = append(lines, emitter.SummaryLine{
		Dir:   "internal/middleware/",
		Files: "error_handler.go, logger.go",
	})

	// Server
	lines = append(lines, emitter.SummaryLine{
		Dir:   "./",
		Files: "server.go, main.go",
	})

	return lines
}

// Placeholder implementations
func (e *GoEmitter) generateTypes(a ast.AST, outDir string) error {
	// TODO: Implement in Task 2
	return nil
}

func (e *GoEmitter) generateRoutes(a ast.AST, outDir string) error {
	// TODO: Implement in Task 3
	return nil
}

func (e *GoEmitter) generateServer(a ast.AST, outDir string) error {
	// TODO: Implement in Task 4
	return nil
}
```

**Validation:**
```bash
cd Veld
go build ./internal/emitter/backend/go
```

---

### Task 2: Type Generation (Days 2–3)

**File:** `internal/emitter/backend/go/types.go`

Generate:
- Structs from models (with JSON tags)
- Interfaces from modules
- Constants from enums

**Key Points:**
- Use `GoAdapter` from Phase 1
- Exported fields (PascalCase)
- Proper struct tags: `\`json:"fieldName"\``, `\`db:"field_name"\``
- Support model inheritance with embedding
- Handle nullable types as pointers

**Example Output:**

```go
// User model
type User struct {
	ID        int64     `json:"id" db:"id"`
	Email     string    `json:"email" db:"email"`
	Name      string    `json:"name" db:"name"`
	CreatedAt time.Time `json:"createdAt" db:"created_at"`
}

// UserService interface
type UserService interface {
	GetUser(ctx context.Context, id int64) (*User, error)
	CreateUser(ctx context.Context, user *User) error
	DeleteUser(ctx context.Context, id int64) error
}

// Role enum
const (
	RoleAdmin    = "admin"
	RoleUser     = "user"
	RoleGuest    = "guest"
)
```

---

### Task 3: Route Generation (Days 4–6)

**File:** `internal/emitter/backend/go/routes.go`

Generate:
- Chi router setup
- HTTP handlers for each action
- Request/response marshaling
- Proper status codes (201 for POST, 204 for DELETE with no body, etc.)

**Key Points:**
- Use Chi's `r.Post()`, `r.Get()`, `r.Delete()` methods
- Extract path parameters: `:userId` → `chi.URLParam(r, "userId")`
- Parse request body: `json.NewDecoder(r.Body).Decode(&req)`
- Write responses: `json.NewEncoder(w).Encode(resp)`
- Handle errors with consistent JSON format

**Example Output:**

```go
func setupRoutes(r *chi.Mux, svc *UserService) {
	r.Post("/users", createUserHandler(svc))
	r.Get("/users/{userId}", getUserHandler(svc))
	r.Delete("/users/{userId}", deleteUserHandler(svc))
}

func createUserHandler(svc *UserService) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		var req CreateUserRequest
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			w.WriteHeader(http.StatusBadRequest)
			json.NewEncoder(w).Encode(ErrorResponse{Error: err.Error()})
			return
		}

		user, err := svc.CreateUser(r.Context(), &req.User)
		if err != nil {
			w.WriteHeader(http.StatusInternalServerError)
			json.NewEncoder(w).Encode(ErrorResponse{Error: err.Error()})
			return
		}

		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusCreated)
		json.NewEncoder(w).Encode(user)
	}
}
```

---

### Task 4: Middleware & Server Setup (Days 7–8)

**Files:** `internal/emitter/backend/go/middleware.go`, `templates.go`

Generate:
- Error handling middleware
- Request logging middleware
- CORS middleware (if needed)
- Main server setup with graceful shutdown

**Key Points:**
- Panic recovery (return 500 error)
- Consistent error response format
- Request logging (method, path, status, duration)
- Chi router initialization with middleware chain

**Example Output:**

```go
// middleware/error_handler.go
func ErrorHandler(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		defer func() {
			if err := recover(); err != nil {
				w.Header().Set("Content-Type", "application/json")
				w.WriteHeader(http.StatusInternalServerError)
				json.NewEncoder(w).Encode(map[string]interface{}{
					"error": "Internal server error",
				})
			}
		}()
		next.ServeHTTP(w, r)
	})
}

// server.go
func NewServer(services Services) *http.Server {
	r := chi.NewRouter()
	r.Use(middleware.ErrorHandler)
	r.Use(middleware.Logger)

	setupRoutes(r, services)

	return &http.Server{
		Addr:         ":8080",
		Handler:      r,
		ReadTimeout:  15 * time.Second,
		WriteTimeout: 15 * time.Second,
	}
}

// main.go
func main() {
	services := NewServices() // inject dependencies
	server := NewServer(services)

	go func() {
		if err := server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			log.Fatalf("Server error: %v", err)
		}
	}()

	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, os.Interrupt, syscall.SIGTERM)
	<-sigChan

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	server.Shutdown(ctx)
}
```

---

### Task 5: Testapp Integration (Days 9–10)

**Directory:** `testapp/go-backend/`

1. **Generate** code from testapp schema:
   ```bash
   cd testapp
   veld generate --backend=go -o go-backend
   ```

2. **Verify** output:
   - `go build ./...` succeeds
   - `go test ./...` runs (even if no tests yet)
   - Manual route testing works

3. **Create** example service implementation:
   - `internal/services/auth_service.go`
   - `internal/services/food_service.go`
   - `main.go` that wires everything together

4. **Test** routes manually:
   ```bash
   cd go-backend
   go run main.go &  # Start server
   curl -X POST http://localhost:8080/auth/login -H "Content-Type: application/json" -d '{"email":"test@test.com"}'
   ```

---

## Testing Strategy

### Unit Tests (`go_test.go`)

```go
func TestEmitGeneratesValidGo(t *testing.T) {
	// Test that emitted Go code compiles
}

func TestTypeGeneration(t *testing.T) {
	// Test struct generation from models
}

func TestRouteGeneration(t *testing.T) {
	// Test handler generation from actions
}

func TestErrorHandling(t *testing.T) {
	// Test error response formatting
}
```

### Integration Tests

- Full testapp generation from schema
- Generated code compiles
- Server starts without errors
- Routes respond with correct status codes
- Request/response marshaling works

---

## SOLID Principles in Phase 2

| Principle | Application |
|-----------|--------------|
| **SRP** | `types.go` handles types, `routes.go` handles routes, `middleware.go` handles middleware |
| **OCP** | New route handlers extend without modifying existing ones |
| **LSP** | All handlers implement `http.HandlerFunc` interface consistently |
| **ISP** | Services implement small, focused interfaces (not monolithic) |
| **DIP** | Main code depends on interfaces (UserService), not concrete implementations |

---

## Deliverables Checklist

- [ ] **Skeleton** — `internal/emitter/backend/go/go.go` with `Emit()` method
- [ ] **Type generation** — Models → Go structs with proper tags
- [ ] **Route generation** — Actions → Chi handlers with correct status codes
- [ ] **Middleware** — Error handling, logging
- [ ] **Server setup** — `main.go` template with graceful shutdown
- [ ] **Testapp integration** — Generated code compiles and runs
- [ ] **Tests** — Unit + integration tests, all passing
- [ ] **Documentation** — Comments in generated code, README for testapp

---

## Success Criteria

✅ `veld generate --backend=go -o testapp/go-backend` produces working code  
✅ Generated code follows Go idioms (CamelCase exports, interfaces, no panics in lib code)  
✅ `go build ./testapp/go-backend/...` succeeds  
✅ Routes return proper JSON responses with correct status codes  
✅ Error handling integrated (invalid input → 400, server error → 500)  
✅ Zero modifications needed to generated code  
✅ All tests passing (existing + new)  

---

## Next Steps After Phase 2

- Code review against SOLID principles
- Performance testing (route generation speed)
- Documentation update (CLAUDE.md, tutorials)
- Begin Phase 3 (Rust, Java, C#, PHP emitters)


