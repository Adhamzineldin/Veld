package format

import (
	"strings"
	"testing"
)

func TestFormatBasicModel(t *testing.T) {
	input := `model User {
  id: uuid
  email: string
  name?: string
}`
	out, err := Format(input)
	if err != nil {
		t.Fatalf("Format: %v", err)
	}
	if !strings.Contains(out, "model User") {
		t.Error("should contain model User")
	}
	if !strings.Contains(out, "id") {
		t.Error("should contain fields")
	}
}

func TestFormatImportsSorted(t *testing.T) {
	input := `import @modules/users
import @models/auth
import @models/user
`
	out, err := Format(input)
	if err != nil {
		t.Fatalf("Format: %v", err)
	}
	// After sorting, @models should come before @modules
	modelsIdx := strings.Index(out, "@models")
	modulesIdx := strings.Index(out, "@modules")
	if modelsIdx < 0 || modulesIdx < 0 {
		t.Fatal("should contain both imports")
	}
	if modelsIdx > modulesIdx {
		t.Error("imports should be sorted: @models before @modules")
	}
}

func TestFormatTrailingNewline(t *testing.T) {
	input := "model User {\n  id: string\n}\n\n\n"
	out, err := Format(input)
	if err != nil {
		t.Fatalf("Format: %v", err)
	}
	if !strings.HasSuffix(out, "\n") {
		t.Error("should end with newline")
	}
	if strings.HasSuffix(out, "\n\n") {
		t.Error("should not end with multiple newlines")
	}
}

func TestFormatEmpty(t *testing.T) {
	out, err := Format("")
	if err != nil {
		t.Fatalf("Format: %v", err)
	}
	if strings.TrimSpace(out) != "" {
		t.Errorf("empty input should produce empty output, got %q", out)
	}
}

func TestFormatEnum(t *testing.T) {
	input := `enum Role {
  admin
  user
  guest
}`
	out, err := Format(input)
	if err != nil {
		t.Fatalf("Format: %v", err)
	}
	if !strings.Contains(out, "enum Role") {
		t.Error("should contain enum Role")
	}
}

func TestFormatIdempotent(t *testing.T) {
	input := `import @models/user

model User {
  id: uuid
  email: string
}
`
	first, err := Format(input)
	if err != nil {
		t.Fatalf("first format: %v", err)
	}
	second, err := Format(first)
	if err != nil {
		t.Fatalf("second format: %v", err)
	}
	if first != second {
		t.Errorf("format is not idempotent:\nfirst:  %q\nsecond: %q", first, second)
	}
}
