/**
 * Veld Language Specification
 * AUTO-GENERATED - DO NOT EDIT
 * Generated from: internal/language/constants.go
 * Version: 1.0.0
 */

export const VELD_SPEC = {
  version: "1.0.0",
  keywords: ["model", "module", "action", "enum", "import", "from", "extends"],
  httpMethods: ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"],
  builtinTypes: ["string", "int", "float", "bool", "date", "datetime", "uuid", "bytes", "json", "any"],
  directives: ["description", "prefix", "method", "path", "input", "output", "query", "stream", "middleware", "errors", "default"],
  specialTypes: ["List", "Map"],
  annotations: ["default", "unique", "required", "optional", "index", "primary", "autoincrement", "readonly"],
  configKeys: {
    input:              "Path to the main .veld entry file",
    backend:            "Backend target: node, python, go, java, csharp, php, rust",
    frontend:           "Frontend SDK: react, vue, angular, svelte, typescript, dart, kotlin, swift, none",
    out:                "Output directory for generated code",
    backendDir:         "Path to backend project directory (for setup patching)",
    backendDirectory:   "Alias for backendDir",
    frontendDir:        "Path to frontend project directory (for setup patching)",
    frontendDirectory:  "Alias for frontendDir",
    baseUrl:            "Base URL baked into the frontend SDK",
    aliases:            "Custom @alias → folder mappings",
  },
};

export const KEYWORDS = new Set(VELD_SPEC.keywords);
export const HTTP_METHODS = new Set(VELD_SPEC.httpMethods);
export const BUILTIN_TYPES = new Set(VELD_SPEC.builtinTypes);
export const DIRECTIVES = new Set(VELD_SPEC.directives);
export const SPECIAL_TYPES = new Set(VELD_SPEC.specialTypes);
export const ANNOTATIONS = new Set(VELD_SPEC.annotations);
export const CONFIG_KEYS = VELD_SPEC.configKeys;
