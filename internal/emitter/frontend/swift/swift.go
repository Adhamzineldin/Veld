// Package swift is the Veld Swift frontend emitter.
// Implementation is split across:
//   - emitter.go  — SwiftEmitter struct, init(), Summary(), Emit()
//   - types.go    — emitEnums(), emitStructs()
//   - client.go   — emitApiEnum() (VeldApi namespace, HTTP + WebSocket methods)
//   - helpers.go  — type mapping, field helpers, collectAllFields, lcFirst
package swift
