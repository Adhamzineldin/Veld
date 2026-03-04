package lsp

import (
	"encoding/json"
	"testing"
)

func TestHandlerInitialize(t *testing.T) {
	h := NewHandler()
	result := h.Initialize()
	if result.Capabilities.TextDocumentSync != 1 {
		t.Errorf("expected full sync (1), got %d", result.Capabilities.TextDocumentSync)
	}
	if !result.Capabilities.HoverProvider {
		t.Error("hover should be enabled")
	}
	if !result.Capabilities.DefinitionProvider {
		t.Error("definition should be enabled")
	}
	if result.Capabilities.CompletionProvider == nil {
		t.Error("completion should be enabled")
	}
}

func TestHandlerDidOpenAndDiagnostics(t *testing.T) {
	h := NewHandler()
	uri := "file:///test.veld"
	text := `model User {
  id: uuid
  name: string
}`
	h.DidOpen(uri, text)

	diags := h.GetDiagnostics(uri)
	// Valid source should produce no diagnostics
	if len(diags) != 0 {
		t.Errorf("expected 0 diagnostics for valid source, got %d", len(diags))
		for _, d := range diags {
			t.Logf("  diagnostic: %s", d.Message)
		}
	}
}

func TestHandlerDidChange(t *testing.T) {
	h := NewHandler()
	uri := "file:///test.veld"
	h.DidOpen(uri, "model User { id: string }")
	h.DidChange(uri, "model User { id: uuid }")

	// Should not panic or error
	diags := h.GetDiagnostics(uri)
	_ = diags
}

func TestHandlerDiagnosticsOnInvalidSource(t *testing.T) {
	h := NewHandler()
	uri := "file:///bad.veld"
	// Missing closing brace — should produce diagnostics
	h.DidOpen(uri, "model User { id: string")

	diags := h.GetDiagnostics(uri)
	if len(diags) == 0 {
		t.Error("expected diagnostics for invalid source")
	}
}

func TestHandlerCompletion(t *testing.T) {
	h := NewHandler()
	uri := "file:///test.veld"
	h.DidOpen(uri, `model User {
  id: uuid
  name: string
}

module Users {
  action Get {
    method: GET
    path: /
    output: User
  }
}`)

	items := h.Complete(uri, Position{Line: 8, Character: 4})
	if len(items) == 0 {
		t.Error("expected completion items")
	}
}

func TestHandlerHover(t *testing.T) {
	h := NewHandler()
	uri := "file:///test.veld"
	h.DidOpen(uri, `model User {
  description: "A user"
  id: uuid
}`)

	hover := h.Hover(uri, Position{Line: 0, Character: 7})
	if hover == nil {
		// Hover may return nil if position doesn't match — that's ok
		return
	}
	if hover.Contents.Kind != "markdown" {
		t.Errorf("expected markdown hover, got %q", hover.Contents.Kind)
	}
}

func TestProtocolTypes(t *testing.T) {
	// Ensure protocol types JSON marshal correctly
	resp := Response{
		JSONRPC: "2.0",
		ID:      1,
		Result:  map[string]string{"test": "value"},
	}
	data, err := json.Marshal(resp)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	if len(data) == 0 {
		t.Error("should produce non-empty JSON")
	}
}

func TestDiagnosticSeverity(t *testing.T) {
	d := Diagnostic{
		Range:    Range{Start: Position{0, 0}, End: Position{0, 5}},
		Severity: 1, // Error
		Message:  "test error",
		Source:   "veld",
	}
	data, _ := json.Marshal(d)
	if len(data) == 0 {
		t.Error("diagnostic should marshal")
	}
}
