package typescript

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
)

// emitImports writes the type import line collecting all used types from all modules.
func emitImports(sb *strings.Builder, a ast.AST) {
	allTypes := make(map[string]bool)
	for _, mod := range a.Modules {
		for _, t := range emitter.CollectUsedTypes(a, mod) {
			allTypes[t] = true
		}
	}
	if len(allTypes) == 0 {
		return
	}

	// Stable order: enums first, then models
	var typeList []string
	for _, en := range a.Enums {
		if allTypes[en.Name] {
			typeList = append(typeList, en.Name)
		}
	}
	for _, m := range a.Models {
		if allTypes[m.Name] {
			typeList = append(typeList, m.Name)
		}
	}
	sb.WriteString(fmt.Sprintf("import type { %s } from '../types';\n", strings.Join(typeList, ", ")))
}

// emitErrorClass writes the VeldApiError class definition.
func emitErrorClass(sb *strings.Builder) {
	sb.WriteString(`
export class VeldApiError extends Error {
  status: number;
  body: string;
  constructor(status: number, body: string) {
    super(` + "`" + `Veld API error ${status}: ${body}` + "`" + `);
    this.name = 'VeldApiError';
    this.status = status;
    this.body = body;
  }
}
`)
}

// emitBaseURL writes the BASE constant.
func emitBaseURL(sb *strings.Builder, opts emitter.EmitOptions) {
	if opts.BaseUrl != "" {
		sb.WriteString(fmt.Sprintf("\nconst BASE = '%s';\n", opts.BaseUrl))
	} else {
		sb.WriteString("\nconst BASE = (typeof process !== 'undefined' && process.env?.VELD_API_URL) || '';\n")
	}
}

// emitHTTPHelpers writes the HTTP method helper functions (get, post, put, etc.)
// for each method actually used.
func emitHTTPHelpers(sb *strings.Builder, methods map[string]bool) {
	if methods["GET"] {
		sb.WriteString(`
async function get<T>(path: string): Promise<T> {
  const res = await fetch(BASE + path);
  if (!res.ok) throw new VeldApiError(res.status, await res.text());
  return res.json();
}
`)
	}

	bodyMethods := []string{"POST", "PUT", "PATCH", "DELETE"}
	for _, m := range bodyMethods {
		if methods[m] {
			fn := strings.ToLower(m)
			if m == "DELETE" {
				fn = "del"
			}
			sb.WriteString(fmt.Sprintf(`
async function %s<T>(path: string, body?: unknown): Promise<T> {
  const res = await fetch(BASE + path, {
    method: '%s',
    headers: { 'Content-Type': 'application/json' },
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  if (!res.ok) throw new VeldApiError(res.status, await res.text());
  return res.json();
}
`, fn, m))
		}
	}
}
