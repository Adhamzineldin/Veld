// Package typescript implements the Veld frontend emitter for TypeScript.
//
// It produces a single file:
//
//	client/api.ts — fetch-based SDK with VeldApiError, path params, all HTTP methods
//
// The emitter self-registers under the name "typescript" via init().
package typescript
