// Package diff compares two Veld ASTs and classifies every change as
// breaking or non-breaking.
//
// Breaking changes are those that require client or server code updates to stay
// correct. Non-breaking (additive) changes can be deployed without touching
// existing consumers.
//
// The diff is surfaced by "veld generate" / "veld watch" whenever a previous
// lock file (.veld.lock.json) exists alongside veld.config.json.
package diff

import (
	"fmt"
	"sort"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// ── Change types ──────────────────────────────────────────────────────────────

// ChangeKind classifies the impact of a single contract change.
type ChangeKind int

const (
	// Breaking changes require consumers (clients or server) to update.
	Breaking ChangeKind = iota
	// Added changes are purely additive and safe to deploy without client updates.
	Added
)

// Change represents a single detected difference between two contract versions.
type Change struct {
	Kind    ChangeKind
	Path    string // human-readable location, e.g. "Users.createUser.input.email"
	Message string // concise description, e.g. `required field "email" removed`
}

func (c Change) IsBreaking() bool { return c.Kind == Breaking }

// ── Public API ────────────────────────────────────────────────────────────────

// Diff compares two ASTs and returns all detected changes sorted with breaking
// changes first, then additions, each group in alphabetical path order.
func Diff(old, new ast.AST) []Change {
	var changes []Change

	oldMods := modulesByName(old.Modules)
	newMods := modulesByName(new.Modules)

	// Removed modules — always breaking.
	for name := range oldMods {
		if _, exists := newMods[name]; !exists {
			changes = append(changes, Change{
				Kind:    Breaking,
				Path:    name,
				Message: fmt.Sprintf("module %q removed", name),
			})
		}
	}

	// Added modules — non-breaking.
	for name := range newMods {
		if _, exists := oldMods[name]; !exists {
			changes = append(changes, Change{
				Kind:    Added,
				Path:    name,
				Message: fmt.Sprintf("module %q added", name),
			})
		}
	}

	// Changed modules — compare action by action.
	for name, oldMod := range oldMods {
		newMod, exists := newMods[name]
		if !exists {
			continue
		}
		changes = append(changes, diffModule(old, new, oldMod, newMod)...)
	}

	sortChanges(changes)
	return changes
}

// HasBreaking returns true if any change in the slice is breaking.
func HasBreaking(changes []Change) bool {
	for _, c := range changes {
		if c.IsBreaking() {
			return true
		}
	}
	return false
}

// ── Module diff ───────────────────────────────────────────────────────────────

func diffModule(oldAST, newAST ast.AST, old, new ast.Module) []Change {
	var changes []Change
	prefix := old.Name

	// Prefix change — clients and servers both depend on the route path.
	if old.Prefix != new.Prefix {
		changes = append(changes, Change{
			Kind:    Breaking,
			Path:    prefix,
			Message: fmt.Sprintf("route prefix changed from %q to %q", old.Prefix, new.Prefix),
		})
	}

	oldActs := actionsByName(old.Actions)
	newActs := actionsByName(new.Actions)

	// Removed actions.
	for name := range oldActs {
		if _, exists := newActs[name]; !exists {
			changes = append(changes, Change{
				Kind:    Breaking,
				Path:    prefix + "." + name,
				Message: fmt.Sprintf("action %q removed", name),
			})
		}
	}

	// Added actions.
	for name := range newActs {
		if _, exists := oldActs[name]; !exists {
			changes = append(changes, Change{
				Kind:    Added,
				Path:    prefix + "." + name,
				Message: fmt.Sprintf("action %q added", name),
			})
		}
	}

	// Changed actions.
	for name, oldAct := range oldActs {
		newAct, exists := newActs[name]
		if !exists {
			continue
		}
		changes = append(changes, diffAction(oldAST, newAST, prefix, name, oldAct, newAct)...)
	}

	return changes
}

// ── Action diff ───────────────────────────────────────────────────────────────

func diffAction(oldAST, newAST ast.AST, module, action string, old, new ast.Action) []Change {
	var changes []Change
	base := module + "." + action

	if old.Method != new.Method {
		changes = append(changes, Change{
			Kind:    Breaking,
			Path:    base,
			Message: fmt.Sprintf("HTTP method changed from %s to %s", old.Method, new.Method),
		})
	}

	if old.Path != new.Path {
		changes = append(changes, Change{
			Kind:    Breaking,
			Path:    base,
			Message: fmt.Sprintf("route path changed from %q to %q", old.Path, new.Path),
		})
	}

	if old.OutputArray != new.OutputArray {
		changes = append(changes, Change{
			Kind:    Breaking,
			Path:    base + ".output",
			Message: fmt.Sprintf("output changed: array=%v → array=%v", old.OutputArray, new.OutputArray),
		})
	}

	changes = append(changes, diffActionModel(oldAST, newAST, base+".input", old.Input, new.Input, true)...)
	changes = append(changes, diffActionModel(oldAST, newAST, base+".output", old.Output, new.Output, false)...)
	changes = append(changes, diffActionModel(oldAST, newAST, base+".query", old.Query, new.Query, true)...)

	return changes
}

// diffActionModel handles the comparison of one model slot (input/output/query).
// isInput controls the field-level breaking-change semantics.
func diffActionModel(oldAST, newAST ast.AST, path, oldName, newName string, isInput bool) []Change {
	switch {
	case oldName == newName && oldName == "":
		return nil // both absent — no change

	case oldName == "" && newName != "":
		if isInput {
			// Adding a required input model means clients must now provide a body.
			return []Change{{Kind: Breaking, Path: path, Message: fmt.Sprintf("input model added (%s) — clients must now send a request body", newName)}}
		}
		return []Change{{Kind: Added, Path: path, Message: fmt.Sprintf("output model added (%s)", newName)}}

	case oldName != "" && newName == "":
		return []Change{{Kind: Breaking, Path: path, Message: fmt.Sprintf("model removed (was %s)", oldName)}}

	case oldName != newName:
		return []Change{{Kind: Breaking, Path: path, Message: fmt.Sprintf("model changed from %s to %s", oldName, newName)}}

	default:
		// Same model name — diff field-by-field.
		return diffModelFields(oldAST, newAST, path, oldName, isInput)
	}
}

// ── Model field diff ──────────────────────────────────────────────────────────

// diffModelFields compares the fields of a model used in two ASTs.
//
// Input semantics (isInput=true):
//   - Required field added   → Breaking (clients must now send it)
//   - Any field removed      → Breaking (server contract no longer accepts it)
//   - Optional field added   → Non-breaking
//   - Field type changed     → Breaking
//   - Optional→Required      → Breaking
//
// Output semantics (isInput=false):
//   - Required field removed → Breaking (clients may read it; now undefined)
//   - Any field added        → Non-breaking (clients ignore unknown fields)
//   - Field type changed     → Breaking
//   - Required→Optional      → Breaking (clients may assume it's always present)
func diffModelFields(oldAST, newAST ast.AST, path, modelName string, isInput bool) []Change {
	oldModel := findModel(oldAST, modelName)
	newModel := findModel(newAST, modelName)
	if oldModel == nil || newModel == nil {
		return nil
	}

	var changes []Change
	oldFields := fieldsByName(allFields(*oldModel, oldAST))
	newFields := fieldsByName(allFields(*newModel, newAST))

	// Removed fields.
	for name, oldF := range oldFields {
		if _, exists := newFields[name]; !exists {
			if isInput {
				changes = append(changes, Change{
					Kind:    Breaking,
					Path:    path + "." + name,
					Message: fmt.Sprintf("field %q removed from input model %s", name, modelName),
				})
			} else if !oldF.Optional {
				changes = append(changes, Change{
					Kind:    Breaking,
					Path:    path + "." + name,
					Message: fmt.Sprintf("required field %q removed from output model %s", name, modelName),
				})
			}
			// Optional field removed from output → non-breaking (skip silently).
		}
	}

	// Added fields.
	for name, newF := range newFields {
		if _, exists := oldFields[name]; !exists {
			if isInput && !newF.Optional {
				changes = append(changes, Change{
					Kind:    Breaking,
					Path:    path + "." + name,
					Message: fmt.Sprintf("required field %q added to input model %s — clients must now send it", name, modelName),
				})
			} else {
				changes = append(changes, Change{
					Kind:    Added,
					Path:    path + "." + name,
					Message: fmt.Sprintf("field %q added to model %s", name, modelName),
				})
			}
		}
	}

	// Modified fields.
	for name, oldF := range oldFields {
		newF, exists := newFields[name]
		if !exists {
			continue
		}
		if typeChanged(oldF, newF) {
			changes = append(changes, Change{
				Kind:    Breaking,
				Path:    path + "." + name,
				Message: fmt.Sprintf("field %q type changed in model %s (%s → %s)", name, modelName, describeType(oldF), describeType(newF)),
			})
		}
		if oldF.Optional != newF.Optional {
			if isInput && !newF.Optional {
				changes = append(changes, Change{
					Kind:    Breaking,
					Path:    path + "." + name,
					Message: fmt.Sprintf("field %q became required in input model %s", name, modelName),
				})
			} else if !isInput && newF.Optional {
				changes = append(changes, Change{
					Kind:    Breaking,
					Path:    path + "." + name,
					Message: fmt.Sprintf("field %q became optional in output model %s — clients may have relied on it always being present", name, modelName),
				})
			}
		}
	}

	return changes
}

// ── Helpers ───────────────────────────────────────────────────────────────────

func modulesByName(mods []ast.Module) map[string]ast.Module {
	m := make(map[string]ast.Module, len(mods))
	for _, mod := range mods {
		m[mod.Name] = mod
	}
	return m
}

func actionsByName(acts []ast.Action) map[string]ast.Action {
	m := make(map[string]ast.Action, len(acts))
	for _, a := range acts {
		m[a.Name] = a
	}
	return m
}

func fieldsByName(fields []ast.Field) map[string]ast.Field {
	m := make(map[string]ast.Field, len(fields))
	for _, f := range fields {
		m[f.Name] = f
	}
	return m
}

func findModel(a ast.AST, name string) *ast.Model {
	for i := range a.Models {
		if a.Models[i].Name == name {
			return &a.Models[i]
		}
	}
	return nil
}

// allFields returns a model's own fields plus all inherited fields, deduplicated
// (child field wins on name collision — mirrors how inheritance works at runtime).
func allFields(model ast.Model, a ast.AST) []ast.Field {
	if model.Extends == "" {
		return model.Fields
	}
	parent := findModel(a, model.Extends)
	if parent == nil {
		return model.Fields
	}
	inherited := allFields(*parent, a)
	seen := make(map[string]bool, len(model.Fields))
	for _, f := range model.Fields {
		seen[f.Name] = true
	}
	out := make([]ast.Field, 0, len(inherited)+len(model.Fields))
	for _, f := range inherited {
		if !seen[f.Name] {
			out = append(out, f)
		}
	}
	return append(out, model.Fields...)
}

func typeChanged(a, b ast.Field) bool {
	return a.Type != b.Type ||
		a.IsArray != b.IsArray ||
		a.IsMap != b.IsMap ||
		a.MapValueType != b.MapValueType
}

func describeType(f ast.Field) string {
	switch {
	case f.IsMap:
		return fmt.Sprintf("Map<string,%s>", f.MapValueType)
	case f.IsArray:
		return f.Type + "[]"
	default:
		return f.Type
	}
}

// sortChanges sorts in place: breaking first (alphabetical by path), then added.
func sortChanges(changes []Change) {
	sort.SliceStable(changes, func(i, j int) bool {
		ki, kj := changes[i].Kind, changes[j].Kind
		if ki != kj {
			return ki < kj // Breaking (0) before Added (1)
		}
		return changes[i].Path < changes[j].Path
	})
}
