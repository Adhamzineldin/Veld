package language

// VeldLanguageSpec defines the complete Veld language specification
// This is the SINGLE SOURCE OF TRUTH for all language constants
type VeldLanguageSpec struct {
	Keywords     []string    `json:"keywords"`
	HttpMethods  []string    `json:"httpMethods"`
	BuiltinTypes []string    `json:"builtinTypes"`
	Directives   []string    `json:"directives"`
	SpecialTypes []string    `json:"specialTypes"`
	Annotations  []string    `json:"annotations"`
	ConfigKeys   []ConfigKey `json:"configKeys"`
	Version      string      `json:"version"`
}

// ConfigKey is a veld.config.json key paired with its IDE description.
// A slice (not a map) so generated plugin output is deterministic.
type ConfigKey struct {
	Key         string `json:"key"`
	Description string `json:"description"`
}

// GetLanguageSpec returns the complete Veld language specification
func GetLanguageSpec() *VeldLanguageSpec {
	return &VeldLanguageSpec{
		Version: "1.0.0",
		// Must stay in sync with the keyword switch in internal/lexer/lexer.go.
		Keywords: []string{
			"model",
			"module",
			"action",
			"enum",
			"constants",
			"constant",
			"import",
			"from",
			"extends",
		},
		HttpMethods: []string{
			"GET",
			"POST",
			"PUT",
			"DELETE",
			"PATCH",
			"HEAD",
			"OPTIONS",
			"WS",
		},
		BuiltinTypes: []string{
			"string",
			"int",
			"long",
			"float",
			"decimal",
			"bool",
			"date",
			"datetime",
			"time",
			"uuid",
			"bytes",
			"json",
			"any",
		},
		Directives: []string{
			"description",
			"prefix",
			"method",
			"path",
			"input",
			"output",
			"response",
			"query",
			"headers",
			"stream",
			"emit",
			"middleware",
			"errors",
			"default",
		},
		SpecialTypes: []string{
			"List",
			"Map",
		},
		// Matches the annotations the parser accepts and the AST models.
		Annotations: []string{
			"default",
			"unique",
			"required",
			"optional",
			"index",
			"primary",
			"autoincrement",
			"readonly",
			"deprecated",
			"example",
			"relation",
			"min",
			"max",
			"minLength",
			"maxLength",
			"regex",
		},
		ConfigKeys: []ConfigKey{
			{"$schema", "JSON Schema reference for IDE autocompletion"},
			{"input", "Path to the main .veld entry file"},
			{"description", "Human/AI-readable project description"},
			{"backendConfig", "Nested backend configuration: { target, framework, out, dir, validate }"},
			{"frontendConfig", "Nested frontend configuration: { target, out, dir }"},
			{"backend", "Backend target (flat, deprecated): node, python, go, java, csharp, php, rust"},
			{"frontend", "Frontend SDK (flat, deprecated): react, vue, angular, svelte, typescript, dart, kotlin, swift, none"},
			{"out", "Output directory for generated code"},
			{"backendOut", "Deprecated — use backendConfig.out"},
			{"frontendOut", "Deprecated — use frontendConfig.out"},
			{"backendDir", "Deprecated — use backendConfig.dir"},
			{"frontendDir", "Deprecated — use frontendConfig.dir"},
			{"backendFramework", "Deprecated — use backendConfig.framework"},
			{"frontendFramework", "Deprecated — use frontendConfig.framework"},
			{"validate", "Generate runtime validators (prefer backendConfig.validate)"},
			{"baseUrl", "Base URL baked into generated SDK clients"},
			{"aliases", "Custom @alias → folder mappings"},
			{"services", "Module name → base URL override for multi-module APIs"},
			{"serverSdk", "Emit server-to-server typed SDK client"},
			{"tools", "Auxiliary generators: { openapi, dockerfile, cicd, database, scaffold, envconfig }"},
			{"hooks", "Lifecycle hooks: { postGenerate }"},
			{"postGenerate", "Deprecated — use hooks.postGenerate"},
			{"registry", "Cloud registry: { enabled, url, org, package, version }"},
			{"workspace", "Multi-service monorepo workspace entries"},
		},
	}
}

// IsKeyword checks if a word is a keyword
func (spec *VeldLanguageSpec) IsKeyword(word string) bool {
	for _, kw := range spec.Keywords {
		if kw == word {
			return true
		}
	}
	return false
}

// IsHttpMethod checks if a word is a valid HTTP method
func (spec *VeldLanguageSpec) IsHttpMethod(word string) bool {
	for _, method := range spec.HttpMethods {
		if method == word {
			return true
		}
	}
	return false
}

// IsBuiltinType checks if a word is a builtin type
func (spec *VeldLanguageSpec) IsBuiltinType(word string) bool {
	for _, t := range spec.BuiltinTypes {
		if t == word {
			return true
		}
	}
	return false
}

// IsDirective checks if a word is a directive
func (spec *VeldLanguageSpec) IsDirective(word string) bool {
	for _, d := range spec.Directives {
		if d == word {
			return true
		}
	}
	return false
}

// IsSpecialType checks if a word is a special type (List, Map)
func (spec *VeldLanguageSpec) IsSpecialType(word string) bool {
	for _, t := range spec.SpecialTypes {
		if t == word {
			return true
		}
	}
	return false
}
