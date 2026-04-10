package validator

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

// primitiveTypes is the set of built-in scalar type names.
var primitiveTypes = map[string]bool{
	"string":   true,
	"int":      true,
	"long":     true,
	"float":    true,
	"decimal":  true,
	"bool":     true,
	"date":     true,
	"datetime": true,
	"time":     true,
	"uuid":     true,
	"bytes":    true,
	"any":      true,
	"json":     true,
}

// loc returns a "file:line:" prefix for error context.
// If the source file is empty, it falls back to just the line number.
func loc(file string, line int) string {
	if file == "" && line == 0 {
		return ""
	}
	name := file
	if name != "" {
		name = filepath.Base(name)
	}
	if name != "" && line > 0 {
		return fmt.Sprintf("%s:%d: ", name, line)
	}
	if line > 0 {
		return fmt.Sprintf("line %d: ", line)
	}
	return ""
}

// Validate performs semantic checks on a parsed AST and returns all errors found.
func Validate(a ast.AST) []error {
	var errs []error

	// Collect enum names and check for duplicates.
	enumNames := make(map[string]bool)
	for _, en := range a.Enums {
		if enumNames[en.Name] {
			errs = append(errs, fmt.Errorf("%sduplicate enum name: %q", loc(en.SourceFile, en.Line), en.Name))
		}
		enumNames[en.Name] = true
		if len(en.Values) == 0 {
			errs = append(errs, fmt.Errorf("%senum %q has no values", loc(en.SourceFile, en.Line), en.Name))
		}
		// Check duplicate values within an enum.
		valSet := make(map[string]bool)
		for _, v := range en.Values {
			if valSet[v] {
				errs = append(errs, fmt.Errorf("%senum %q: duplicate value %q", loc(en.SourceFile, en.Line), en.Name, v))
			}
			valSet[v] = true
		}
	}

	// Validate constants groups.
	constNames := make(map[string]bool)
	for _, cg := range a.Constants {
		if constNames[cg.Name] {
			errs = append(errs, fmt.Errorf("%sduplicate constants group name: %q", loc(cg.SourceFile, cg.Line), cg.Name))
		}
		if enumNames[cg.Name] {
			errs = append(errs, fmt.Errorf("%sname collision: %q is defined as both constants and enum", loc(cg.SourceFile, cg.Line), cg.Name))
		}
		constNames[cg.Name] = true
		if len(cg.Fields) == 0 {
			errs = append(errs, fmt.Errorf("%sconstants group %q has no fields", loc(cg.SourceFile, cg.Line), cg.Name))
		}
		fieldSet := make(map[string]bool)
		for _, f := range cg.Fields {
			if fieldSet[f.Name] {
				errs = append(errs, fmt.Errorf("%sconstants %q: duplicate field %q", loc(cg.SourceFile, f.Line), cg.Name, f.Name))
			}
			fieldSet[f.Name] = true
			if err := validateConstantValue(f); err != nil {
				errs = append(errs, fmt.Errorf("%sconstants %q.%s: %w", loc(cg.SourceFile, f.Line), cg.Name, f.Name, err))
			}
		}
	}

	// Collect model names and check for duplicates.
	modelNames := make(map[string]bool)
	for _, m := range a.Models {
		if modelNames[m.Name] {
			errs = append(errs, fmt.Errorf("%sduplicate model name: %q", loc(m.SourceFile, m.Line), m.Name))
		}
		if enumNames[m.Name] {
			errs = append(errs, fmt.Errorf("%sname collision: %q is defined as both model and enum", loc(m.SourceFile, m.Line), m.Name))
		}
		if constNames[m.Name] {
			errs = append(errs, fmt.Errorf("%sname collision: %q is defined as both model and constants", loc(m.SourceFile, m.Line), m.Name))
		}
		modelNames[m.Name] = true
	}

	// All known type names (models + enums + primitives).
	allTypeNames := make([]string, 0, len(modelNames)+len(enumNames))
	for name := range modelNames {
		allTypeNames = append(allTypeNames, name)
	}
	for name := range enumNames {
		allTypeNames = append(allTypeNames, name)
	}

	// Validate model field types.
	for _, m := range a.Models {
		// Validate extends (parent must exist, no circular inheritance).
		if m.Extends != "" {
			if !modelNames[m.Extends] {
				errs = append(errs, fmt.Errorf("%smodel %q: extends unknown model %q", loc(m.SourceFile, m.Line), m.Name, m.Extends))
			} else {
				// Check for circular inheritance.
				visited := map[string]bool{m.Name: true}
				cur := m.Extends
				for cur != "" {
					if visited[cur] {
						errs = append(errs, fmt.Errorf("%smodel %q: circular inheritance detected via %q", loc(m.SourceFile, m.Line), m.Name, cur))
						break
					}
					visited[cur] = true
					// Find parent model
					found := false
					for _, pm := range a.Models {
						if pm.Name == cur {
							cur = pm.Extends
							found = true
							break
						}
					}
					if !found {
						break
					}
				}
			}
		}

		fieldNames := make(map[string]bool)
		for _, f := range m.Fields {
			if fieldNames[f.Name] {
				errs = append(errs, fmt.Errorf("%smodel %q: duplicate field name %q", loc(m.SourceFile, f.Line), m.Name, f.Name))
			}
			fieldNames[f.Name] = true

			// Map<string, V> — validate the value type
			if f.IsMap {
				vt := f.MapValueType
				if !primitiveTypes[vt] && !modelNames[vt] && !enumNames[vt] {
					errs = append(errs, fmt.Errorf("%smodel %q, field %q: undefined Map value type %q", loc(m.SourceFile, f.Line), m.Name, f.Name, vt))
				}
				continue // Map fields don't need normal type validation
			}

			// Union types — validate each member
			if len(f.UnionTypes) > 0 {
				for _, ut := range f.UnionTypes {
					if !primitiveTypes[ut] && !modelNames[ut] && !enumNames[ut] {
						// String literals in union types (e.g. "DRAFT" | "PENDING") are valid
						// They are stored as-is from the parser, not as quoted strings
						suggestion := findSuggestion(ut, allTypeNames)
						if suggestion != "" {
							errs = append(errs, fmt.Errorf("%smodel %q, field %q: undefined union member type %q (did you mean %q?)", loc(m.SourceFile, f.Line), m.Name, f.Name, ut, suggestion))
						}
						// Don't flag string literals in unions as errors — they are valid enum-like values
					}
				}
				continue // Union fields don't need normal type validation
			}

			baseType := f.Type
			if !primitiveTypes[baseType] && !modelNames[baseType] && !enumNames[baseType] {
				suggestion := findSuggestion(baseType, allTypeNames)
				if suggestion != "" {
					errs = append(errs, fmt.Errorf("%smodel %q, field %q: undefined type %q (did you mean %q?)", loc(m.SourceFile, f.Line), m.Name, f.Name, baseType, suggestion))
				} else {
					errs = append(errs, fmt.Errorf("%smodel %q, field %q: undefined type %q", loc(m.SourceFile, f.Line), m.Name, f.Name, baseType))
				}
			}

			// Validate @default values
			if f.Default != "" {
				errs = append(errs, validateDefault(m.Name, f, enumNames, a.Enums, m.SourceFile)...)
			}
		}
	}

	// Check modules for duplicate names and validate action type references.
	moduleNames := make(map[string]bool)
	for _, mod := range a.Modules {
		if moduleNames[mod.Name] {
			errs = append(errs, fmt.Errorf("%sduplicate module name: %q", loc(mod.SourceFile, mod.Line), mod.Name))
		}
		moduleNames[mod.Name] = true

		actionNames := make(map[string]bool)
		for _, act := range mod.Actions {
			if actionNames[act.Name] {
				errs = append(errs, fmt.Errorf("%smodule %q: duplicate action name: %q", loc(mod.SourceFile, act.Line), mod.Name, act.Name))
			}
			actionNames[act.Name] = true

			if act.Input != "" && !modelNames[act.Input] {
				suggestion := findSuggestion(act.Input, allTypeNames)
				if suggestion != "" {
					errs = append(errs, fmt.Errorf("%smodule %q, action %q: undefined input type %q (did you mean %q?)", loc(mod.SourceFile, act.Line), mod.Name, act.Name, act.Input, suggestion))
				} else {
					errs = append(errs, fmt.Errorf("%smodule %q, action %q: undefined input type %q", loc(mod.SourceFile, act.Line), mod.Name, act.Name, act.Input))
				}
			}
			if act.Output != "" && !modelNames[act.Output] && !enumNames[act.Output] && !primitiveTypes[act.Output] {
				suggestion := findSuggestion(act.Output, allTypeNames)
				if suggestion != "" {
					errs = append(errs, fmt.Errorf("%smodule %q, action %q: undefined output type %q (did you mean %q?)", loc(mod.SourceFile, act.Line), mod.Name, act.Name, act.Output, suggestion))
				} else {
					errs = append(errs, fmt.Errorf("%smodule %q, action %q: undefined output type %q", loc(mod.SourceFile, act.Line), mod.Name, act.Name, act.Output))
				}
			}
			if act.Query != "" && !modelNames[act.Query] {
				suggestion := findSuggestion(act.Query, allTypeNames)
				if suggestion != "" {
					errs = append(errs, fmt.Errorf("%smodule %q, action %q: undefined query type %q (did you mean %q?)", loc(mod.SourceFile, act.Line), mod.Name, act.Name, act.Query, suggestion))
				} else {
					errs = append(errs, fmt.Errorf("%smodule %q, action %q: undefined query type %q", loc(mod.SourceFile, act.Line), mod.Name, act.Name, act.Query))
				}
			}
			if act.Headers != "" && !modelNames[act.Headers] {
				suggestion := findSuggestion(act.Headers, allTypeNames)
				if suggestion != "" {
					errs = append(errs, fmt.Errorf("%smodule %q, action %q: undefined headers type %q (did you mean %q?)", loc(mod.SourceFile, act.Line), mod.Name, act.Name, act.Headers, suggestion))
				} else {
					errs = append(errs, fmt.Errorf("%smodule %q, action %q: undefined headers type %q", loc(mod.SourceFile, act.Line), mod.Name, act.Name, act.Headers))
				}
			}

			// Validate WebSocket actions
			if act.Method == "WS" {
				if act.Stream == "" {
					errs = append(errs, fmt.Errorf("%smodule %q, action %q: WS action requires stream type", loc(mod.SourceFile, act.Line), mod.Name, act.Name))
				} else if !modelNames[act.Stream] && !enumNames[act.Stream] && !primitiveTypes[act.Stream] {
					suggestion := findSuggestion(act.Stream, allTypeNames)
					if suggestion != "" {
						errs = append(errs, fmt.Errorf("%smodule %q, action %q: undefined stream type %q (did you mean %q?)", loc(mod.SourceFile, act.Line), mod.Name, act.Name, act.Stream, suggestion))
					} else {
						errs = append(errs, fmt.Errorf("%smodule %q, action %q: undefined stream type %q", loc(mod.SourceFile, act.Line), mod.Name, act.Name, act.Stream))
					}
				}
				// Validate emit type (client→server messages) — optional but must be a known type if set
				if act.Emit != "" && !modelNames[act.Emit] && !enumNames[act.Emit] && !primitiveTypes[act.Emit] {
					suggestion := findSuggestion(act.Emit, allTypeNames)
					if suggestion != "" {
						errs = append(errs, fmt.Errorf("%smodule %q, action %q: undefined emit type %q (did you mean %q?)", loc(mod.SourceFile, act.Line), mod.Name, act.Name, act.Emit, suggestion))
					} else {
						errs = append(errs, fmt.Errorf("%smodule %q, action %q: undefined emit type %q", loc(mod.SourceFile, act.Line), mod.Name, act.Name, act.Emit))
					}
				}
			} else {
				if act.Stream != "" {
					errs = append(errs, fmt.Errorf("%smodule %q, action %q: stream field is only valid for WS actions", loc(mod.SourceFile, act.Line), mod.Name, act.Name))
				}
				if act.Emit != "" {
					errs = append(errs, fmt.Errorf("%smodule %q, action %q: emit field is only valid for WS actions", loc(mod.SourceFile, act.Line), mod.Name, act.Name))
				}
			}

			// Validate middleware names (warn about common typos)
			for _, mw := range act.Middleware {
				if mw == "" {
					errs = append(errs, fmt.Errorf("%smodule %q, action %q: empty middleware name", loc(mod.SourceFile, act.Line), mod.Name, act.Name))
				}
			}
		}
	}

	// ── Cross-module route conflict detection ───────────────────────────
	// Build a map of (METHOD, normalizedPath) → first occurrence to detect
	// overlapping routes across different modules.
	type routeKey struct{ method, path string }
	routeOwners := make(map[routeKey]string) // key → "Module.Action"
	for _, mod := range a.Modules {
		for _, act := range mod.Actions {
			fullPath := act.Path
			if mod.Prefix != "" {
				fullPath = mod.Prefix + act.Path
			}
			// Normalize path params: /users/:id → /users/:param
			normalized := normalizeRoutePath(fullPath)
			key := routeKey{method: strings.ToUpper(act.Method), path: normalized}
			owner := fmt.Sprintf("%s.%s", mod.Name, act.Name)
			if existing, ok := routeOwners[key]; ok {
				errs = append(errs, fmt.Errorf(
					"%sroute conflict: %s %s in %s overlaps with %s",
					loc(mod.SourceFile, act.Line), act.Method, fullPath, owner, existing,
				))
			} else {
				routeOwners[key] = owner
			}
		}
	}

	// ── Per-file import validation ──────────────────────────────────────
	// If the loader provided a FileImports map, verify that every type
	// referenced in a file is either defined in that same file or in a file
	// it directly imports. This catches "works by accident" transitive refs.
	if a.FileImports != nil {
		errs = append(errs, validateFileImports(a)...)
	}

	return errs
}

