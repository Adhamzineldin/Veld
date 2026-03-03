/**
 * Veld Language Specification
 * AUTO-GENERATED - DO NOT EDIT
 * Generated from: internal/language/constants.go
 * Version: 1.0.0
 */

package dev.veld.jetbrains

object VeldLanguageSpec {
    const val VERSION = "1.0.0"
    
    val KEYWORDS = setOf("model", "module", "action", "enum", "import", "from", "extends")
    val HTTP_METHODS = setOf("GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS")
    val BUILTIN_TYPES = setOf("string", "int", "float", "bool", "date", "datetime", "uuid", "bytes", "json", "any")
    val DIRECTIVES = setOf("description", "prefix", "method", "path", "input", "output", "query", "stream", "middleware", "errors", "default")
    val SPECIAL_TYPES = setOf("List", "Map")
    val KNOWN_ANNOTATIONS = setOf("default", "unique", "required", "optional", "index", "primary", "autoincrement", "readonly")

    /** All valid keys for veld.config.json */
    val CONFIG_KEYS = mapOf(
        "input"              to "Path to the main .veld entry file (e.g. \"app.veld\")",
        "backend"            to "Backend target: node, python, go, java, csharp, php, rust",
        "frontend"           to "Frontend SDK: react, vue, angular, svelte, typescript, dart, kotlin, swift, none",
        "out"                to "Output directory for generated code (e.g. \"../generated\")",
        "backendDir"         to "Path to backend project directory (for setup patching)",
        "backendDirectory"   to "Alias for backendDir",
        "frontendDir"        to "Path to frontend project directory (for setup patching)",
        "frontendDirectory"  to "Alias for frontendDir",
        "baseUrl"            to "Base URL baked into the frontend SDK (e.g. \"/api/v1\")",
        "aliases"            to "Custom @alias → folder mappings (e.g. { \"auth\": \"services/auth\" })"
    )
    
    fun isKeyword(word: String) = KEYWORDS.contains(word)
    fun isHttpMethod(word: String) = HTTP_METHODS.contains(word)
    fun isBuiltinType(word: String) = BUILTIN_TYPES.contains(word)
    fun isDirective(word: String) = DIRECTIVES.contains(word)
    fun isSpecialType(word: String) = SPECIAL_TYPES.contains(word)
}
