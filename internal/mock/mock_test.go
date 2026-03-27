package mock

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

func minimalAST() ast.AST {
	return ast.AST{
		Models: []ast.Model{
			{
				Name: "User",
				Fields: []ast.Field{
					{Name: "id", Type: "uuid"},
					{Name: "name", Type: "string"},
					{Name: "age", Type: "int"},
					{Name: "score", Type: "float"},
					{Name: "active", Type: "bool"},
					{Name: "createdAt", Type: "datetime"},
					{Name: "birthDate", Type: "date"},
					{Name: "tags", Type: "string", IsArray: true},
					{Name: "metadata", Type: "string", IsMap: true, MapValueType: "string"},
				},
			},
		},
		Enums: []ast.Enum{
			{Name: "Role", Values: []string{"admin", "user", "guest"}},
		},
		Modules: []ast.Module{
			{
				Name:   "Users",
				Prefix: "/api",
				Actions: []ast.Action{
					{Name: "GetUsers", Method: "GET", Path: "/users", Output: "User", OutputArray: true},
					{Name: "GetUser", Method: "GET", Path: "/users/:id", Output: "User"},
					{Name: "CreateUser", Method: "POST", Path: "/users", Input: "User", Output: "User"},
					{Name: "DeleteUser", Method: "DELETE", Path: "/users/:id"},
				},
			},
		},
	}
}

func TestExampleValuePrimitives(t *testing.T) {
	a := minimalAST()

	tests := []struct {
		typeName string
		want     interface{}
	}{
		{"string", "example-value"},
		{"int", 42},
		{"float", 9.99},
		{"bool", true},
		{"uuid", "550e8400-e29b-41d4-a716-446655440000"},
		{"date", "2026-03-08"},
		{"datetime", "2026-03-08T12:00:00Z"},
	}

	for _, tt := range tests {
		got := exampleValue(a, tt.typeName, 0)
		if got != tt.want {
			t.Errorf("exampleValue(%q) = %v, want %v", tt.typeName, got, tt.want)
		}
	}
}

func TestExampleValueEnum(t *testing.T) {
	a := minimalAST()
	got := exampleValue(a, "Role", 0)
	if got != "admin" {
		t.Errorf("exampleValue(Role) = %v, want admin", got)
	}
}

func TestExampleValueModel(t *testing.T) {
	a := minimalAST()
	got := exampleValue(a, "User", 0)
	obj, ok := got.(map[string]interface{})
	if !ok {
		t.Fatalf("exampleValue(User) is not a map, got %T", got)
	}
	if obj["id"] != "550e8400-e29b-41d4-a716-446655440000" {
		t.Errorf("User.id = %v, want uuid", obj["id"])
	}
	if obj["name"] != "example-value" {
		t.Errorf("User.name = %v, want example-value", obj["name"])
	}
	if obj["age"] != 42 {
		t.Errorf("User.age = %v, want 42", obj["age"])
	}
	// Check array field.
	tags, ok := obj["tags"].([]interface{})
	if !ok || len(tags) != 1 {
		t.Errorf("User.tags should be array with 1 element, got %v", obj["tags"])
	}
	// Check map field.
	meta, ok := obj["metadata"].(map[string]interface{})
	if !ok {
		t.Errorf("User.metadata should be a map, got %T", obj["metadata"])
	} else if _, hasKey := meta["key"]; !hasKey {
		t.Errorf("User.metadata should have 'key' entry")
	}
}

func TestExampleValueExtends(t *testing.T) {
	a := ast.AST{
		Models: []ast.Model{
			{Name: "Base", Fields: []ast.Field{{Name: "id", Type: "uuid"}}},
			{Name: "Child", Extends: "Base", Fields: []ast.Field{{Name: "name", Type: "string"}}},
		},
	}
	got := exampleValue(a, "Child", 0)
	obj, ok := got.(map[string]interface{})
	if !ok {
		t.Fatalf("exampleValue(Child) is not a map")
	}
	if _, hasID := obj["id"]; !hasID {
		t.Error("Child should inherit 'id' from Base")
	}
	if _, hasName := obj["name"]; !hasName {
		t.Error("Child should have own field 'name'")
	}
}

func TestExampleValueDepthLimit(t *testing.T) {
	a := ast.AST{
		Models: []ast.Model{
			{Name: "Node", Fields: []ast.Field{{Name: "child", Type: "Node"}}},
		},
	}
	got := exampleValue(a, "Node", 0)
	// Should not panic; depth limit prevents infinite recursion.
	if got == nil {
		t.Error("exampleValue(Node, 0) should not be nil at depth 0")
	}
}

