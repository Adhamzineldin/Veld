package emitter

import (
	"fmt"
	"sync"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// EmitOptions carries config-driven options to emitters.
type EmitOptions struct {
	BaseUrl string // base URL for the frontend SDK (empty = env var fallback)
	DryRun  bool   // if true, emit nothing — just validate
}

// Emitter writes generated output files for a given AST.
type Emitter interface {
	Emit(a ast.AST, outDir string, opts EmitOptions) error
}

// Summarizer optionally returns a human-friendly summary of what was generated.
type Summarizer interface {
	// Summary returns lines describing the generated output for each module.
	Summary(modules []string) []SummaryLine
}

// SummaryLine is one row of the post-generation summary.
type SummaryLine struct {
	Dir   string // e.g. "types/"
	Files string // e.g. "auth.ts, users.ts"
}

// BackendEmitter marks an emitter as a backend code generator.
type BackendEmitter interface {
	Emitter
	Summarizer
	IsBackend() // marker — implement as a no-op
}

// FrontendEmitter marks an emitter as a frontend SDK generator.
type FrontendEmitter interface {
	Emitter
	Summarizer
	IsFrontend() // marker — implement as a no-op
}

// ── registry ──────────────────────────────────────────────────────────────────

var (
	mu        sync.RWMutex
	backends  = map[string]BackendEmitter{}
	frontends = map[string]FrontendEmitter{}
)

// RegisterBackend registers a backend emitter under the given name.
// Typically called from an emitter package's init() function.
func RegisterBackend(name string, e BackendEmitter) {
	mu.Lock()
	defer mu.Unlock()
	backends[name] = e
}

// RegisterFrontend registers a frontend emitter under the given name.
func RegisterFrontend(name string, e FrontendEmitter) {
	mu.Lock()
	defer mu.Unlock()
	frontends[name] = e
}

// GetBackend returns the backend emitter registered under the given name.
func GetBackend(name string) (BackendEmitter, error) {
	mu.RLock()
	defer mu.RUnlock()
	e, ok := backends[name]
	if !ok {
		return nil, fmt.Errorf("unknown backend %q (supported: %s)", name, listKeys(backends))
	}
	return e, nil
}

// GetFrontend returns the frontend emitter registered under the given name.
// A name of "none" returns nil, nil (skip frontend generation).
func GetFrontend(name string) (FrontendEmitter, error) {
	if name == "none" {
		return nil, nil
	}
	mu.RLock()
	defer mu.RUnlock()
	e, ok := frontends[name]
	if !ok {
		return nil, fmt.Errorf("unknown frontend %q (supported: none, %s)", name, listKeys(frontends))
	}
	return e, nil
}

// ListBackends returns all registered backend names.
func ListBackends() []string {
	mu.RLock()
	defer mu.RUnlock()
	return sortedKeys(backends)
}

// ListFrontends returns all registered frontend names.
func ListFrontends() []string {
	mu.RLock()
	defer mu.RUnlock()
	return sortedKeys(frontends)
}

func listKeys[V any](m map[string]V) string {
	keys := sortedKeys(m)
	result := ""
	for i, k := range keys {
		if i > 0 {
			result += ", "
		}
		result += k
	}
	return result
}

func sortedKeys[V any](m map[string]V) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	// Simple insertion sort — registry is tiny.
	for i := 1; i < len(keys); i++ {
		for j := i; j > 0 && keys[j] < keys[j-1]; j-- {
			keys[j], keys[j-1] = keys[j-1], keys[j]
		}
	}
	return keys
}