// validateFileImports checks that each file only uses types it defines locally
// or explicitly imports. Types available only through transitive imports are flagged.
func validateFileImports(a ast.AST) []error {
	var errs []error

	// Build typeName → sourceFile map.
	typeSource := make(map[string]string) // typeName → absolute source file path
	for _, m := range a.Models {
		if m.SourceFile != "" {
			typeSource[m.Name] = m.SourceFile
		}
	}
	for _, en := range a.Enums {
		if en.SourceFile != "" {
			typeSource[en.Name] = en.SourceFile
		}
	}

	// checkTypeVisible verifies a type is accessible from the given file.
	checkTypeVisible := func(typeName, fromFile string, line int, context string) {
		if typeName == "" || primitiveTypes[typeName] {
			return
		}
		defFile, ok := typeSource[typeName]
		if !ok {
			return // type doesn't exist — caught by earlier validation
		}
		if defFile == fromFile {
			return // defined in the same file
		}
		// Check if fromFile directly imports defFile.
		for _, imp := range a.FileImports[fromFile] {
			if imp == defFile {
				return // directly imported
			}
		}
		errs = append(errs, fmt.Errorf(
			"%s%s: type %q is defined in %s but not imported",
			loc(fromFile, line), context, typeName, filepath.Base(defFile),
		))
	}

	// Validate model field type references.
	for _, m := range a.Models {
		if m.SourceFile == "" {
			continue
		}
		if m.Extends != "" {
			checkTypeVisible(m.Extends, m.SourceFile, m.Line, fmt.Sprintf("model %q", m.Name))
		}
		for _, f := range m.Fields {
			if f.IsMap {
				checkTypeVisible(f.MapValueType, m.SourceFile, f.Line, fmt.Sprintf("model %q, field %q", m.Name, f.Name))
			} else if len(f.UnionTypes) > 0 {
				for _, ut := range f.UnionTypes {
					checkTypeVisible(ut, m.SourceFile, f.Line, fmt.Sprintf("model %q, field %q", m.Name, f.Name))
				}
			} else {
				checkTypeVisible(f.Type, m.SourceFile, f.Line, fmt.Sprintf("model %q, field %q", m.Name, f.Name))
			}
		}
	}

	// Validate module action type references.
	for _, mod := range a.Modules {
		if mod.SourceFile == "" {
			continue
		}
		for _, act := range mod.Actions {
			checkTypeVisible(act.Input, mod.SourceFile, act.Line, fmt.Sprintf("module %q, action %q", mod.Name, act.Name))
			checkTypeVisible(act.Output, mod.SourceFile, act.Line, fmt.Sprintf("module %q, action %q", mod.Name, act.Name))
			checkTypeVisible(act.Query, mod.SourceFile, act.Line, fmt.Sprintf("module %q, action %q", mod.Name, act.Name))
			checkTypeVisible(act.Headers, mod.SourceFile, act.Line, fmt.Sprintf("module %q, action %q", mod.Name, act.Name))
			checkTypeVisible(act.Stream, mod.SourceFile, act.Line, fmt.Sprintf("module %q, action %q", mod.Name, act.Name))
		}
	}

	return errs
}