func TestBuildRoutes(t *testing.T) {
	a := minimalAST()
	routes := buildRoutes(a)
	if len(routes) != 4 {
		t.Fatalf("expected 4 routes, got %d", len(routes))
	}

	expected := []struct {
		method string
		path   string
	}{
		{"GET", "/api/users"},
		{"GET", "/api/users/:id"},
		{"POST", "/api/users"},
		{"DELETE", "/api/users/:id"},
	}

	for i, e := range expected {
		if routes[i].method != e.method {
			t.Errorf("route[%d] method = %s, want %s", i, routes[i].method, e.method)
		}
		gotPath := "/" + joinParts(routes[i].parts)
		if gotPath != e.path {
			t.Errorf("route[%d] path = %s, want %s", i, gotPath, e.path)
		}
	}
}

func TestBuildRoutesSkipsWebSocket(t *testing.T) {
	a := ast.AST{
		Modules: []ast.Module{
			{
				Name:   "Chat",
				Prefix: "/ws",
				Actions: []ast.Action{
					{Name: "Connect", Method: "WS", Path: "/chat"},
					{Name: "GetHistory", Method: "GET", Path: "/history"},
				},
			},
		},
	}
	routes := buildRoutes(a)
	if len(routes) != 1 {
		t.Fatalf("expected 1 route (WS skipped), got %d", len(routes))
	}
	if routes[0].method != "GET" {
		t.Errorf("expected GET, got %s", routes[0].method)
	}
}

func TestMatchRoute(t *testing.T) {
	rt := route{method: "GET", parts: splitPath("/api/users/:id")}

	if !matchRoute(rt, "GET", "/api/users/123") {
		t.Error("should match /api/users/123")
	}
	if matchRoute(rt, "POST", "/api/users/123") {
		t.Error("should not match POST method")
	}
	if matchRoute(rt, "GET", "/api/users") {
		t.Error("should not match /api/users (missing segment)")
	}
	if matchRoute(rt, "GET", "/api/users/123/extra") {
		t.Error("should not match extra segments")
	}
}

func TestMockHandlerGET(t *testing.T) {
	a := minimalAST()
	routes := buildRoutes(a)

	// GET /api/users → 200 with array
	req := httptest.NewRequest("GET", "/api/users", nil)
	w := httptest.NewRecorder()
	for _, rt := range routes {
		if matchRoute(rt, "GET", "/api/users") {
			rt.handler(w, req)
			break
		}
	}
	if w.Code != http.StatusOK {
		t.Errorf("GET /api/users status = %d, want 200", w.Code)
	}
	var arr []interface{}
	if err := json.Unmarshal(w.Body.Bytes(), &arr); err != nil {
		t.Fatalf("response is not a JSON array: %v", err)
	}
	if len(arr) != 1 {
		t.Errorf("expected array of 1 element, got %d", len(arr))
	}
}

func TestMockHandlerPOST(t *testing.T) {
	a := minimalAST()
	routes := buildRoutes(a)

	req := httptest.NewRequest("POST", "/api/users", nil)
	w := httptest.NewRecorder()
	for _, rt := range routes {
		if matchRoute(rt, "POST", "/api/users") {
			rt.handler(w, req)
			break
		}
	}
	if w.Code != http.StatusCreated {
		t.Errorf("POST /api/users status = %d, want 201", w.Code)
	}
	var obj map[string]interface{}
	if err := json.Unmarshal(w.Body.Bytes(), &obj); err != nil {
		t.Fatalf("response is not a JSON object: %v", err)
	}
}

func TestMockHandlerDELETE(t *testing.T) {
	a := minimalAST()
	routes := buildRoutes(a)

	req := httptest.NewRequest("DELETE", "/api/users/42", nil)
	w := httptest.NewRecorder()
	for _, rt := range routes {
		if matchRoute(rt, "DELETE", "/api/users/42") {
			rt.handler(w, req)
			break
		}
	}
	if w.Code != http.StatusNoContent {
		t.Errorf("DELETE /api/users/:id status = %d, want 204", w.Code)
	}
	if w.Body.Len() != 0 {
		t.Errorf("DELETE with no output should have empty body, got %q", w.Body.String())
	}
}

func TestMockHandlerGETSingle(t *testing.T) {
	a := minimalAST()
	routes := buildRoutes(a)

	req := httptest.NewRequest("GET", "/api/users/abc", nil)
	w := httptest.NewRecorder()
	for _, rt := range routes {
		if matchRoute(rt, "GET", "/api/users/abc") {
			rt.handler(w, req)
			break
		}
	}
	if w.Code != http.StatusOK {
		t.Errorf("GET /api/users/:id status = %d, want 200", w.Code)
	}
	var obj map[string]interface{}
	if err := json.Unmarshal(w.Body.Bytes(), &obj); err != nil {
		t.Fatalf("response is not a JSON object: %v", err)
	}
	if obj["id"] != "550e8400-e29b-41d4-a716-446655440000" {
		t.Errorf("expected uuid for id field")
	}
}

// joinParts is a test helper that joins path parts with "/".
func joinParts(parts []string) string {
	result := ""
	for i, p := range parts {
		if i > 0 {
			result += "/"
		}
		result += p
	}
	return result
}
