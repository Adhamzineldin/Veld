// Package kotlin is the Veld Kotlin frontend emitter.
// Implementation is split across:
//   - emitter.go  — KotlinEmitter struct, init(), Summary(), Emit()
//   - types.go    — emitEnums(), emitDataClasses()
//   - client.go   — emitApiObject() (VeldApi object, HTTP + WebSocket methods)
//   - helpers.go  — type mapping, field helpers, collectAllFields, lcFirst
package kotlin
