// Package javascript implements the Veld frontend emitter for plain JavaScript.
//
// It mirrors the TypeScript frontend emitter but outputs .js files with JSDoc
// annotations instead of TypeScript syntax. Uses native fetch, zero dependencies.
//
// The emitter self-registers under the name "javascript" via init().
package javascript
