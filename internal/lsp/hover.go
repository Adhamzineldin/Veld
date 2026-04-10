package lsp

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

func computeHover(text string, pos Position, a ast.AST) *Hover {
	// ── Line-level hover for path:/prefix: directives ────────────────
	if h := hoverPathDirective(text, pos, a); h != nil {
		return h
	}
	// ── Line-level hover for @default annotations ────────────────────
	if h := hoverDefaultAnnotation(text, pos); h != nil {
		return h
	}

	word := wordAtPosition(text, pos)
	if word == "" {
		return nil
	}

	// Check models
	for _, m := range a.Models {
		if m.Name == word {
			var sb strings.Builder
			sb.WriteString(fmt.Sprintf("**model %s**\n\n", m.Name))
			if m.Description != "" {
				sb.WriteString(m.Description + "\n\n")
			}
			if m.Extends != "" {
				sb.WriteString(fmt.Sprintf("extends `%s`\n\n", m.Extends))
			}
			sb.WriteString("Fields:\n")
			for _, f := range m.Fields {
				opt := ""
				if f.Optional {
					opt = "?"
				}
				typeStr := f.Type
				if len(f.UnionTypes) > 0 {
					typeStr = strings.Join(f.UnionTypes, " | ")
				}
				sb.WriteString(fmt.Sprintf("- `%s%s`: %s\n", f.Name, opt, typeStr))
			}
			return &Hover{
				Contents: MarkupContent{Kind: "markdown", Value: sb.String()},
			}
		}
	}

	// Check enums
	for _, en := range a.Enums {
		if en.Name == word {
			var sb strings.Builder
			sb.WriteString(fmt.Sprintf("**enum %s**\n\n", en.Name))
			if en.Description != "" {
				sb.WriteString(en.Description + "\n\n")
			}
			sb.WriteString("Values: `" + strings.Join(en.Values, "`, `") + "`")
			return &Hover{
				Contents: MarkupContent{Kind: "markdown", Value: sb.String()},
			}
		}
	}

	// Check modules
	for _, mod := range a.Modules {
		if mod.Name == word {
			var sb strings.Builder
			sb.WriteString(fmt.Sprintf("**module %s**\n\n", mod.Name))
			if mod.Description != "" {
				sb.WriteString(mod.Description + "\n\n")
			}
			if mod.Prefix != "" {
				sb.WriteString(fmt.Sprintf("prefix: `%s`\n\n", mod.Prefix))
			}
			sb.WriteString(fmt.Sprintf("%d action(s)\n", len(mod.Actions)))
			for _, act := range mod.Actions {
				fullPath := resolveFullPath(a.Prefix, mod.Prefix, act.Path)
				sb.WriteString(fmt.Sprintf("- `%s %s` — %s\n", act.Method, fullPath, act.Name))
			}
			return &Hover{
				Contents: MarkupContent{Kind: "markdown", Value: sb.String()},
			}
		}

		// Check actions
		for _, act := range mod.Actions {
			if act.Name == word {
				fullPath := resolveFullPath(a.Prefix, mod.Prefix, act.Path)
				var sb strings.Builder
				sb.WriteString(fmt.Sprintf("**action %s**\n\n", act.Name))
				if act.Description != "" {
					sb.WriteString(act.Description + "\n\n")
				}
				sb.WriteString(fmt.Sprintf("Full route: `%s %s`\n\n", act.Method, fullPath))
				if act.Input != "" {
					sb.WriteString(fmt.Sprintf("- input: `%s`\n", act.Input))
				}
				if act.Output != "" {
					out := act.Output
					if act.OutputArray {
						out += "[]"
					}
					if act.SuccessStatus > 0 {
						sb.WriteString(fmt.Sprintf("- output: `%s` → %d\n", out, act.SuccessStatus))
					} else {
						sb.WriteString(fmt.Sprintf("- output: `%s`\n", out))
					}
				}
				if act.Stream != "" {
					sb.WriteString(fmt.Sprintf("- stream: `%s`\n", act.Stream))
				}
				if len(act.Errors) > 0 {
					sb.WriteString(fmt.Sprintf("- errors: `%s`\n", strings.Join(act.Errors, "`, `")))
				}
				return &Hover{
					Contents: MarkupContent{Kind: "markdown", Value: sb.String()},
				}
			}
		}
	}

	return nil
}

func wordAtPosition(text string, pos Position) string {
	lines := strings.Split(text, "\n")
	if pos.Line >= len(lines) {
		return ""
	}
	line := lines[pos.Line]
	if pos.Character >= len(line) {
		return ""
	}

	// Find word boundaries
	start := pos.Character
	for start > 0 && isIdentChar(line[start-1]) {
		start--
	}
	end := pos.Character
	for end < len(line) && isIdentChar(line[end]) {
		end++
	}

	if start == end {
		return ""
	}
	return line[start:end]
}

