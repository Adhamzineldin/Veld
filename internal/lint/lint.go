// Package lint performs static analysis on a compiled Veld AST and reports
// contract quality issues. Unlike the validator (which rejects structurally
// invalid contracts), the linter reports warnings about patterns that are
// legal but likely unintentional or problematic in production.
//
// All checks are purely additive — they never block code generation.
// Callers decide whether to surface issues as warnings or hard errors.
package lint

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// ── Issue types ───────────────────────────────────────────────────────────────

// Severity classifies how serious a lint issue is.
type Severity int

const (
	Warning Severity = iota
	Error
)

func (s Severity) String() string {
	if s == Error {
		return "error"
	}
	return "warning"
}

// Issue is a single lint finding.
type Issue struct {
	Severity Severity
	Rule     string // machine-readable rule ID, e.g. "unused-model"
	Path     string // human-readable location, e.g. "Users.createUser"
	Message  string // concise description
}

func (i Issue) IsError() bool { return i.Severity == Error }

// ── Public API ────────────────────────────────────────────────────────────────

// Lint runs all checks against the AST and returns any issues found,
// sorted by severity (errors first) then by path.
func Lint(a ast.AST) []Issue {
	var issues []Issue

	issues = append(issues, checkUnusedModels(a)...)
	issues = append(issues, checkEmptyModules(a)...)
	issues = append(issues, checkDuplicateRoutes(a)...)
	issues = append(issues, checkDuplicateActionNames(a)...)
	issues = append(issues, checkEmptyModels(a)...)
	issues = append(issues, checkMissingDescriptions(a)...)

	sortIssues(issues)
	return issues
}

// HasErrors returns true if any issue in the slice has Error severity.
func HasErrors(issues []Issue) bool {
	for _, i := range issues {
		if i.IsError() {
			return true
		}
	}
	return false
}

// ── Checks ────────────────────────────────────────────────────────────────────

// checkUnusedModels finds models that are defined but never referenced as
// input, output, query, or extended-by another model.
func checkUnusedModels(a ast.AST) []Issue {
	used := make(map[string]bool)

	for _, mod := range a.Modules {
		for _, act := range mod.Actions {
			used[act.Input] = true
			used[act.Output] = true
			used[act.Query] = true
		}
	}
	for _, m := range a.Models {
		if m.Extends != "" {
			used[m.Extends] = true
		}
	}
	delete(used, "") // empty string is not a model name

	var issues []Issue
	for _, m := range a.Models {
		if !used[m.Name] {
			issues = append(issues, Issue{
				Severity: Warning,
				Rule:     "unused-model",
				Path:     m.Name,
				Message:  fmt.Sprintf("model %q is defined but never used as input, output, query, or base type", m.Name),
			})
		}
	}
	return issues
}

// checkEmptyModules finds modules with no actions.
func checkEmptyModules(a ast.AST) []Issue {
	var issues []Issue
	for _, mod := range a.Modules {
		if len(mod.Actions) == 0 {
			issues = append(issues, Issue{
				Severity: Warning,
				Rule:     "empty-module",
				Path:     mod.Name,
				Message:  fmt.Sprintf("module %q has no actions", mod.Name),
			})
		}
	}
	return issues
}

// checkEmptyModels finds models with no fields (and no parent to inherit from).
func checkEmptyModels(a ast.AST) []Issue {
	var issues []Issue
	for _, m := range a.Models {
		if len(m.Fields) == 0 && m.Extends == "" {
			issues = append(issues, Issue{
				Severity: Warning,
				Rule:     "empty-model",
				Path:     m.Name,
				Message:  fmt.Sprintf("model %q has no fields", m.Name),
			})
		}
	}
	return issues
}

// checkDuplicateRoutes finds actions across all modules that share the same
// HTTP method + resolved path, which would cause runtime route conflicts.
func checkDuplicateRoutes(a ast.AST) []Issue {
	type routeKey struct{ method, path string }
	seen := make(map[routeKey]string) // key → "Module.Action"
	var issues []Issue

	for _, mod := range a.Modules {
		for _, act := range mod.Actions {
			resolved := act.Path
			if mod.Prefix != "" {
				resolved = mod.Prefix + act.Path
			}
			key := routeKey{strings.ToUpper(act.Method), resolved}
			label := mod.Name + "." + act.Name
			if prev, exists := seen[key]; exists {
				issues = append(issues, Issue{
					Severity: Error,
					Rule:     "duplicate-route",
					Path:     label,
					Message:  fmt.Sprintf("route %s %s is already registered by %s", key.method, key.path, prev),
				})
			} else {
				seen[key] = label
			}
		}
	}
	return issues
}

// checkDuplicateActionNames finds actions within the same module that share a
// name, which would generate conflicting service interface methods.
func checkDuplicateActionNames(a ast.AST) []Issue {
	var issues []Issue
	for _, mod := range a.Modules {
		seen := make(map[string]bool, len(mod.Actions))
		for _, act := range mod.Actions {
			if seen[act.Name] {
				issues = append(issues, Issue{
					Severity: Error,
					Rule:     "duplicate-action",
					Path:     mod.Name + "." + act.Name,
					Message:  fmt.Sprintf("action %q is defined more than once in module %s", act.Name, mod.Name),
				})
			}
			seen[act.Name] = true
		}
	}
	return issues
}

// checkMissingDescriptions warns when a module or action has no description,
// since Veld uses descriptions for generated documentation and OpenAPI summaries.
func checkMissingDescriptions(a ast.AST) []Issue {
	var issues []Issue
	for _, mod := range a.Modules {
		if mod.Description == "" {
			issues = append(issues, Issue{
				Severity: Warning,
				Rule:     "missing-description",
				Path:     mod.Name,
				Message:  fmt.Sprintf("module %q has no description — add one for richer docs and OpenAPI output", mod.Name),
			})
		}
		for _, act := range mod.Actions {
			if act.Description == "" {
				issues = append(issues, Issue{
					Severity: Warning,
					Rule:     "missing-description",
					Path:     mod.Name + "." + act.Name,
					Message:  fmt.Sprintf("action %q has no description", act.Name),
				})
			}
		}
	}
	return issues
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// sortIssues sorts in place: errors before warnings, then alphabetically by path.
func sortIssues(issues []Issue) {
	// Simple insertion sort — lint lists are typically small.
	for i := 1; i < len(issues); i++ {
		for j := i; j > 0; j-- {
			a, b := issues[j-1], issues[j]
			less := false
			if a.Severity != b.Severity {
				less = a.Severity > b.Severity // Error(1) > Warning(0) → errors first
			} else {
				less = a.Path > b.Path
			}
			if less {
				issues[j-1], issues[j] = issues[j], issues[j-1]
			} else {
				break
			}
		}
	}
}
