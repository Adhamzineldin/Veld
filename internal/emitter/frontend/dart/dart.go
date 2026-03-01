// Package dart is the Veld Dart/Flutter frontend emitter.
// Implementation is split across:
//   - emitter.go  — DartEmitter struct, init(), Summary(), Emit()
//   - types.go    — emitEnums(), emitModels()
//   - client.go   — emitApiClass() (VeldApi class, HTTP + WebSocket methods)
//   - helpers.go  — type mapping, field helpers, collectAllFields, lcFirst
package dart
