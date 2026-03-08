package emitter

import (
	"regexp"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
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
		if act.Stream != "" {
			queue = append(queue, act.Stream)
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
				// Union type members that reference models
				for _, ut := range f.UnionTypes {
					if _, isModel := byName[ut]; isModel && !used[ut] {
						queue = append(queue, ut)
					}
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
			for _, ut := range f.UnionTypes {
				if enumNames[ut] {
					usedEnums[ut] = true
				}
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
		for _, name := range []string{act.Input, act.Output, act.Query, act.Stream} {
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
			// Union type members that reference models or enums
			for _, ut := range f.UnionTypes {
				if _, isModel := byName[ut]; isModel && !seen[ut] {
					seen[ut] = true
					result = append(result, ut)
				}
				if enumNames[ut] && !seen[ut] {
					seen[ut] = true
					result = append(result, ut)
				}
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

// ToCamelCase converts a PascalCase name to camelCase (lowercase first letter).
func ToCamelCase(s string) string {
	if s == "" {
		return s
	}
	// Find the boundary: lowercase everything up to (but not including) the
	// last uppercase letter in a leading-uppercase run.  E.g. "HTTPClient" → "httpClient".
	i := 0
	for i < len(s) && s[i] >= 'A' && s[i] <= 'Z' {
		i++
	}
	switch {
	case i == 0:
		return s // already starts lowercase
	case i == 1:
		return strings.ToLower(s[:1]) + s[1:]
	default:
		// e.g. i==4 for "HTTPClient" → "http" + "Client"
		return strings.ToLower(s[:i-1]) + s[i-1:]
	}
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

// ResolveRoutePath computes the full route path by joining the app-level prefix,
// the module prefix, and the action path. Empty segments are skipped.
func ResolveRoutePath(appPrefix string, mod ast.Module, act ast.Action) string {
	return appPrefix + mod.Prefix + act.Path
}

// ToScreamingSnake converts a camelCase or PascalCase name to SCREAMING_SNAKE_CASE.
func ToScreamingSnake(s string) string {
	return strings.ToUpper(ToSnakeCase(s))
}

// ErrorCode builds a deterministic error code string from an action name and
// error name. Example: ("ListUsers", "NotFound") → "LIST_USERS_NOT_FOUND".
func ErrorCode(actionName, errorName string) string {
	return ToScreamingSnake(actionName) + "_" + ToScreamingSnake(errorName)
}

// ErrorHTTPStatus maps well-known error names to HTTP status codes.
// Unknown names default to 500.
var errorStatusMap = map[string]int{
	"NotFound":            404,
	"Unauthorized":        401,
	"Forbidden":           403,
	"Conflict":            409,
	"BadRequest":          400,
	"ValidationFailed":    422,
	"Gone":                410,
	"TooManyRequests":     429,
	"InternalServerError": 500,
	"ServiceUnavailable":  503,
	"NotImplemented":      501,
}

// ErrorHTTPStatus returns the HTTP status code for a well-known error name.
// Unknown names return 500.
func ErrorHTTPStatus(errorName string) int {
	if status, ok := errorStatusMap[errorName]; ok {
		return status
	}
	return 500
}

// HasErrors returns true if any action in the module defines error codes.
func HasErrors(mod ast.Module) bool {
	for _, act := range mod.Actions {
		if len(act.Errors) > 0 {
			return true
		}
	}
	return false
}
