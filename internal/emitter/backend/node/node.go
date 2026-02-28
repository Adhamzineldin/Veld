// Package node is the Veld Node.js backend emitter.
// Implementation is split across:
//   - main.go       — NodeEmitter struct, init(), Summary(), Emit()
//   - types.go      — emitPerModuleTypes()
//   - interfaces.go — emitInterface()
//   - routes.go     — emitRoutes()
//   - schemas.go    — emitZodSchemas() + Zod helpers
//   - barrel.go     — emitBarrel() (index.ts + package.json)
package node
