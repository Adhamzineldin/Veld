// Package python is the Veld Python backend emitter (Flask + Pydantic).
// Implementation is split across:
//   - main.go       — PythonEmitter struct, init(), Summary(), Emit(), createDirs()
//   - types.go      — emitPerModuleTypes(), veldFieldToPy(), veldScalarToPy()
//   - interfaces.go — emitInterface(), formatPyOutputType()
//   - routes.go     — emitRoutes()
//   - schemas.go    — emitPydanticSchemas(), pyDefault()
package python
