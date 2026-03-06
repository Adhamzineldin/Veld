// Package nest is the Veld NestJS backend emitter.
// Implementation is split across:
//   - main.go       — NestEmitter struct, init(), Summary(), Emit()
//   - types.go      — emitPerModuleTypes()
//   - interfaces.go — emitInterface()
//   - routes.go     — emitControllers()
//   - barrel.go     — emitBarrel() (index.ts + package.json)
package nest
