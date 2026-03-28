package typescript

import (
	"strings"
)

// emitVeldClientConfig writes the VeldClientConfig type and resolveBase helper.
func emitVeldClientConfig(sb *strings.Builder) {
	sb.WriteString(`
export type VeldClientConfig = {
  baseUrl?: string;
  headers?: Record<string, string>;
};

export function resolveBase(config?: VeldClientConfig | string): string {
  if (typeof config === 'string') return config;
  return config?.baseUrl ?? (typeof process !== 'undefined' ? (process.env['VELD_API_URL'] ?? '') : '');
}
`)
}

// emitErrorClass writes the VeldApiError class definition.
func emitErrorClass(sb *strings.Builder) {
	sb.WriteString(`
export class VeldApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly body: string,
    public readonly code?: string,
  ) {
    super(` + "`" + `HTTP ${status}: ${body}` + "`" + `);
    this.name = 'VeldApiError';
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
export function isErrorCode<T extends string>(err: unknown, code: T): err is VeldApiError & { code: T };
export function isErrorCode(err: unknown, code: string): err is VeldApiError;
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

// emitBuildQueryString writes the buildQueryString utility (used by module clients).
func emitBuildQueryString(sb *strings.Builder) {
	sb.WriteString(`
/**
 * Build a query string from an object, filtering out undefined and null values.
 * Returns '' if no valid params remain, otherwise returns '?key=value&...'.
 */
export function buildQueryString(params?: Record<string, unknown>): string {
  if (!params) return '';
  const filtered: Record<string, string> = {};
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null) {
      filtered[key] = String(value);
    }
  }
  const qs = new URLSearchParams(filtered).toString();
  return qs ? '?' + qs : '';
}
`)
}
