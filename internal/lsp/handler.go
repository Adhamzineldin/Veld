package lsp

import (
	"strings"
	"sync"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/lexer"
	"github.com/Adhamzineldin/Veld/internal/parser"
	"github.com/Adhamzineldin/Veld/internal/validator"
)

// Handler manages document state and provides LSP operations.
type Handler struct {
	mu        sync.RWMutex
	documents map[string]string  // URI → content
	asts      map[string]ast.AST // URI → parsed AST
}

// NewHandler creates a new Handler.
func NewHandler() *Handler {
	return &Handler{
		documents: make(map[string]string),
		asts:      make(map[string]ast.AST),
	}
}

// Initialize returns the server capabilities.
func (h *Handler) Initialize() InitializeResult {
	return InitializeResult{
		Capabilities: ServerCapabilities{
			TextDocumentSync:   1, // Full sync
			CompletionProvider: &struct{}{},
			HoverProvider:      true,
			DefinitionProvider: true,
		},
	}
}

// DidOpen stores the document content and parses it.
func (h *Handler) DidOpen(uri, text string) {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.documents[uri] = text
	h.parseDocument(uri, text)
}

// DidChange updates the document content and re-parses.
func (h *Handler) DidChange(uri, text string) {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.documents[uri] = text
	h.parseDocument(uri, text)
}

func (h *Handler) parseDocument(uri, text string) {
	l := lexer.New(text)
	tokens, err := l.Tokenize()
	if err != nil {
		return
	}
	p := parser.New(tokens)
	a, err := p.Parse()
	if err != nil {
		return
	}
	h.asts[uri] = a
}

// GetDiagnostics runs lexer+parser+validator and returns LSP diagnostics.
func (h *Handler) GetDiagnostics(uri string) []Diagnostic {
	h.mu.RLock()
	text, ok := h.documents[uri]
	h.mu.RUnlock()
	if !ok {
		return nil
	}
	return computeDiagnostics(text)
}

// Complete returns completion items at the given position.
func (h *Handler) Complete(uri string, pos Position) []CompletionItem {
	h.mu.RLock()
	text, ok := h.documents[uri]
	a := h.asts[uri]
	h.mu.RUnlock()
	if !ok {
		return nil
	}
	return computeCompletions(text, pos, a)
}

// Hover returns hover info at the given position.
func (h *Handler) Hover(uri string, pos Position) *Hover {
	h.mu.RLock()
	text, ok := h.documents[uri]
	a := h.asts[uri]
	h.mu.RUnlock()
	if !ok {
		return nil
	}
	return computeHover(text, pos, a)
}

// Definition returns the definition location for the symbol at the given position.
func (h *Handler) Definition(uri string, pos Position) *Location {
	h.mu.RLock()
	text, ok := h.documents[uri]
	a := h.asts[uri]
	h.mu.RUnlock()
	if !ok {
		return nil
	}
	return computeDefinition(text, pos, a, uri)
}

// ── diagnostics ───────────────────────────────────────────────────────────────

func computeDiagnostics(text string) []Diagnostic {
	var diags []Diagnostic

	l := lexer.New(text)
	tokens, err := l.Tokenize()
	if err != nil {
		line := extractLine(err.Error())
		diags = append(diags, Diagnostic{
			Range:    lineRange(line),
			Severity: 1,
			Message:  err.Error(),
			Source:   "veld",
		})
		return diags
	}

	p := parser.New(tokens)
	a, err := p.Parse()
	if err != nil {
		line := extractLine(err.Error())
		diags = append(diags, Diagnostic{
			Range:    lineRange(line),
			Severity: 1,
			Message:  err.Error(),
			Source:   "veld",
		})
		return diags
	}

	errs := validator.Validate(a)
	for _, e := range errs {
		line := extractLine(e.Error())
		diags = append(diags, Diagnostic{
			Range:    lineRange(line),
			Severity: 1,
			Message:  e.Error(),
			Source:   "veld",
		})
	}

	return diags
}

func extractLine(msg string) int {
	// Try to parse "line N:" or "file:N:" patterns
	parts := strings.SplitN(msg, ":", 3)
	if len(parts) >= 2 {
		for _, part := range parts[:2] {
			part = strings.TrimSpace(part)
			part = strings.TrimPrefix(part, "line ")
			var n int
			for _, ch := range part {
				if ch >= '0' && ch <= '9' {
					n = n*10 + int(ch-'0')
				} else {
					n = 0
					break
				}
			}
			if n > 0 {
				return n - 1 // LSP is 0-indexed
			}
		}
	}
	return 0
}

func lineRange(line int) Range {
	return Range{
		Start: Position{Line: line, Character: 0},
		End:   Position{Line: line, Character: 1000},
	}
}
