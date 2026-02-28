/**
 * Veld Language Specification
 * AUTO-GENERATED - DO NOT EDIT
 * Generated from: internal/language/constants.go
 * Version: 1.0.0
 */

package dev.veld.jetbrains

object VeldLanguageSpec {
    const val VERSION = "1.0.0"
    
    val KEYWORDS = setOf("model", "module", "action", "enum", "import", "extends")
    val HTTP_METHODS = setOf("GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS")
    val BUILTIN_TYPES = setOf("string", "int", "float", "bool", "date", "datetime", "uuid", "bytes", "json", "any")
    val DIRECTIVES = setOf("description", "prefix", "method", "path", "input", "output", "default")
    val SPECIAL_TYPES = setOf("List", "Map")
    val KNOWN_ANNOTATIONS = setOf(
        "default", "required", "min", "max", "minLength", "maxLength",
        "regex", "unique", "deprecated", "nullable", "index", "primaryKey"
    )

    fun isKeyword(word: String) = KEYWORDS.contains(word)
    fun isHttpMethod(word: String) = HTTP_METHODS.contains(word)
    fun isBuiltinType(word: String) = BUILTIN_TYPES.contains(word)
    fun isDirective(word: String) = DIRECTIVES.contains(word)
    fun isSpecialType(word: String) = SPECIAL_TYPES.contains(word)
}
