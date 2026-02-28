"use strict";
/**
 * Veld Language Specification
 * AUTO-GENERATED - DO NOT EDIT
 * Generated from: internal/language/constants.go
 * Version: 1.0.0
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.SPECIAL_TYPES = exports.DIRECTIVES = exports.BUILTIN_TYPES = exports.HTTP_METHODS = exports.KEYWORDS = exports.VELD_SPEC = void 0;
exports.VELD_SPEC = {
    version: "1.0.0",
    keywords: ["model", "module", "action", "enum", "import", "extends"],
    httpMethods: ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"],
    builtinTypes: ["string", "int", "float", "bool", "date", "datetime", "uuid", "bytes", "json", "any"],
    directives: ["description", "prefix", "method", "path", "input", "output", "default"],
    specialTypes: ["List", "Map"],
};
exports.KEYWORDS = new Set(exports.VELD_SPEC.keywords);
exports.HTTP_METHODS = new Set(exports.VELD_SPEC.httpMethods);
exports.BUILTIN_TYPES = new Set(exports.VELD_SPEC.builtinTypes);
exports.DIRECTIVES = new Set(exports.VELD_SPEC.directives);
exports.SPECIAL_TYPES = new Set(exports.VELD_SPEC.specialTypes);
//# sourceMappingURL=veld-language-spec.js.map