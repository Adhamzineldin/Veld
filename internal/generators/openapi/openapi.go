// Package openapi generates an OpenAPI 3.0 specification from a Veld AST.
//
// Registration happens via init() — blank-import in cmd/veld/main.go:
//
//	_ "github.com/Adhamzineldin/Veld/internal/emitter/openapi"
package openapi

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
)

func init() {
	emitter.RegisterTool("openapi", New())
}

// OpenAPIEmitter generates an OpenAPI 3.0 spec from a Veld AST.
type OpenAPIEmitter struct{}

func New() *OpenAPIEmitter      { return &OpenAPIEmitter{} }
func (*OpenAPIEmitter) IsTool() {}

// Summary returns a human-readable list of generated files.
func (e *OpenAPIEmitter) Summary(_ []string) []emitter.SummaryLine {
	return []emitter.SummaryLine{
		{Dir: "./", Files: "openapi.json"},
	}
}

// Emit writes openapi.json into outDir.
func (e *OpenAPIEmitter) Emit(a ast.AST, outDir string, opts emitter.EmitOptions) error {
	if opts.DryRun {
		return nil
	}
	if err := os.MkdirAll(outDir, 0755); err != nil {
		return err
	}

	spec := BuildSpec(a)
	data, err := json.MarshalIndent(spec, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal openapi: %w", err)
	}
	return os.WriteFile(filepath.Join(outDir, "openapi.json"), data, 0644)
}

// BuildSpec constructs the full OpenAPI 3.0 map from a Veld AST.
func BuildSpec(a ast.AST) map[string]interface{} {
	paths := buildPaths(a)
	schemas := buildSchemas(a)

	return map[string]interface{}{
		"openapi": "3.0.3",
		"info": map[string]interface{}{
			"title":       "Veld API",
			"description": "Auto-generated OpenAPI specification from Veld contracts",
			"version":     "1.0.0",
		},
		"paths": paths,
		"components": map[string]interface{}{
			"schemas": schemas,
		},
	}
}

func buildPaths(a ast.AST) map[string]interface{} {
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

			op := map[string]interface{}{
				"tags":        []string{tag},
				"operationId": mod.Name + "_" + act.Name,
			}
			if act.Description != "" {
				op["summary"] = act.Description
			}
			if mod.Description != "" {
				op["description"] = mod.Description
			}

			// Path parameters
			var params []map[string]interface{}
			for _, p := range pathParams {
				params = append(params, map[string]interface{}{
					"name":     p,
					"in":       "path",
					"required": true,
					"schema":   map[string]interface{}{"type": "string"},
				})
			}

			// Query parameters from query model
			if act.Query != "" {
				for _, m := range a.Models {
					if m.Name == act.Query {
						for _, f := range m.Fields {
							params = append(params, map[string]interface{}{
								"name":     f.Name,
								"in":       "query",
								"required": !f.Optional,
								"schema":   oaFieldSchema(f),
							})
						}
						break
					}
				}
			}

			if len(params) > 0 {
				op["parameters"] = params
			}

			// Request body
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

			// Responses
			responses := map[string]interface{}{}
			successCode := "200"
			if strings.ToUpper(act.Method) == "POST" {
				successCode = "201"
			}
			if strings.ToUpper(act.Method) == "DELETE" && act.Output == "" {
				successCode = "204"
			}

			if act.Output != "" {
				outputSchema := map[string]interface{}{"$ref": "#/components/schemas/" + act.Output}
				if act.OutputArray {
					outputSchema = map[string]interface{}{
						"type":  "array",
						"items": map[string]interface{}{"$ref": "#/components/schemas/" + act.Output},
					}
				}
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

			responses["400"] = map[string]interface{}{
				"description": "Bad Request",
				"content": map[string]interface{}{
					"application/json": map[string]interface{}{
						"schema": map[string]interface{}{"$ref": "#/components/schemas/ErrorResponse"},
					},
				},
			}
			responses["500"] = map[string]interface{}{
				"description": "Internal Server Error",
				"content": map[string]interface{}{
					"application/json": map[string]interface{}{
						"schema": map[string]interface{}{"$ref": "#/components/schemas/ErrorResponse"},
					},
				},
			}

			op["responses"] = responses

			method := strings.ToLower(act.Method)
			if _, ok := paths[oaPath]; !ok {
				paths[oaPath] = make(map[string]interface{})
			}
			paths[oaPath].(map[string]interface{})[method] = op
		}
	}

	return paths
}

func buildSchemas(a ast.AST) map[string]interface{} {
	schemas := make(map[string]interface{})

	// ErrorResponse schema (standard)
	schemas["ErrorResponse"] = map[string]interface{}{
		"type":        "object",
		"description": "Standard error response",
		"properties": map[string]interface{}{
			"code":    map[string]interface{}{"type": "integer"},
			"message": map[string]interface{}{"type": "string"},
			"details": map[string]interface{}{"type": "string"},
		},
		"required": []string{"code", "message"},
	}

	// Enums
	for _, en := range a.Enums {
		s := map[string]interface{}{
			"type": "string",
			"enum": en.Values,
		}
		if en.Description != "" {
			s["description"] = en.Description
		}
		schemas[en.Name] = s
	}

	// Models
	for _, m := range a.Models {
		props := make(map[string]interface{})
		var required []string

		for _, f := range m.Fields {
			props[f.Name] = oaFieldSchema(f)
			if !f.Optional {
				required = append(required, f.Name)
			}
		}

		schema := map[string]interface{}{
			"type":       "object",
			"properties": props,
		}
		if len(required) > 0 {
			schema["required"] = required
		}
		if m.Description != "" {
			schema["description"] = m.Description
		}
		if m.Extends != "" {
			schema["allOf"] = []interface{}{
				map[string]interface{}{"$ref": "#/components/schemas/" + m.Extends},
				map[string]interface{}{
					"type":       "object",
					"properties": props,
				},
			}
			delete(schema, "type")
			delete(schema, "properties")
		}

		schemas[m.Name] = schema
	}

	return schemas
}

func oaFieldSchema(f ast.Field) map[string]interface{} {
	if f.IsMap {
		return map[string]interface{}{
			"type":                 "object",
			"additionalProperties": oaTypeSchema(f.MapValueType),
		}
	}
	if f.IsArray {
		return map[string]interface{}{
			"type":  "array",
			"items": oaTypeSchema(f.Type),
		}
	}
	return oaTypeSchema(f.Type)
}

func oaTypeSchema(t string) map[string]interface{} {
	switch t {
	case "string":
		return map[string]interface{}{"type": "string"}
	case "int":
		return map[string]interface{}{"type": "integer", "format": "int32"}
	case "float":
		return map[string]interface{}{"type": "number", "format": "double"}
	case "decimal":
		return map[string]interface{}{"type": "string", "format": "decimal"}
	case "bool":
		return map[string]interface{}{"type": "boolean"}
	case "date":
		return map[string]interface{}{"type": "string", "format": "date"}
	case "datetime":
		return map[string]interface{}{"type": "string", "format": "date-time"}
	case "uuid":
		return map[string]interface{}{"type": "string", "format": "uuid"}
	default:
		// Model/enum reference
		return map[string]interface{}{"$ref": "#/components/schemas/" + t}
	}
}
