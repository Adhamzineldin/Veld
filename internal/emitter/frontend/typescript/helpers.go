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
		sb.WriteString("\nconst BASE = (typeof process !== 'undefined' && process.env?.VELD_API_URL) || '';\n")
	}
}

// emitHTTPHelpers writes the exported HTTP helpers so per-module files can import them.
func emitHTTPHelpers(sb *strings.Builder, methods map[string]bool) {
	if methods["GET"] {
		sb.WriteString(`
export async function get<T>(path: string): Promise<T> {
  const res = await fetch(BASE + path);
  if (!res.ok) throw await parseErrorResponse(res);
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
}
