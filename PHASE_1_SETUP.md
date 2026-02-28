# Phase 1: Architecture & Refactoring — Detailed Setup

**Duration:** 2–3 weeks  
**Goal:** Establish extension framework for new emitters

## Step 1: Create Language Adapter Package

Create `internal/emitter/lang/` directory with foundational interfaces.

### Files to Create:
1. `internal/emitter/lang/lang.go` — Core interfaces (LanguageAdapter, TypeGenerator, etc.)
2. `internal/emitter/lang/conventions.go` — Naming, formatting rules
3. `internal/emitter/codegen/writer.go` — Buffered code writer
4. `internal/emitter/codegen/formatter.go` — Indentation, imports
5. Enhance `internal/emitter/emitter.go` — Add LanguageMetadata

## Step 2: Implement Go Language Adapter

Create `internal/emitter/lang/golang.go` with Go-specific conventions.

## Step 3: Create Shared Code Generation Utilities

Implement Writer and Formatter utilities in `internal/emitter/codegen/`.

## Step 4: Validate Phase 1

- Run `go test ./internal/emitter/...`
- Verify Node/Python emitters still work
- No breaking changes

## Step 5: Document Phase 1 Completion

Update IMPLEMENTATION_ROADMAP.md with completed tasks.

---

**Next:** Follow detailed implementation in PHASE_1_IMPLEMENTATION.md

