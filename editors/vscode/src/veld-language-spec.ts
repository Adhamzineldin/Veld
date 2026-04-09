/**
 * Veld Language Specification
 * AUTO-GENERATED - DO NOT EDIT
 * Generated from: internal/language/constants.go
 * Version: 1.0.0
 */

export const VELD_SPEC = {
  version: "1.0.0",
  keywords: ["model", "module", "action", "enum", "constants", "import", "from", "extends"],
  httpMethods: ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"],
  builtinTypes: ["string", "int", "float", "decimal", "bool", "date", "datetime", "uuid", "bytes", "json", "any"],
  directives: ["description", "prefix", "method", "path", "input", "output", "query", "stream", "middleware", "errors", "default"],
  specialTypes: ["List", "Map"],
  annotations: ["default", "unique", "required", "optional", "index", "primary", "autoincrement", "readonly"],
  configKeys: {
    "$schema":          "JSON Schema reference for IDE autocompletion",
    input:              "Path to the main .veld entry file",
    description:        "Human/AI-readable project description",
    backendConfig:      "Nested backend configuration: { target, framework, out, dir, validate }",
    frontendConfig:     "Nested frontend configuration: { target, out, dir }",
    backend:            "Backend target (flat, deprecated): node, python, go, java, csharp, php, rust",
    frontend:           "Frontend SDK (flat, deprecated): react, vue, angular, svelte, typescript, dart, kotlin, swift, none",
    out:                "Output directory for generated code",
    backendOut:         "Deprecated — use backendConfig.out",
    frontendOut:        "Deprecated — use frontendConfig.out",
    backendDir:         "Deprecated — use backendConfig.dir",
    frontendDir:        "Deprecated — use frontendConfig.dir",
    backendFramework:   "Deprecated — use backendConfig.framework",
    frontendFramework:  "Deprecated — use frontendConfig.framework",
    validate:           "Generate runtime validators (prefer backendConfig.validate)",
    baseUrl:            "Base URL baked into generated SDK clients",
    aliases:            "Custom @alias → folder mappings",
    services:           "Module name → base URL override for multi-module APIs",
    serverSdk:          "Emit server-to-server typed SDK client",
    tools:              "Auxiliary generators: { openapi, dockerfile, cicd, database, scaffold, envconfig }",
    hooks:              "Lifecycle hooks: { postGenerate }",
    postGenerate:       "Deprecated — use hooks.postGenerate",
    registry:           "Cloud registry: { enabled, url, org, package, version }",
    workspace:          "Multi-service monorepo workspace entries",
  },
};

export const KEYWORDS = new Set(VELD_SPEC.keywords);
export const HTTP_METHODS = new Set(VELD_SPEC.httpMethods);
export const BUILTIN_TYPES = new Set(VELD_SPEC.builtinTypes);
export const DIRECTIVES = new Set(VELD_SPEC.directives);
export const SPECIAL_TYPES = new Set(VELD_SPEC.specialTypes);
export const ANNOTATIONS = new Set(VELD_SPEC.annotations);
export const CONFIG_KEYS = VELD_SPEC.configKeys;
