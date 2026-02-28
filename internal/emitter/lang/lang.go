package lang

import (
	"github.com/veld-dev/veld/internal/ast"
)

// LanguageMetadata describes capabilities and features of a language backend.
type LanguageMetadata struct {
	Name              string   // e.g. "go", "rust", "java"
	Version           string   // language version
	Runtime           string   // "compiled" or "interpreted"
	Framework         string   // e.g. "chi", "axum", "spring-boot"
	Features          []string // e.g. ["streaming", "async", "websockets"]
	ExportPath        string   // where generated files go
	ImportPaths       []string // common import patterns
	TypeMapperVersion string   // version of type system
}

// NamingContext provides context for identifier naming conventions.
type NamingContext int

const (
	NamingContextExported NamingContext = iota // PascalCase, exported
	NamingContextPrivate                       // camelCase, private
	NamingContextConstant                      // SCREAMING_SNAKE_CASE
	NamingContextPackage                       // snake_case
	NamingContextDatabase                      // snake_case
)

// CommentStyle describes language comment syntax.
type CommentStyle struct {
	Single   string // e.g., "//"
	Multi    string // e.g., "/*"
	MultiEnd string // e.g., "*/"
}

// LanguageAdapter provides language-specific conventions and type mappings.
// Implementations: GoAdapter, RustAdapter, JavaAdapter, CSharpAdapter, PHPAdapter
type LanguageAdapter interface {
	// Metadata returns information about this language.
	Metadata() LanguageMetadata

	// MapType converts a Veld type to the target language's type.
	// Returns the type string and any imports needed.
	// Examples:
	//   MapType("string") -> ("string", nil) for Go
	//   MapType("int") -> ("int64", nil) for Go
	//   MapType("List<User>") -> ("[]User", ["User"]) for Go
	MapType(veldType string) (targetType string, imports []string, err error)

	// NamingConvention converts a name to the target language's convention.
	// context specifies whether this is a field, constant, package name, etc.
	NamingConvention(name string, context NamingContext) string

	// StructFieldTag generates a struct/class field tag/annotation.
	// Examples:
	//   Go: `json:"id"`
	//   Java: `@SerializedName("id")`
	//   Rust: (empty, uses serde derive)
	StructFieldTag(fieldName string, fieldType string) string

	// ImportStatement generates import/use syntax.
	// Examples:
	//   Go: `import "package"`
	//   Rust: `use module::Type;`
	//   Java: `import package.Class;`
	ImportStatement(module string, alias string) string

	// CommentSyntax returns the language's comment style.
	CommentSyntax() CommentStyle

	// FileExtension returns the file extension for source files.
	// Examples: ".go", ".rs", ".java", ".cs", ".php"
	FileExtension() string

	// NullableType returns how the language represents nullable/optional types.
	// Examples:
	//   Go: "*string" or "sql.NullString"
	//   Rust: "Option<String>"
	//   Java: "Optional<String>"
	NullableType(baseType string) string
}

// TypeGenerator generates model and enum definitions.
type TypeGenerator interface {
	// GenerateModel generates a model/struct/class definition.
	GenerateModel(model ast.Model, adapter LanguageAdapter) (code string, err error)

	// GenerateEnum generates an enum definition.
	GenerateEnum(enum ast.Enum, adapter LanguageAdapter) (code string, err error)
}

// RouteGenerator generates HTTP route handlers and middleware.
type RouteGenerator interface {
	// GenerateRoutes generates all HTTP routes for a module.
	GenerateRoutes(module ast.Module, adapter LanguageAdapter) (code string, err error)

	// GenerateMiddleware generates common middleware (error handling, logging).
	GenerateMiddleware(adapter LanguageAdapter) (code string, err error)

	// GenerateErrorHandler generates error handling logic.
	GenerateErrorHandler(adapter LanguageAdapter) (code string, err error)
}

// SchemaValidator generates validation schemas for input validation.
type SchemaValidator interface {
	// GenerateSchema generates validation logic for an action input.
	// Examples:
	//   Node: Zod schema
	//   Python: Pydantic BaseModel
	//   Go: validator struct tags or validation helpers
	GenerateSchema(action ast.Action, adapter LanguageAdapter) (code string, err error)

	// ValidationLibraryName returns the validation library name.
	// Examples: "zod", "pydantic", "validator"
	ValidationLibraryName() string
}
