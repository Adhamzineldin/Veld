package language

// VeldLanguageSpec defines the complete Veld language specification
// This is the SINGLE SOURCE OF TRUTH for all language constants
type VeldLanguageSpec struct {
	Keywords     []string `json:"keywords"`
	HttpMethods  []string `json:"httpMethods"`
	BuiltinTypes []string `json:"builtinTypes"`
	Directives   []string `json:"directives"`
	SpecialTypes []string `json:"specialTypes"`
	Annotations  []string `json:"annotations"`
	ConfigKeys   []string `json:"configKeys"`
	Version      string   `json:"version"`
}

// GetLanguageSpec returns the complete Veld language specification
func GetLanguageSpec() *VeldLanguageSpec {
	return &VeldLanguageSpec{
		Version: "1.0.0",
		Keywords: []string{
			"model",
			"module",
			"action",
			"enum",
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
		},
		BuiltinTypes: []string{
			"string",
			"int",
			"float",
			"bool",
			"date",
			"datetime",
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
			"query",
			"stream",
			"middleware",
			"errors",
			"default",
		},
		SpecialTypes: []string{
			"List",
			"Map",
		},
		Annotations: []string{
			"default",
			"unique",
			"required",
			"optional",
			"index",
			"primary",
			"autoincrement",
			"readonly",
		},
		ConfigKeys: []string{
			"$schema",
			"input",
			"description",
			"backendConfig",
			"frontendConfig",
			"backend",
			"frontend",
			"out",
			"backendOut",
			"frontendOut",
			"backendDir",
			"frontendDir",
			"backendFramework",
			"frontendFramework",
			"validate",
			"baseUrl",
			"aliases",
			"services",
			"serverSdk",
			"tools",
			"hooks",
			"postGenerate",
			"registry",
			"workspace",
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
