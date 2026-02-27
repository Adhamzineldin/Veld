package validator

import (
	"fmt"
	"strings"

	"github.com/veld-dev/veld/internal/ast"
)

// primitiveTypes is the set of built-in scalar type names.
var primitiveTypes = map[string]bool{
	"string":   true,
	"int":      true,
	"float":    true,
	"bool":     true,
	"date":     true,
	"datetime": true,
	"uuid":     true,
}

// Validate performs semantic checks on a parsed AST and returns all errors found.
func Validate(a ast.AST) []error {
	var errs []error

	// Collect enum names and check for duplicates.
	enumNames := make(map[string]bool)
	for _, en := range a.Enums {
		if enumNames[en.Name] {
			errs = append(errs, fmt.Errorf("duplicate enum name: %q", en.Name))
		}
		enumNames[en.Name] = true
		if len(en.Values) == 0 {
			errs = append(errs, fmt.Errorf("enum %q has no values", en.Name))
		}
		// Check duplicate values within an enum.
		valSet := make(map[string]bool)
		for _, v := range en.Values {
			if valSet[v] {
				errs = append(errs, fmt.Errorf("enum %q: duplicate value %q", en.Name, v))
			}
			valSet[v] = true
		}
	}

	// Collect model names and check for duplicates.
	modelNames := make(map[string]bool)
	for _, m := range a.Models {
		if modelNames[m.Name] {
			errs = append(errs, fmt.Errorf("duplicate model name: %q", m.Name))
		}
		if enumNames[m.Name] {
			errs = append(errs, fmt.Errorf("name collision: %q is defined as both model and enum", m.Name))
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
		fieldNames := make(map[string]bool)
		for _, f := range m.Fields {
			if fieldNames[f.Name] {
				errs = append(errs, fmt.Errorf("model %q: duplicate field name %q", m.Name, f.Name))
			}
			fieldNames[f.Name] = true

			baseType := f.Type
			if !primitiveTypes[baseType] && !modelNames[baseType] && !enumNames[baseType] {
				suggestion := findSuggestion(baseType, allTypeNames)
				if suggestion != "" {
					errs = append(errs, fmt.Errorf("model %q, field %q: undefined type %q\n  did you mean %q?", m.Name, f.Name, baseType, suggestion))
				} else {
					errs = append(errs, fmt.Errorf("model %q, field %q: undefined type %q", m.Name, f.Name, baseType))
				}
			}

			// Validate @default values
			if f.Default != "" {
				errs = append(errs, validateDefault(m.Name, f, enumNames, a.Enums)...)
			}
		}
	}

	// Check modules for duplicate names and validate action type references.
	moduleNames := make(map[string]bool)
	for _, mod := range a.Modules {
		if moduleNames[mod.Name] {
			errs = append(errs, fmt.Errorf("duplicate module name: %q", mod.Name))
		}
		moduleNames[mod.Name] = true

		actionNames := make(map[string]bool)
		for _, act := range mod.Actions {
			if actionNames[act.Name] {
				errs = append(errs, fmt.Errorf("module %q: duplicate action name: %q", mod.Name, act.Name))
			}
			actionNames[act.Name] = true

			if act.Input != "" && !modelNames[act.Input] {
				suggestion := findSuggestion(act.Input, allTypeNames)
				if suggestion != "" {
					errs = append(errs, fmt.Errorf("module %q, action %q: undefined input type %q\n  did you mean %q?", mod.Name, act.Name, act.Input, suggestion))
				} else {
					errs = append(errs, fmt.Errorf("module %q, action %q: undefined input type %q", mod.Name, act.Name, act.Input))
				}
			}
			if act.Output != "" && !modelNames[act.Output] && !enumNames[act.Output] && !primitiveTypes[act.Output] {
				suggestion := findSuggestion(act.Output, allTypeNames)
				if suggestion != "" {
					errs = append(errs, fmt.Errorf("module %q, action %q: undefined output type %q\n  did you mean %q?", mod.Name, act.Name, act.Output, suggestion))
				} else {
					errs = append(errs, fmt.Errorf("module %q, action %q: undefined output type %q", mod.Name, act.Name, act.Output))
				}
			}
			if act.Query != "" && !modelNames[act.Query] {
				suggestion := findSuggestion(act.Query, allTypeNames)
				if suggestion != "" {
					errs = append(errs, fmt.Errorf("module %q, action %q: undefined query type %q\n  did you mean %q?", mod.Name, act.Name, act.Query, suggestion))
				} else {
					errs = append(errs, fmt.Errorf("module %q, action %q: undefined query type %q", mod.Name, act.Name, act.Query))
				}
			}
		}
	}

	return errs
}

// validateDefault checks that a @default value is compatible with the field type.
func validateDefault(modelName string, f ast.Field, enumNames map[string]bool, enums []ast.Enum) []error {
	var errs []error
	val := f.Default

	switch f.Type {
	case "string", "date", "datetime", "uuid":
		if !strings.HasPrefix(val, "\"") {
			errs = append(errs, fmt.Errorf("model %q, field %q: @default for %s must be a string, got %s", modelName, f.Name, f.Type, val))
		}
	case "int":
		if strings.HasPrefix(val, "\"") {
			errs = append(errs, fmt.Errorf("model %q, field %q: @default for int must be a number, got %s", modelName, f.Name, val))
		}
		if strings.Contains(val, ".") {
			errs = append(errs, fmt.Errorf("model %q, field %q: @default for int must be a whole number, got %s", modelName, f.Name, val))
		}
	case "float":
		if strings.HasPrefix(val, "\"") {
			errs = append(errs, fmt.Errorf("model %q, field %q: @default for float must be a number, got %s", modelName, f.Name, val))
		}
	case "bool":
		if val != "true" && val != "false" {
			errs = append(errs, fmt.Errorf("model %q, field %q: @default for bool must be true or false, got %s", modelName, f.Name, val))
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
						errs = append(errs, fmt.Errorf("model %q, field %q: @default(%s) is not a valid value for enum %q", modelName, f.Name, val, f.Type))
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
