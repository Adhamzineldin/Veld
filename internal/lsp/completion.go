package lsp

import (
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

func computeCompletions(text string, pos Position, a ast.AST) []CompletionItem {
	lines := strings.Split(text, "\n")
	if pos.Line >= len(lines) {
		return nil
	}

	line := lines[pos.Line]
	prefix := ""
	if pos.Character <= len(line) {
		prefix = strings.TrimSpace(line[:pos.Character])
	}

	var items []CompletionItem

	// Keywords
	keywords := []string{
		"model", "module", "action", "enum", "import", "from",
		"method", "path", "input", "output", "query", "stream",
		"middleware", "errors", "description", "prefix", "extends", "default",
	}
	for _, kw := range keywords {
		items = append(items, CompletionItem{
			Label:  kw,
			Kind:   14, // Keyword
			Detail: "keyword",
		})
	}

	// Type names
	types := []string{"string", "int", "float", "bool", "date", "datetime", "uuid"}
	for _, t := range types {
		items = append(items, CompletionItem{
			Label:  t,
			Kind:   6, // Variable (type)
			Detail: "primitive type",
		})
	}

	// HTTP methods
	methods := []string{"GET", "POST", "PUT", "DELETE", "PATCH", "WS"}
	for _, m := range methods {
		items = append(items, CompletionItem{
			Label:  m,
			Kind:   6,
			Detail: "HTTP method",
		})
	}

	// Model names from AST
	for _, m := range a.Models {
		items = append(items, CompletionItem{
			Label:         m.Name,
			Kind:          7, // Class
			Detail:        "model",
			Documentation: m.Description,
		})
	}

	// Enum names from AST
	for _, en := range a.Enums {
		items = append(items, CompletionItem{
			Label:         en.Name,
			Kind:          13, // Enum
			Detail:        "enum",
			Documentation: en.Description,
		})
	}

	// Import path completions
	if strings.HasPrefix(prefix, "import") {
		items = append(items, CompletionItem{
			Label:  "@models/",
			Kind:   6,
			Detail: "import alias",
		})
		items = append(items, CompletionItem{
			Label:  "@modules/",
			Kind:   6,
			Detail: "import alias",
		})
	}

	return items
}
