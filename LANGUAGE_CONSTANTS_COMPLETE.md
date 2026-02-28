# VELD LANGUAGE CONSTANTS - ARCHITECTURE COMPLETE ✅

**Status:** Single Source of Truth Implemented  
**Date:** February 28, 2026

---

## 🎯 YOUR QUESTION ANSWERED

**"Should language constants be defined in plugins or source code?"**

### Answer: ✅ SOURCE CODE ONLY

Define once, auto-generate for plugins. Never duplicate.

---

## 🏗️ THE ARCHITECTURE

```
┌─────────────────────────────────────────────────────┐
│                                                       │
│   SINGLE SOURCE OF TRUTH (Go)                        │
│                                                       │
│   internal/language/constants.go                     │
│   ├─ Keywords: model, module, action, enum...       │
│   ├─ HttpMethods: GET, POST, PUT, DELETE, PATCH...  │
│   ├─ BuiltinTypes: string, int, float, bool...      │
│   ├─ Directives: method, path, input, output...     │
│   └─ SpecialTypes: List, Map                         │
│                                                       │
│   GetLanguageSpec() → Returns complete spec          │
│                                                       │
└─────────────────────┬─────────────────────────────────┘
                      │
        ┌─────────────┴─────────────┐
        │                           │
        ▼                           ▼
        
    Generator Tool            Backend Uses It
    cmd/generate-language/    internal/language/
    main.go                   veld.go
    (Creates files)           (Parses & validates)
    
    │
    ├─ Auto-generates TypeScript (VS Code)
    ├─ Auto-generates Kotlin (JetBrains)
    ├─ Auto-generates JSON (Config)
    └─ Auto-generates Go (Backend spec)
```

---

## 📁 FILES CREATED

### Source (ONE Definition)
✅ `internal/language/constants.go` - VeldLanguageSpec type
✅ `internal/language/veld.go` - Parser using spec

### Generator Tool
✅ `cmd/generate-language/main.go` - Creates everything below

### Auto-Generated (Don't edit!)
✅ `veld-language.json` - JSON spec
✅ `editors/vscode/src/veld-language-spec.ts` - TypeScript
✅ `editors/jetbrains/src/main/kotlin/.../VeldLanguageSpec.kt` - Kotlin

---

## 🚀 HOW TO USE

### Step 1: Edit Source (ONE FILE)
```go
// internal/language/constants.go
func GetLanguageSpec() *VeldLanguageSpec {
    return &VeldLanguageSpec{
        Keywords: []string{
            "model", "module", "action", "enum", "import", "extends",
        },
        HttpMethods: []string{
            "GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS",
        },
        BuiltinTypes: []string{
            "string", "int", "float", "bool", "date", "datetime", "uuid", "bytes", "json", "any",
        },
        // ... etc
    }
}
```

### Step 2: Run Generator
```bash
go run cmd/generate-language/main.go
```

**Output:**
```
✅ Generated veld-language.json
✅ Generated editors\vscode\src\veld-language-spec.ts
✅ Generated editors\jetbrains\src\main\kotlin\dev\veld\jetbrains\VeldLanguageSpec.kt
✅ Language files generated successfully
```

### Step 3: Everything Updates Automatically
- ✅ VS Code plugin
- ✅ JetBrains plugin
- ✅ Go backend
- ✅ JSON config

---

## ✨ WHAT IT FIXES

### Before (Your Problems)
```
❌ HEAD treated as Type
❌ Constants duplicated everywhere
❌ Manual updates to each plugin
❌ Easy to miss updates
❌ Different versions in different files
❌ Spaghetti maintenance nightmare
```

### After (Professional)
```
✅ HEAD recognized as HTTP Method
✅ Constants defined once
✅ Auto-generated everywhere
✅ Impossible to miss updates
✅ Always in sync
✅ Clean, maintainable architecture
```

---

## 🔄 WORKFLOW

### To add new HTTP method (example):

**1. Edit source:**
```bash
vim internal/language/constants.go
```

Add to HttpMethods:
```go
HttpMethods: []string{
    "GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS",
    "TRACE",  // ← Add new method
}
```

**2. Run generator:**
```bash
go run cmd/generate-language/main.go
```

**3. Done!**
- VS Code plugin has TRACE
- JetBrains plugin has TRACE
- Backend validates TRACE
- JSON config has TRACE

**No manual updates needed!**

---

## 💡 HOW IT PREVENTS THE "HEAD" PROBLEM

### Before (Manual Constants)
```typescript
// editors/vscode/src/extension.ts (hardcoded)
const HTTP_METHODS = ['GET', 'POST', 'PUT', 'DELETE', 'PATCH'];
// ❌ Forgot HEAD!

const BUILTIN_TYPES = ['string', 'int', 'HEAD', ...];  
// ❌ HEAD ends up as type instead!
```

### After (Auto-Generated)
```go
// internal/language/constants.go (source)
HttpMethods: []string{
    "GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS",
}
// ✅ Complete list, no mistakes
```

Auto-generated TypeScript:
```typescript
// editors/vscode/src/veld-language-spec.ts (generated)
export const HTTP_METHODS = new Set([
    "GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"
]);
// ✅ HEAD is always there, always correct
```

**Result:** HEAD NEVER treated as type because it comes from a single, authoritative source.

---

## 📊 CURRENT SPEC

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

## ✅ BENEFITS

| Aspect | Before | After |
|--------|--------|-------|
| **Definition Location** | 5+ places | 1 place |
| **Update Process** | Manual in each file | Automated |
| **Consistency** | Inconsistent | Always in sync |
| **Maintenance** | Nightmare | Simple |
| **Errors** | Easy to miss | Impossible |
| **Code Duplication** | Everywhere | Zero |
| **Professional** | No | ✅ YES |

---

## 🎓 CLEAN CODE PRINCIPLE

**Don't Repeat Yourself (DRY)**

Before: Constants duplicated everywhere  
After: Single source, auto-generated where needed

---

## 📚 RELATED FILES

- `LANGUAGE_CONSTANTS_ARCHITECTURE.md` - Detailed explanation
- `LANGUAGE_ARCHITECTURE_SOLUTION.md` - How it solves problems
- `internal/language/constants.go` - The source
- `cmd/generate-language/main.go` - The generator

---

## ✨ SUMMARY

**Single Source of Truth Architecture:**
- Define language spec once in Go
- Auto-generate for TypeScript (VS Code)
- Auto-generate for Kotlin (JetBrains)
- Auto-generate JSON for documentation
- Everything always in sync
- Zero code duplication
- Professional, clean, maintainable

**This is how professional projects handle language definitions.**