func isIdentChar(ch byte) bool {
	return (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') ||
		(ch >= '0' && ch <= '9') || ch == '_'
}

// resolveFullPath combines app prefix + module prefix + action path into
// the full route that will be registered at runtime.
func resolveFullPath(appPrefix, modulePrefix, actionPath string) string {
	full := actionPath
	if modulePrefix != "" {
		full = modulePrefix + full
	}
	if appPrefix != "" {
		full = appPrefix + full
	}
	return full
}

// hoverPathDirective checks if the cursor is on a path: or prefix: line
// and shows the fully resolved route path with all prefixes combined.
func hoverPathDirective(text string, pos Position, a ast.AST) *Hover {
	lines := strings.Split(text, "\n")
	if pos.Line >= len(lines) {
		return nil
	}
	line := strings.TrimSpace(lines[pos.Line])

	// Find which module/action context we're in by scanning backwards
	if strings.HasPrefix(line, "path:") {
		pathVal := strings.TrimSpace(strings.TrimPrefix(line, "path:"))
		mod, act := findContextAt(lines, pos.Line)
		if act != "" && mod != "" {
			modPrefix := ""
			appPrefix := a.Prefix
			for _, m := range a.Modules {
				if m.Name == mod {
					modPrefix = m.Prefix
					break
				}
			}
			fullPath := resolveFullPath(appPrefix, modPrefix, pathVal)
			return &Hover{
				Contents: MarkupContent{
					Kind:  "markdown",
					Value: fmt.Sprintf("**Resolved route:** `%s`\n\napp prefix: `%s` → module prefix: `%s` → path: `%s`", fullPath, appPrefix, modPrefix, pathVal),
				},
			}
		}
	}

	if strings.HasPrefix(line, "prefix:") {
		prefixVal := strings.TrimSpace(strings.TrimPrefix(line, "prefix:"))
		mod, _ := findContextAt(lines, pos.Line)
		if mod != "" {
			appPrefix := a.Prefix
			fullPrefix := appPrefix + prefixVal
			return &Hover{
				Contents: MarkupContent{
					Kind:  "markdown",
					Value: fmt.Sprintf("**Resolved prefix:** `%s`\n\napp prefix: `%s` + module prefix: `%s`", fullPrefix, appPrefix, prefixVal),
				},
			}
		} else {
			// Top-level prefix
			return &Hover{
				Contents: MarkupContent{
					Kind:  "markdown",
					Value: fmt.Sprintf("**App prefix:** `%s`\n\nThis prefix is prepended to all module routes.", prefixVal),
				},
			}
		}
	}

	return nil
}

// hoverDefaultAnnotation shows type info when hovering over @default.
func hoverDefaultAnnotation(text string, pos Position) *Hover {
	lines := strings.Split(text, "\n")
	if pos.Line >= len(lines) {
		return nil
	}
	line := lines[pos.Line]
	trimmed := strings.TrimSpace(line)

	// Check if line contains @default(...)
	idx := strings.Index(trimmed, "@default(")
	if idx < 0 {
		return nil
	}

	// Extract field type from the line (fieldName: Type @default(...))
	colonIdx := strings.Index(trimmed, ":")
	if colonIdx < 0 {
		return nil
	}
	afterColon := strings.TrimSpace(trimmed[colonIdx+1:])
	// Type is everything before the first @
	atIdx := strings.Index(afterColon, "@")
	if atIdx < 0 {
		return nil
	}
	fieldType := strings.TrimSpace(afterColon[:atIdx])

	// Extract default value
	start := strings.Index(trimmed, "@default(") + len("@default(")
	end := strings.Index(trimmed[start:], ")")
	if end < 0 {
		return nil
	}
	defaultVal := trimmed[start : start+end]

	var expected string
	switch fieldType {
	case "string", "date", "datetime", "uuid", "decimal":
		expected = "quoted string (e.g. `\"value\"`)"
	case "int":
		expected = "whole number (e.g. `42`)"
	case "float":
		expected = "number (e.g. `3.14`)"
	case "bool":
		expected = "`true` or `false`"
	default:
		expected = "enum value"
	}

	return &Hover{
		Contents: MarkupContent{
			Kind:  "markdown",
			Value: fmt.Sprintf("**@default(%s)**\n\nField type: `%s`\nExpected: %s", defaultVal, fieldType, expected),
		},
	}
}

// findContextAt scans backwards from lineNum to find the enclosing module/action names.
func findContextAt(lines []string, lineNum int) (moduleName, actionName string) {
	depth := 0
	for i := lineNum; i >= 0; i-- {
		trimmed := strings.TrimSpace(lines[i])
		if trimmed == "}" {
			depth++
		}
		if trimmed == "{" || strings.HasSuffix(trimmed, "{") {
			if depth > 0 {
				depth--
				continue
			}
		}
		if strings.HasPrefix(trimmed, "action ") && actionName == "" {
			parts := strings.Fields(trimmed)
			if len(parts) >= 2 {
				actionName = strings.TrimSuffix(parts[1], "{")
			}
		}
		if strings.HasPrefix(trimmed, "module ") {
			parts := strings.Fields(trimmed)
			if len(parts) >= 2 {
				moduleName = strings.TrimSuffix(parts[1], "{")
				return
			}
		}
	}
	return
}
