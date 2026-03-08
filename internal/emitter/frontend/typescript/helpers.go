package typescript

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/emitter"
)

// emitErrorClass writes the VeldApiError class definition.
func emitErrorClass(sb *strings.Builder) {
	sb.WriteString(`
export class VeldApiError extends Error {
  status: number;
  body: string;
  code: string;
  constructor(status: number, body: string, code?: string) {
    super(` + "`" + `Veld API error ${status}: ${body}` + "`" + `);
    this.name = 'VeldApiError';
    this.status = status;
    this.body = body;
    this.code = code ?? '';
  }
}

/** Type guard — narrows unknown catch value to VeldApiError. */
export function isApiError(err: unknown): err is VeldApiError {
  return err instanceof VeldApiError;
}

/**
 * Check if an error matches a specific error code.
 * Usage: if (isErrorCode(err, usersApi.errors.getUser.notFound)) { ... }
 */
export function isErrorCode(err: unknown, code: string): err is VeldApiError {
  return err instanceof VeldApiError && err.code === code;
}

async function parseErrorResponse(res: Response): Promise<VeldApiError> {
  const text = await res.text();
  try {
    const json = JSON.parse(text);
    return new VeldApiError(res.status, json.error ?? text, json.code ?? '');
  } catch {
    return new VeldApiError(res.status, text);
  }
}
`)
}

// emitBaseURL writes the BASE constant.
func emitBaseURL(sb *strings.Builder, opts emitter.EmitOptions) {
	if opts.BaseUrl != "" {
		sb.WriteString(fmt.Sprintf("\nconst BASE = '%s';\n", opts.BaseUrl))
	} else {
		// Use import.meta.env for Vite/browser, process.env for Node/SSR, empty string as fallback.
		sb.WriteString("\nconst BASE =\n")
		sb.WriteString("  (typeof import.meta !== 'undefined' && import.meta.env?.VITE_API_URL) ||\n")
		sb.WriteString("  (typeof process !== 'undefined' && process.env?.VELD_API_URL) ||\n")
		sb.WriteString("  '';\n")
	}
}

// emitHTTPHelpers writes the exported HTTP helpers so per-module files can import them.
// All methods are always emitted so per-module imports never fail.
func emitHTTPHelpers(sb *strings.Builder) {
	sb.WriteString(`
export async function get<T>(path: string): Promise<T> {
  const res = await fetch(BASE + path);
  if (!res.ok) throw await parseErrorResponse(res);
  return res.json();
}
`)

	bodyMethods := []string{"POST", "PUT", "PATCH", "DELETE"}
	for _, m := range bodyMethods {
		fn := strings.ToLower(m)
		if m == "DELETE" {
			fn = "del"
		}
		sb.WriteString(fmt.Sprintf(`
export async function %s<T>(path: string, body?: unknown): Promise<T> {
  const res = await fetch(BASE + path, {
    method: '%s',
    headers: { 'Content-Type': 'application/json' },
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  if (!res.ok) throw await parseErrorResponse(res);
  return res.json();
}
`, fn, m))
	}
}
