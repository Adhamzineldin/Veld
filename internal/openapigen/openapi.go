// Package openapigen builds an OpenAPI 3.0 spec from a Veld AST.
package openapigen

import (
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
)

// BuildSpec generates a complete OpenAPI 3.0.3 spec as a nested map.
func BuildSpec(a ast.AST) map[string]interface{} {
	modelMap := make(map[string]bool, len(a.Models))
	for _, m := range a.Models {
		modelMap[m.Name] = true
	}
	enumMap := make(map[string]ast.Enum, len(a.Enums))
	for _, en := range a.Enums {
		enumMap[en.Name] = en
	}

	paths := make(map[string]interface{})
	for _, mod := range a.Modules {
		tag := mod.Name
		for _, act := range mod.Actions {
			routePath := act.Path
			if mod.Prefix != "" {
				routePath = mod.Prefix + act.Path
			}
			oaPath := emitter.ToOpenAPIPath(routePath)
			pathParams := emitter.ExtractPathParams(routePath)

			responses := make(map[string]interface{})
			successCode := "200"
			if strings.ToUpper(act.Method) == "POST" {
				successCode = "201"
			}
			if strings.ToUpper(act.Method) == "DELETE" && act.Output == "" {
				successCode = "204"
			}

			if act.Output != "" {
				outputSchema := schemaRef(act.Output, act.OutputArray, modelMap)
				responses[successCode] = map[string]interface{}{
					"description": "Success",
					"content": map[string]interface{}{
						"application/json": map[string]interface{}{
							"schema": outputSchema,
						},
					},
				}
			} else {
				responses[successCode] = map[string]interface{}{"description": "Success"}
			}

			if act.Input != "" {
				responses["400"] = map[string]interface{}{"description": "Validation error"}
			}
			if len(act.Middleware) > 0 {
				responses["401"] = map[string]interface{}{"description": "Unauthorized"}
				responses["403"] = map[string]interface{}{"description": "Forbidden"}
			}
			if len(pathParams) > 0 {
				responses["404"] = map[string]interface{}{"description": "Not found"}
			}
			responses["500"] = map[string]interface{}{"description": "Internal server error"}

			op := map[string]interface{}{
				"tags":        []string{tag},
				"operationId": mod.Name + "_" + act.Name,
				"responses":   responses,
			}
			if act.Description != "" {
				op["summary"] = act.Description
			}
			if act.Deprecated != "" {
				op["deprecated"] = true
			}
			if len(pathParams) > 0 {
				params := make([]map[string]interface{}, 0, len(pathParams))
				for _, p := range pathParams {
					params = append(params, map[string]interface{}{
						"name": p, "in": "path", "required": true,
						"schema": map[string]interface{}{"type": "string"},
					})
				}
				op["parameters"] = params
			}
			if act.Input != "" {
				op["requestBody"] = map[string]interface{}{
					"required": true,
					"content": map[string]interface{}{
						"application/json": map[string]interface{}{
							"schema": map[string]interface{}{"$ref": "#/components/schemas/" + act.Input},
						},
					},
				}
			}
			method := strings.ToLower(act.Method)
			if _, ok := paths[oaPath]; !ok {
				paths[oaPath] = make(map[string]interface{})
			}
			paths[oaPath].(map[string]interface{})[method] = op
		}
	}

	schemas := make(map[string]interface{})
	for _, m := range a.Models {
		props := make(map[string]interface{})
		var required []string
		for _, f := range m.Fields {
			props[f.Name] = fieldSchema(f, modelMap, enumMap)
			if !f.Optional {
				required = append(required, f.Name)
			}
		}
		schema := map[string]interface{}{"type": "object", "properties": props}
		if len(required) > 0 {
			schema["required"] = required
		}
		if m.Description != "" {
			schema["description"] = m.Description
		}
		if m.Extends != "" {
			schema["allOf"] = []interface{}{
				map[string]interface{}{"$ref": "#/components/schemas/" + m.Extends},
				map[string]interface{}{"type": "object", "properties": props},
			}
			delete(schema, "type")
			delete(schema, "properties")
		}
		schemas[m.Name] = schema
	}
	for _, en := range a.Enums {
		s := map[string]interface{}{"type": "string", "enum": en.Values}
		if en.Description != "" {
			s["description"] = en.Description
		}
		schemas[en.Name] = s
	}

	return map[string]interface{}{
		"openapi": "3.0.3",
		"info": map[string]interface{}{
			"title": "Veld API", "version": "1.0.0",
			"description": "Auto-generated API specification from Veld contracts",
		},
		"paths":      paths,
		"components": map[string]interface{}{"schemas": schemas},
	}
}

func schemaRef(typeName string, isArray bool, models map[string]bool) map[string]interface{} {
	var base map[string]interface{}
	if models[typeName] {
		base = map[string]interface{}{"$ref": "#/components/schemas/" + typeName}
	} else {
		base = map[string]interface{}{"type": mapType(typeName)}
		if f := mapFormat(typeName); f != "" {
			base["format"] = f
		}
	}
	if isArray {
		return map[string]interface{}{"type": "array", "items": base}
	}
	return base
}

func fieldSchema(f ast.Field, models map[string]bool, enums map[string]ast.Enum) map[string]interface{} {
	if f.IsMap {
		valSchema := map[string]interface{}{"type": mapType(f.MapValueType)}
		if models[f.MapValueType] {
			valSchema = map[string]interface{}{"$ref": "#/components/schemas/" + f.MapValueType}
		}
		return map[string]interface{}{"type": "object", "additionalProperties": valSchema}
	}
	if en, ok := enums[f.Type]; ok {
		prop := map[string]interface{}{"type": "string", "enum": en.Values}
		if f.IsArray {
			return map[string]interface{}{"type": "array", "items": prop}
		}
		return prop
	}
	if models[f.Type] {
		ref := map[string]interface{}{"$ref": "#/components/schemas/" + f.Type}
		if f.IsArray {
			return map[string]interface{}{"type": "array", "items": ref}
		}
		return ref
	}
	prop := map[string]interface{}{"type": mapType(f.Type)}
	if ft := mapFormat(f.Type); ft != "" {
		prop["format"] = ft
	}
	if f.Default != "" {
		prop["default"] = f.Default
	}
	if f.Example != "" {
		prop["example"] = f.Example
	}
	if f.Deprecated != "" {
		prop["deprecated"] = true
	}
	if f.IsArray {
		return map[string]interface{}{"type": "array", "items": prop}
	}
	return prop
}

func mapType(t string) string {
	switch t {
	case "int":
		return "integer"
	case "float":
		return "number"
	case "bool":
		return "boolean"
	case "date", "datetime", "uuid", "string":
		return "string"
	default:
		return t
	}
}

func mapFormat(t string) string {
	switch t {
	case "date":
		return "date"
	case "datetime":
		return "date-time"
	case "uuid":
		return "uuid"
	case "int":
		return "int64"
	case "float":
		return "double"
	default:
		return ""
	}
}

// Type is exported for use by other packages.
func Type(t string) string   { return mapType(t) }
func Format(t string) string { return mapFormat(t) }
