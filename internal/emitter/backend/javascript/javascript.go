// Package javascript is the Veld JavaScript backend emitter.
// It mirrors the Node emitter but outputs .js files with JSDoc annotations
// instead of TypeScript syntax.
//
// Implementation is split across:
//   - main.go       — JSEmitter struct, init(), Summary(), Emit()
//   - types.go      — emitPerModuleTypes()
//   - interfaces.go — emitInterface()
//   - routes.go     — emitRoutes()
//   - errors.go     — emitErrors()
//   - validate.go   — emitValidators()
//   - barrel.go     — emitBarrel() (index.js + package.json)
package javascript
