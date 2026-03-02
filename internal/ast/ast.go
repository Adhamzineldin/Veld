package ast

// AST is the root of a parsed .veld contract.
type AST struct {
	ASTVersion  string              `json:"astVersion"`
	Prefix      string              `json:"prefix,omitempty"` // app-level route prefix (prepended to all modules)
	Imports     []string            `json:"-"`                // resolved by the file loader, not serialised
	Models      []Model             `json:"models"`
	Modules     []Module            `json:"modules"`
	Enums       []Enum              `json:"enums,omitempty"`
	FileImports map[string][]string `json:"-"` // sourceFile → directly imported file paths
}

// Enum is a named set of string constants.
type Enum struct {
	Name        string   `json:"name"`
	Description string   `json:"description,omitempty"`
	Values      []string `json:"values"`
	SourceFile  string   `json:"-"`
	Line        int      `json:"-"` // line in source where this enum was defined
}

// Model is a named data type with typed fields.
type Model struct {
	Name        string  `json:"name"`
	Description string  `json:"description,omitempty"`
	Extends     string  `json:"extends,omitempty"` // parent model name for inheritance
	Fields      []Field `json:"fields"`
	SourceFile  string  `json:"-"` // absolute path; set by file loader, not serialised
	Line        int     `json:"-"` // line in source where this model was defined
}

// Field is a single property of a Model.
type Field struct {
	Name         string `json:"name"`
	Type         string `json:"type"`
	Optional     bool   `json:"optional,omitempty"`
	IsArray      bool   `json:"isArray,omitempty"`
	IsMap        bool   `json:"isMap,omitempty"`        // Map<string, V>
	MapValueType string `json:"mapValueType,omitempty"` // the V in Map<string, V>
	Default      string `json:"default,omitempty"`      // @default(value)
	Line         int    `json:"-"`                      // line in source where this field was defined
}

// Module groups related Actions.
type Module struct {
	Name        string   `json:"name"`
	Description string   `json:"description,omitempty"`
	Prefix      string   `json:"prefix,omitempty"` // route path prefix
	Actions     []Action `json:"actions"`
	SourceFile  string   `json:"-"` // absolute path; set by file loader, not serialised
	Line        int      `json:"-"` // line in source where this module was defined
}

// Action is a single API endpoint inside a Module.
type Action struct {
	Name        string   `json:"name"`
	Description string   `json:"description,omitempty"`
	Method      string   `json:"method"`
	Path        string   `json:"path"`
	Input       string   `json:"input"`
	Output      string   `json:"output"`
	OutputArray bool     `json:"outputArray,omitempty"` // output User[] → true
	Query       string   `json:"query,omitempty"`       // query param model
	Stream      string   `json:"stream,omitempty"`      // WebSocket message type for WS actions
	Errors      []string `json:"errors,omitempty"`      // typed error codes for this action
	Middleware  []string `json:"middleware"`
	Line        int      `json:"-"` // line in source where this action was defined
}