// validateDefault checks that a @default value is compatible with the field type.
func validateDefault(modelName string, f ast.Field, enumNames map[string]bool, enums []ast.Enum, sourceFile string) []error {
	var errs []error
	val := f.Default
	prefix := loc(sourceFile, f.Line)
	isQuoted := strings.HasPrefix(val, "\"")
	isBoolLiteral := val == "true" || val == "false"

	switch f.Type {
	case "string", "date", "datetime", "uuid", "decimal":
		if !isQuoted {
			errs = append(errs, fmt.Errorf("%smodel %q, field %q: @default for %s must be a quoted string, got %s", prefix, modelName, f.Name, f.Type, val))
		}
	case "int":
		if isQuoted {
			errs = append(errs, fmt.Errorf("%smodel %q, field %q: @default for int must be a number, got %s", prefix, modelName, f.Name, val))
		} else if isBoolLiteral {
			errs = append(errs, fmt.Errorf("%smodel %q, field %q: @default for int must be a number, got %s", prefix, modelName, f.Name, val))
		} else if strings.Contains(val, ".") {
			errs = append(errs, fmt.Errorf("%smodel %q, field %q: @default for int must be a whole number, got %s", prefix, modelName, f.Name, val))
		}
	case "float":
		if isQuoted {
			errs = append(errs, fmt.Errorf("%smodel %q, field %q: @default for float must be a number, got %s", prefix, modelName, f.Name, val))
		} else if isBoolLiteral {
			errs = append(errs, fmt.Errorf("%smodel %q, field %q: @default for float must be a number, got %s", prefix, modelName, f.Name, val))
		}
	case "bool":
		if !isBoolLiteral {
			errs = append(errs, fmt.Errorf("%smodel %q, field %q: @default for bool must be true or false, got %s", prefix, modelName, f.Name, val))
		}
	default:
		// Check enum defaults
		if enumNames[f.Type] {
			for _, en := range enums {
				if en.Name == f.Type {
					found := false
					for _, ev := range en.Values {
						if ev == val {
							found = true
							break
						}
					}
					if !found {
						errs = append(errs, fmt.Errorf("%smodel %q, field %q: @default(%s) is not a valid value for enum %q", prefix, modelName, f.Name, val, f.Type))
					}
					break
				}
			}
		}
	}
	return errs
}

