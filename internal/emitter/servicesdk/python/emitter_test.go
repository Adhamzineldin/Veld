package python

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
)

func testAST() ast.AST {
	return ast.AST{
		ASTVersion: "1.0.0",
		Models: []ast.Model{
			{
				Name: "User",
				Fields: []ast.Field{
					{Name: "id", Type: "uuid"},
					{Name: "email", Type: "string"},
					{Name: "name", Type: "string", Optional: true},
				},
			},
			{
				Name: "LoginInput",
				Fields: []ast.Field{
					{Name: "email", Type: "string"},
					{Name: "password", Type: "string"},
				},
			},
			{
				Name: "TokenPair",
				Fields: []ast.Field{
					{Name: "accessToken", Type: "string"},
					{Name: "refreshToken", Type: "string"},
				},
			},
		},
		Modules: []ast.Module{
			{
				Name:   "IAM",
				Prefix: "/api/iam",
				Actions: []ast.Action{
					{
						Name:   "Login",
						Method: "POST",
						Path:   "/login",
						Input:  "LoginInput",
						Output: "TokenPair",
					},
					{
						Name:   "GetProfile",
						Method: "GET",
						Path:   "/me",
						Output: "User",
					},
					{
						Name:   "GetUser",
						Method: "GET",
						Path:   "/users/:id",
						Output: "User",
					},
				},
			},
		},
	}
}

func TestEmitServiceSdk(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "veld-sdk-python-test-*")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(tmpDir)

	e := New()
	opts := emitter.EmitOptions{
		ConsumedServices: []emitter.ConsumedServiceInfo{
			{
				Name:    "iam",
				AST:     testAST(),
				BaseUrl: "http://iam-service:3001",
			},
		},
	}

	if err := e.Emit(ast.AST{}, tmpDir, opts); err != nil {
		t.Fatalf("Emit failed: %v", err)
	}

	// Check expected files exist.
	expectedFiles := []string{
		"sdk/iam/client.py",
		"sdk/iam/types.py",
		"sdk/iam/__init__.py",
		"sdk/__init__.py",
	}
	for _, f := range expectedFiles {
		path := filepath.Join(tmpDir, f)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			t.Errorf("expected file %s does not exist", f)
		}
	}

	// Check client.py content.
	clientBytes, err := os.ReadFile(filepath.Join(tmpDir, "sdk/iam/client.py"))
	if err != nil {
		t.Fatal(err)
	}
	client := string(clientBytes)

	if !strings.Contains(client, "class IamClient") {
		t.Error("client.py missing IamClient class")
	}
	if !strings.Contains(client, "VELD_IAM_URL") {
		t.Error("client.py missing VELD_IAM_URL env var")
	}
	if !strings.Contains(client, "http://iam-service:3001") {
		t.Error("client.py missing baked-in baseUrl")
	}
	if !strings.Contains(client, "def login(") {
		t.Error("client.py missing login method")
	}
	if !strings.Contains(client, "def get_profile(") {
		t.Error("client.py missing get_profile method")
	}
	if !strings.Contains(client, "VeldApiError") {
		t.Error("client.py missing VeldApiError")
	}
	// Path param interpolation for GetUser.
	if !strings.Contains(client, "{id}") {
		t.Error("client.py missing path param interpolation")
	}

	// Check types.py content.
	typesBytes, err := os.ReadFile(filepath.Join(tmpDir, "sdk/iam/types.py"))
	if err != nil {
		t.Fatal(err)
	}
	types := string(typesBytes)
	if !strings.Contains(types, "class User(") {
		t.Error("types.py missing User class")
	}
	if !strings.Contains(types, "class LoginInput(") {
		t.Error("types.py missing LoginInput class")
	}
	if !strings.Contains(types, "class TokenPair(") {
		t.Error("types.py missing TokenPair class")
	}
}
