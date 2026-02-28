# VELD LANGUAGE CONSTANTS - SINGLE SOURCE OF TRUTH

**Problem Solved:** No more duplicate language definitions across codebase  
**Date:** February 28, 2026

---

## 🎯 THE ARCHITECTURE

```
┌─────────────────────────────────────────────────────┐
│                                                       │
│   internal/language/constants.go                     │
│   (SINGLE SOURCE OF TRUTH)                           │
│                                                       │
│   - KEYWORDS                                         │
│   - HTTP_METHODS                                     │
│   - BUILTIN_TYPES                                    │
│   - DIRECTIVES                                       │
│   - SPECIAL_TYPES                                    │
│                                                       │
└───────────────────┬─────────────────────────────────┘
                    │
        ┌───────────┼───────────┬──────────────┐
        │           │           │              │
        ▼           ▼           ▼              ▼
    
    Run: go run cmd/generate-language/main.go
    
    │           │           │              │
    ▼           ▼           ▼              ▼
    
┌─────────────┬──────────────┬──────────────┬──────────────┐
│             │              │              │              │
│ TypeScript  │   Kotlin     │     JSON     │      Go      │
│ (VS Code)   │ (JetBrains)  │  (Config)    │  (Backend)   │
│             │              │              │              │
└─────────────┴──────────────┴──────────────┴──────────────┘
```

---

## 📁 FILE STRUCTURE

### Source (One Definition)
```
internal/language/
├── constants.go      ✅ SINGLE SOURCE OF TRUTH
└── veld.go          (Uses GetLanguageSpec())
```

### Generated Files (Auto-generated)
```
editors/vscode/src/
├── veld-language-spec.ts   ✅ AUTO-GENERATED (TypeScript)
└── extension.ts            (Imports from above)

editors/jetbrains/src/main/kotlin/dev/veld/jetbrains/
├── VeldLanguageSpec.kt     ✅ AUTO-GENERATED (Kotlin)
└── VeldSyntaxHighlighter.kt (Uses above)

veld-language.json          ✅ AUTO-GENERATED (JSON)
```

### Generator Tool
```
cmd/generate-language/
└── main.go                 ✅ Generates all files
```

---

## 🔄 HOW TO UPDATE

### To add a new HTTP method:

**Step 1:** Edit ONE file
```go
// internal/language/constants.go
func GetLanguageSpec() *VeldLanguageSpec {
    return &VeldLanguageSpec{
        HttpMethods: []string{
            "GET",
            "POST",
            "PUT",
            "DELETE",
            "PATCH",
            "HEAD",
            "OPTIONS",
            "TRACE",  // ← ADD HERE
        },
        // ...
    }
}
```

**Step 2:** Run generator
```bash
go run cmd/generate-language/main.go
```

**Step 3:** Everything updates automatically!
- ✅ VS Code plugin gets new method
- ✅ JetBrains plugin gets new method
- ✅ Go backend gets new method
- ✅ JSON config gets new method

---

## 📝 WHAT EACH FILE CONTAINS

### `internal/language/constants.go`
```go
// THE DEFINITION
type VeldLanguageSpec struct {
    Keywords     []string
    HttpMethods  []string
    BuiltinTypes []string
    Directives   []string
    SpecialTypes []string
}

// Helper methods
func (spec *VeldLanguageSpec) IsKeyword(word string) bool
func (spec *VeldLanguageSpec) IsHttpMethod(word string) bool
func (spec *VeldLanguageSpec) IsBuiltinType(word string) bool
// ... etc
```

### `veld-language-spec.ts` (Generated)
```typescript
// Auto-generated TypeScript sets
export const KEYWORDS = new Set(['model', 'module', ...])
export const HTTP_METHODS = new Set(['GET', 'POST', ...])
export const BUILTIN_TYPES = new Set(['string', 'int', ...])
export const DIRECTIVES = new Set(['method', 'path', ...])
export const SPECIAL_TYPES = new Set(['List', 'Map'])
```

### `VeldLanguageSpec.kt` (Generated)
```kotlin
// Auto-generated Kotlin object
object VeldLanguageSpec {
    val KEYWORDS = setOf("model", "module", ...)
    val HTTP_METHODS = setOf("GET", "POST", ...)
    val BUILTIN_TYPES = setOf("string", "int", ...)
    
    fun isKeyword(word: String) = KEYWORDS.contains(word)
    fun isHttpMethod(word: String) = HTTP_METHODS.contains(word)
}
```

