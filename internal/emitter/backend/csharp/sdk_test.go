package csharp

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
)

func testSdkAST() ast.AST {
	return ast.AST{
		Models: []ast.Model{
			{Name: "User", Fields: []ast.Field{{Name: "id", Type: "uuid"}, {Name: "email", Type: "string"}}},
			{Name: "LoginInput", Fields: []ast.Field{{Name: "email", Type: "string"}, {Name: "password", Type: "string"}}},
			{Name: "TokenPair", Fields: []ast.Field{{Name: "accessToken", Type: "string"}}},
		},
		Modules: []ast.Module{{
			Name: "IAM", Prefix: "/api/iam",
			Actions: []ast.Action{
				{Name: "Login", Method: "POST", Path: "/login", Input: "LoginInput", Output: "TokenPair"},
				{Name: "GetProfile", Method: "GET", Path: "/me", Output: "User"},
			},
		}},
	}
}

func TestEmitServiceSdk(t *testing.T) {
	tmp := t.TempDir()
	e := New()
	consumed := []emitter.ConsumedServiceInfo{{Name: "iam", AST: testSdkAST(), BaseUrl: "http://iam:3001"}}

	if err := e.EmitServiceSdk(consumed, tmp, emitter.EmitOptions{}); err != nil {
		t.Fatal(err)
	}

	clientPath := filepath.Join(tmp, "Sdk", "Iam", "IamClient.cs")
	data, err := os.ReadFile(clientPath)
	if err != nil {
		t.Fatalf("client file not found: %v", err)
	}
	client := string(data)

	if !strings.Contains(client, "class IamClient") {
		t.Error("missing IamClient class")
	}
	if !strings.Contains(client, "VELD_IAM_URL") {
		t.Error("missing VELD_IAM_URL env var")
	}
	if !strings.Contains(client, "http://iam:3001") {
		t.Error("missing baked-in baseUrl")
	}
	if !strings.Contains(client, "LoginAsync(") {
		t.Error("missing Login method")
	}

	typesPath := filepath.Join(tmp, "Sdk", "Iam", "Types.cs")
	data, err = os.ReadFile(typesPath)
	if err != nil {
		t.Fatalf("types file not found: %v", err)
	}
	types := string(data)
	if !strings.Contains(types, "class User") {
		t.Error("missing User class in types")
	}
	if !strings.Contains(types, "JsonPropertyName") {
		t.Error("missing JsonPropertyName attributes")
	}
}