// findSuggestion returns the closest matching name, or "" if nothing is close.
func findSuggestion(input string, candidates []string) string {
	inputLower := strings.ToLower(input)
	bestDist := len(input)/2 + 1 // threshold: must be within half the length
	best := ""
	for _, c := range candidates {
		d := levenshtein(inputLower, strings.ToLower(c))
		if d < bestDist {
			bestDist = d
			best = c
		}
	}
	return best
}

// levenshtein computes the edit distance between two strings.
func levenshtein(a, b string) int {
	la, lb := len(a), len(b)
	if la == 0 {
		return lb
	}
	if lb == 0 {
		return la
	}
	prev := make([]int, lb+1)
	for j := 0; j <= lb; j++ {
		prev[j] = j
	}
	for i := 1; i <= la; i++ {
		curr := make([]int, lb+1)
		curr[0] = i
		for j := 1; j <= lb; j++ {
			cost := 1
			if a[i-1] == b[j-1] {
				cost = 0
			}
			curr[j] = min3(curr[j-1]+1, prev[j]+1, prev[j-1]+cost)
		}
		prev = curr
	}
	return prev[lb]
}

func min3(a, b, c int) int {
	if a < b {
		if a < c {
			return a
		}
		return c
	}
	if b < c {
		return b
	}
	return c
}

