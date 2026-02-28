package emitter

import (
	"regexp"
	"strings"

	"github.com/veld-dev/veld/internal/ast"
)

// paramRegex matches Express-style path params like :id, :userId etc.
var paramRegex = regexp.MustCompile(`:([a-zA-Z_][a-zA-Z0-9_]*)`)

// ExtractPathParams returns the parameter names from a route path like /users/:id.
func ExtractPathParams(path string) []string {
	matches := paramRegex.FindAllStringSubmatch(path, -1)
	params := make([]string, 0, len(matches))
	for _, m := range matches {
		params = append(params, m[1])
	}
	return params
}

// ToTemplateLiteral converts /users/:id to /users/${id} for JS template literals.
func ToTemplateLiteral(path string) string {
	return paramRegex.ReplaceAllString(path, `${$1}`)
}

// ToFlaskPath converts /users/:id to /users/<id> for Flask route registration.
func ToFlaskPath(path string) string {
	return paramRegex.ReplaceAllString(path, `<$1>`)
}

// ToOpenAPIPath converts /users/:id to /users/{id} for OpenAPI specs.
func ToOpenAPIPath(path string) string {
	return paramRegex.ReplaceAllString(path, `{$1}`)
}

// ToChiPath converts /users/:id to /users/{id} for Chi router.
func ToChiPath(path string) string {
	return paramRegex.ReplaceAllString(path, `{$1}`)
}

// CollectTransitiveModels returns all model names needed by a module, following
// model references in fields transitively.
func CollectTransitiveModels(a ast.AST, mod ast.Module) map[string]bool {
	byName := make(map[string]ast.Model, len(a.Models))
	for _, m := range a.Models {
		byName[m.Name] = m
	}

	used := make(map[string]bool)
	queue := []string{}
	for _, act := range mod.Actions {
		if act.Input != "" {
			queue = append(queue, act.Input)
		}
		if act.Output != "" {
			queue = append(queue, act.Output)
		}
		if act.Query != "" {
			queue = append(queue, act.Query)
		}
	}
	for len(queue) > 0 {
		name := queue[0]
		queue = queue[1:]
		if used[name] {
			continue
		}
		used[name] = true
		if m, ok := byName[name]; ok {
			// Follow extends chain
			if m.Extends != "" && !used[m.Extends] {
				queue = append(queue, m.Extends)
			}
			for _, f := range m.Fields {
				// Regular model references
				if _, isModel := byName[f.Type]; isModel && !used[f.Type] {
					queue = append(queue, f.Type)
				}
				// Map<string, V> value type references
				if f.IsMap {
					if _, isModel := byName[f.MapValueType]; isModel && !used[f.MapValueType] {
						queue = append(queue, f.MapValueType)
					}
				}
			}
		}
	}
	return used
}

// CollectUsedEnums returns the set of enum names referenced by models used in a module.
func CollectUsedEnums(a ast.AST, mod ast.Module) map[string]bool {
	enumNames := make(map[string]bool)
	for _, en := range a.Enums {
		enumNames[en.Name] = true
	}

	usedModels := CollectTransitiveModels(a, mod)
	usedEnums := make(map[string]bool)

	for _, m := range a.Models {
		if !usedModels[m.Name] {
			continue
		}
		for _, f := range m.Fields {
			if enumNames[f.Type] {
				usedEnums[f.Type] = true
			}
			if f.IsMap && enumNames[f.MapValueType] {
				usedEnums[f.MapValueType] = true
			}
		}
	}
	return usedEnums
}

// CollectUsedTypes returns the unique type names (models + enums) referenced
// directly or transitively by a module, in a stable order.
func CollectUsedTypes(a ast.AST, mod ast.Module) []string {
	seen := make(map[string]bool)
	var result []string

	enumNames := make(map[string]bool)
	for _, en := range a.Enums {
		enumNames[en.Name] = true
	}

	// Direct action references.
	for _, act := range mod.Actions {
		for _, name := range []string{act.Input, act.Output, act.Query} {
			if name != "" && !seen[name] && !IsPrimitive(name) {
				seen[name] = true
				result = append(result, name)
			}
		}
	}

	// Transitive model references.
	usedModels := CollectTransitiveModels(a, mod)
	byName := make(map[string]ast.Model, len(a.Models))
	for _, m := range a.Models {
		byName[m.Name] = m
	}
	for _, m := range a.Models {
		if !usedModels[m.Name] {
			continue
		}
		if !seen[m.Name] {
			seen[m.Name] = true
			result = append(result, m.Name)
		}
		for _, f := range m.Fields {
			base := f.Type
			if _, isModel := byName[base]; isModel && !seen[base] {
				seen[base] = true
				result = append(result, base)
			}
			if enumNames[base] && !seen[base] {
				seen[base] = true
				result = append(result, base)
			}
		}
	}

	// Enums.
	usedEnums := CollectUsedEnums(a, mod)
	for _, en := range a.Enums {
		if usedEnums[en.Name] && !seen[en.Name] {
			seen[en.Name] = true
			result = append(result, en.Name)
		}
	}

	return result
}

// IsPrimitive returns true for built-in Veld scalar types.
func IsPrimitive(t string) bool {
	switch t {
	case "string", "int", "float", "bool", "date", "datetime", "uuid":
		return true
	}
	return false
}

// CollectModuleMiddleware returns all unique middleware names used in a module,
// in order of first appearance.
func CollectModuleMiddleware(mod ast.Module) []string {
	seen := make(map[string]bool)
	var result []string
	for _, act := range mod.Actions {
		for _, mw := range act.Middleware {
			if !seen[mw] {
				seen[mw] = true
				result = append(result, mw)
			}
		}
	}
	return result
}

// ToSnakeCase converts a camelCase or PascalCase name to snake_case.
func ToSnakeCase(s string) string {
	var result strings.Builder
	for i, r := range s {
		if r >= 'A' && r <= 'Z' {
			if i > 0 {
				result.WriteByte('_')
			}
			result.WriteRune(r + 32) // to lower
		} else {
			result.WriteRune(r)
		}
	}
	return result.String()
}
