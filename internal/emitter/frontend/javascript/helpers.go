package javascript

import (
	"fmt"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/emitter"
)

// emitErrorClass writes the VeldApiError class in plain JavaScript.
func emitErrorClass(sb *strings.Builder) {
	sb.WriteString(`
class VeldApiError extends Error {
  /**
   * @param {number} status
   * @param {string} body
   * @param {string} [code]
   */
  constructor(status, body, code) {
    super(` + "`" + `Veld API error ${status}: ${body}` + "`" + `);
    this.name = 'VeldApiError';
    /** @type {number} */
    this.status = status;
    /** @type {string} */
    this.body = body;
    /** @type {string} */
    this.code = code ?? '';
  }
}

/**
 * Type guard — checks if value is a VeldApiError.
 * @param {*} err
 * @returns {boolean}
 */
function isApiError(err) {
  return err instanceof VeldApiError;
}

/**
 * Check if an error matches a specific error code.
 * @param {*} err
 * @param {string} code
 * @returns {boolean}
 */
function isErrorCode(err, code) {
  return err instanceof VeldApiError && err.code === code;
}

/**
 * @param {Response} res
 * @returns {Promise<VeldApiError>}
 */
async function parseErrorResponse(res) {
  const text = await res.text();
  try {
    const json = JSON.parse(text);
    return new VeldApiError(res.status, json.error ?? text, json.code ?? '');
  } catch (_) {
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

// emitHTTPHelpers writes the HTTP helper functions and module.exports.
func emitHTTPHelpers(sb *strings.Builder, methods map[string]bool) {
	sb.WriteString(`
/**
 * Build a query string from an object, filtering out undefined and null values.
 * Returns '' if no valid params remain, otherwise returns '?key=value&...'.
 * @param {Record<string, *>} [params]
 * @returns {string}
 */
function buildQueryString(params) {
  if (!params) return '';
  const filtered = {};
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null) {
      filtered[key] = String(value);
    }
  }
  const qs = new URLSearchParams(filtered).toString();
  return qs ? '?' + qs : '';
}
`)

	if methods["GET"] {
		sb.WriteString(`
/**
 * @template T
 * @param {string} path
 * @returns {Promise<T>}
 */
async function get(path) {
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
/**
 * @template T
 * @param {string} path
 * @param {*} [body]
 * @returns {Promise<T>}
 */
async function %s(path, body) {
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

	// CommonJS exports for all helpers.
	sb.WriteString("\nmodule.exports = { VeldApiError, isApiError, isErrorCode, buildQueryString, get, post, put, patch, del };\n")
}
