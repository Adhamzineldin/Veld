/**
 * Veld Language Specification
 * AUTO-GENERATED - DO NOT EDIT
 * Generated from: internal/language/constants.go
 * Version: 1.0.0
 */

package dev.veld.jetbrains

object VeldLanguageSpec {
    const val VERSION = "1.0.0"
    
    val KEYWORDS = setOf("model", "module", "action", "enum", "constants", "constant", "import", "from", "extends")
    val HTTP_METHODS = setOf("GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS", "WS")
   val BUILTIN_TYPES = setOf("string", "int", "long", "float", "decimal", "bool", "date", "datetime", "time", "uuid", "bytes", "json", "any")
    val DIRECTIVES = setOf("description", "prefix", "method", "path", "input", "output", "response", "query", "headers", "stream", "emit", "middleware", "errors", "default")
    val SPECIAL_TYPES = setOf("List", "Map")
    val KNOWN_ANNOTATIONS = setOf("default", "unique", "required", "optional", "index", "primary", "autoincrement", "readonly", "deprecated", "example", "relation", "min", "max", "minLength", "maxLength", "regex")

    /** All valid keys for veld.config.json */
    val CONFIG_KEYS = mapOf(
        "\$schema"           to "JSON Schema reference for IDE autocompletion",
        "input"              to "Path to the main .veld entry file",
        "description"        to "Human/AI-readable project description",
        "backendConfig"      to "Nested backend configuration: { target, framework, out, dir, validate }",
        "frontendConfig"     to "Nested frontend configuration: { target, out, dir }",
        "backend"            to "Backend target (flat, deprecated): node, python, go, java, csharp, php, rust",
        "frontend"           to "Frontend SDK (flat, deprecated): react, vue, angular, svelte, typescript, dart, kotlin, swift, none",
        "out"                to "Output directory for generated code",
        "backendOut"         to "Deprecated — use backendConfig.out",
        "frontendOut"        to "Deprecated — use frontendConfig.out",
        "backendDir"         to "Deprecated — use backendConfig.dir",
        "frontendDir"        to "Deprecated — use frontendConfig.dir",
        "backendFramework"   to "Deprecated — use backendConfig.framework",
        "frontendFramework"  to "Deprecated — use frontendConfig.framework",
        "validate"           to "Generate runtime validators (prefer backendConfig.validate)",
        "baseUrl"            to "Base URL baked into generated SDK clients",
        "aliases"            to "Custom @alias → folder mappings",
        "services"           to "Module name → base URL override for multi-module APIs",
        "serverSdk"          to "Emit server-to-server typed SDK client",
        "tools"              to "Auxiliary generators: { openapi, dockerfile, cicd, database, scaffold, envconfig }",
        "hooks"              to "Lifecycle hooks: { postGenerate }",
        "postGenerate"       to "Deprecated — use hooks.postGenerate",
        "registry"           to "Cloud registry: { enabled, url, org, package, version }",
        "workspace"          to "Multi-service monorepo workspace entries"
    )
    
    fun isKeyword(word: String) = KEYWORDS.contains(word)
    fun isHttpMethod(word: String) = HTTP_METHODS.contains(word)
    fun isBuiltinType(word: String) = BUILTIN_TYPES.contains(word)
    fun isDirective(word: String) = DIRECTIVES.contains(word)
    fun isSpecialType(word: String) = SPECIAL_TYPES.contains(word)
}
