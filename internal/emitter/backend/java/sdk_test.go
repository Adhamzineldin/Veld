package java

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

	clientPath := filepath.Join(tmp, "src", "main", "java", "maayn", "veld", "generated", "sdk", "iam", "IamClient.java")
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
	if !strings.Contains(client, "login(") {
		t.Error("missing login method")
	}
	if !strings.Contains(client, "getProfile(") {
		t.Error("missing getProfile method")
	}

	// Models are now organised per-module: models/{moduleLower}/{Model}.java
	// The IAM module owns User and LoginInput, so they land in models/iam/
	typesPath := filepath.Join(tmp, "src", "main", "java", "maayn", "veld", "generated", "sdk", "iam", "models", "iam", "User.java")
	if _, err := os.Stat(typesPath); os.IsNotExist(err) {
		t.Error("User.java not generated in models/iam/")
	}
}