// normalizeRoutePath replaces path parameters with a placeholder so that
// /users/:id and /users/:userId are treated as the same route.
func normalizeRoutePath(path string) string {
	parts := strings.Split(path, "/")
	for i, p := range parts {
		if strings.HasPrefix(p, ":") {
			parts[i] = ":param"
		}
	}
	return strings.Join(parts, "/")
}

// validateConstantValue checks that the value is compatible with the declared type.
func validateConstantValue(f ast.ConstantField) error {
	switch f.Type {
	case "string", "uuid", "date", "datetime", "decimal":
		// These are always stored as string literals — parser ensures they're quoted.
		return nil
	case "int":
		for i, ch := range f.Value {
			if ch == '-' && i == 0 {
				continue
			}
			if ch < '0' || ch > '9' {
				return fmt.Errorf("expected integer value, got %q", f.Value)
			}
		}
	case "float":
		hasDot := false
		for i, ch := range f.Value {
			if ch == '-' && i == 0 {
				continue
			}
			if ch == '.' {
				if hasDot {
					return fmt.Errorf("expected float value, got %q", f.Value)
				}
				hasDot = true
				continue
			}
			if ch < '0' || ch > '9' {
				return fmt.Errorf("expected float value, got %q", f.Value)
			}
		}
	case "bool":
		if f.Value != "true" && f.Value != "false" {
			return fmt.Errorf("expected true or false, got %q", f.Value)
		}
	default:
		return fmt.Errorf("unsupported constant type %q", f.Type)
	}
	return nil
}
