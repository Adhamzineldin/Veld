package emitter

import (
	"fmt"
	"sync"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// EmitOptions carries config-driven options to emitters.
type EmitOptions struct {
	BaseUrl           string                // default base URL for frontend SDK (empty = env var fallback)
	DryRun            bool                  // if true, emit nothing — just validate
	Validate          bool                  // if true, emit zero-dep runtime validators and wire them into route handlers
	BackendFramework  string                // e.g. "express", "flask", "chi", "spring" — "" means "plain" (no framework)
	FrontendFramework string                // e.g. "react", "vue", "angular", "svelte" — "" means "none"
	Services          map[string]string     // module name → base URL override; nil = all modules use BaseUrl
	ServerSdk         bool                  // also emit a server-to-server typed client (generated/server-client/)
	Description       string                // project description surfaced in AGENTS.md and generated READMEs
	ConsumedServices  []ConsumedServiceInfo // services this entry depends on (for service SDK generation)
}

// ConsumedServiceInfo is the resolved representation of a consumed service,
// ready for SDK generation. Passed to emitters via EmitOptions.
type ConsumedServiceInfo struct {
	Name    string  // workspace entry name (e.g., "iam")
	AST     ast.AST // fully parsed AST of the consumed service
	BaseUrl string  // default base URL (may be empty)
}

// BaseUrlForModule returns the effective base URL for the given module name.
// Per-module Services entry takes priority over the global BaseUrl.
func (o EmitOptions) BaseUrlForModule(moduleName string) string {
	if o.Services != nil {
		if u, ok := o.Services[moduleName]; ok && u != "" {
			return u
		}
	}
	return o.BaseUrl
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
// Every backend MUST implement EmitServiceSdk — this is what generates typed
// inter-service HTTP clients when a workspace entry declares "consumes".
type BackendEmitter interface {
	Emitter
	Summarizer
	IsBackend() // marker — implement as a no-op

	// EmitServiceSdk generates typed HTTP client SDKs for each consumed service
	// in the backend's target language. Called during workspace generation when
	// a service declares "consumes" in its config.
	EmitServiceSdk(consumed []ConsumedServiceInfo, outDir string, opts EmitOptions) error
}

// FrontendEmitter marks an emitter as a frontend SDK generator.
type FrontendEmitter interface {
	Emitter
	Summarizer
	IsFrontend() // marker — implement as a no-op
}

// ToolEmitter marks an emitter as a tooling generator (CI/CD, Dockerfile, etc.).
// Tools are NOT backends — they generate auxiliary project files, not service code.
type ToolEmitter interface {
	Emitter
	Summarizer
	IsTool() // marker — implement as a no-op
}

// ── registry ──────────────────────────────────────────────────────────────────

var (
	mu        sync.RWMutex
	backends  = map[string]BackendEmitter{}
	frontends = map[string]FrontendEmitter{}
	tools     = map[string]ToolEmitter{}
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

// RegisterTool registers a tool emitter under the given name.
// Tools are auxiliary generators (CI/CD, Dockerfile, etc.) — NOT backends.
func RegisterTool(name string, e ToolEmitter) {
	mu.Lock()
	defer mu.Unlock()
	tools[name] = e
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

// GetTool returns the tool emitter registered under the given name.
func GetTool(name string) (ToolEmitter, error) {
	mu.RLock()
	defer mu.RUnlock()
	e, ok := tools[name]
	if !ok {
		return nil, fmt.Errorf("unknown tool %q (supported: %s)", name, listKeys(tools))
	}
	return e, nil
}

// GetBackendOrTool tries backend first, then tool. Used by `veld generate`
// to support both real backends and tool emitters via --backend flag.
func GetBackendOrTool(name string) (Emitter, bool, error) {
	mu.RLock()
	defer mu.RUnlock()
	if e, ok := backends[name]; ok {
		return e, true, nil // true = is a real backend
	}
	if e, ok := tools[name]; ok {
		return e, false, nil // false = is a tool
	}
	all := listKeys(backends) + ", " + listKeys(tools)
	return nil, false, fmt.Errorf("unknown backend/tool %q (supported: %s)", name, all)
}

// ListBackends returns all registered backend names (real backends only).
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

// ListTools returns all registered tool emitter names.
func ListTools() []string {
	mu.RLock()
	defer mu.RUnlock()
	return sortedKeys(tools)
}

// ListAllTargets returns all registered backend + tool names (used in CLI help).
func ListAllTargets() []string {
	mu.RLock()
	defer mu.RUnlock()
	result := sortedKeys(backends)
	result = append(result, sortedKeys(tools)...)
	return result
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
