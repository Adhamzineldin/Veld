/**
 * Veld Language Specification
 * AUTO-GENERATED - DO NOT EDIT
 * Generated from: internal/language/constants.go
 * Version: 1.0.0
 */

export const VELD_SPEC = {
  version: "1.0.0",
  keywords: ["model", "module", "action", "enum", "import", "extends"],
  httpMethods: ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"],
  builtinTypes: ["string", "int", "float", "bool", "date", "datetime", "uuid", "bytes", "json", "any"],
  directives: ["description", "prefix", "method", "path", "input", "output", "default"],
  specialTypes: ["List", "Map"],
};

export const KEYWORDS = new Set(VELD_SPEC.keywords);
export const HTTP_METHODS = new Set(VELD_SPEC.httpMethods);
export const BUILTIN_TYPES = new Set(VELD_SPEC.builtinTypes);
export const DIRECTIVES = new Set(VELD_SPEC.directives);
export const SPECIAL_TYPES = new Set(VELD_SPEC.specialTypes);
