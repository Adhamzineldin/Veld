package errors

import "testing"

func TestVeldErrorFormat(t *testing.T) {
	tests := []struct {
		name     string
		err      *VeldError
		expected string
	}{
		{"file+line", NewParseError("app.veld", 10, "unexpected token"), "app.veld:10: unexpected token"},
		{"file only", &VeldError{Kind: KindConfig, File: "config.json", Message: "missing field"}, "config.json: missing field"},
		{"line only", NewParseError("", 5, "bad syntax"), "line 5: bad syntax"},
		{"no location", NewConfigError("no input", nil), "no input"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := tc.err.Error()
			if got != tc.expected {
				t.Errorf("got %q, want %q", got, tc.expected)
			}
		})
	}
}

func TestVeldErrorUnwrap(t *testing.T) {
	inner := NewParseError("a.veld", 1, "inner")
	outer := &VeldError{Kind: KindEmit, Message: "outer", Cause: inner}
	if outer.Unwrap() != inner {
		t.Error("Unwrap should return Cause")
	}
}

func TestListCollect(t *testing.T) {
	l := &List{}
	if l.HasErrors() {
		t.Error("empty list should not have errors")
	}
	l.Add(NewParseError("a.veld", 1, "e1"))
	l.Add(NewValidationError("b.veld", 2, "e2"))
	if l.Len() != 2 {
		t.Errorf("expected 2, got %d", l.Len())
	}
	errs := l.AsErrors()
	if len(errs) != 2 {
		t.Errorf("AsErrors: expected 2, got %d", len(errs))
	}
}

func TestAsVeldError(t *testing.T) {
	ve := NewParseError("x.veld", 1, "test")
	if AsVeldError(ve) == nil {
		t.Error("should extract VeldError")
	}
	if AsVeldError(nil) != nil {
		t.Error("nil should return nil")
	}
}

func TestKindString(t *testing.T) {
	kinds := map[Kind]string{
		KindConfig: "config", KindParse: "parse", KindValidation: "validation",
		KindEmit: "emit", KindLint: "lint", KindIO: "io",
	}
	for k, expected := range kinds {
		if k.String() != expected {
			t.Errorf("Kind(%d).String() = %q, want %q", k, k.String(), expected)
		}
	}
}