### `veld-language.json` (Generated)
```json
{
  "version": "1.0.0",
  "keywords": ["model", "module", ...],
  "httpMethods": ["GET", "POST", ...],
  "builtinTypes": ["string", "int", ...],
  "directives": ["method", "path", ...],
  "specialTypes": ["List", "Map"]
}
```

---

## ✅ HOW VS CODE PLUGIN USES IT

```typescript
// editors/vscode/src/extension.ts

// Import the auto-generated constants
import { KEYWORDS, HTTP_METHODS, BUILTIN_TYPES, DIRECTIVES } from './veld-language-spec';

// Use them in the language server
class VeldLanguageServer {
    validateDocument() {
        // Uses HTTP_METHODS from generated file
        if (!HTTP_METHODS.has(methodName)) {
            // Error: invalid method
        }
    }
    
    getCompletions() {
        // Uses BUILTIN_TYPES from generated file
        for (const type of BUILTIN_TYPES) {
            completions.push(type);
        }
    }
}
```

---

## ✅ HOW JETBRAINS PLUGIN USES IT

```kotlin
// editors/jetbrains/src/main/kotlin/.../VeldLexer.kt

// Import the auto-generated spec
import dev.veld.jetbrains.VeldLanguageSpec

class VeldLexer {
    fun tokenize() {
        when {
            // Uses HTTP_METHODS from generated file
            VeldLanguageSpec.isHttpMethod(word) -> token = HTTP_METHOD_TOKEN
            // Uses KEYWORDS from generated file
            VeldLanguageSpec.isKeyword(word) -> token = KEYWORD_TOKEN
            // etc
        }
    }
}
```

---

## 🚀 BENEFITS

### Before (No Single Source)
```
❌ Keyword defined in 3 places
❌ Update Go, update VS Code, update JetBrains
❌ Easy to miss updates (bugs!)
❌ Different versions in different places
❌ Spaghetti maintenance
```

### After (Single Source)
```
✅ Defined once in Go
✅ Auto-generated everywhere
✅ All tools always in sync
✅ Update once, deploy everywhere
✅ Zero maintenance
```

---

## 🔧 WORKFLOW

### When adding new language feature:

1. **Edit ONE file**
   ```bash
   vim internal/language/constants.go
   ```

2. **Run generator**
   ```bash
   go run cmd/generate-language/main.go
   ```

3. **Commit everything**
   ```bash
   git add internal/language/constants.go
   git add editors/vscode/src/veld-language-spec.ts
   git add editors/jetbrains/src/main/kotlin/.../VeldLanguageSpec.kt
   git add veld-language.json
   git commit -m "Add new language feature"
   ```

**No manual updates to plugins!**

---

## 📊 CURRENT LANGUAGE SPEC

**Version:** 1.0.0

**Keywords:** (6)
- model, module, action, enum, import, extends

**HTTP Methods:** (7)
- GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS

**Built-in Types:** (10)
- string, int, float, bool, date, datetime, uuid, bytes, json, any

**Directives:** (7)
- description, prefix, method, path, input, output, default

**Special Types:** (2)
- List, Map

---

## 🎯 ADDING TO CI/CD

### GitHub Actions (.github/workflows/update-language.yml)

```yaml
name: Update Language Spec

on:
  push:
    paths:
      - 'internal/language/constants.go'

jobs:
  generate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-go@v4
        with:
          go-version: 1.21
      
      - name: Generate language files
        run: go run cmd/generate-language/main.go
      
      - name: Commit changes
        run: |
          git config user.name "Veld Bot"
          git config user.email "bot@veld.dev"
          git add editors/vscode/src/veld-language-spec.ts
          git add editors/jetbrains/src/main/kotlin/.../VeldLanguageSpec.kt
          git add veld-language.json
          git commit -m "Update language spec [skip ci]"
          git push
```

**Benefit:** Language spec auto-updates across all files on every commit!

---

## ✨ SUMMARY

**Single Source of Truth Architecture:**
- ✅ One definition (Go)
- ✅ Auto-generated for all tools
- ✅ Always in sync
- ✅ Zero maintenance
- ✅ Easy to extend

**Never duplicate language definitions again!**


