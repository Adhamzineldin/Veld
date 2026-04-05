package node

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
)

// testAST builds a minimal IAM-style AST for testing.
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
						Name:        "Login",
						Method:      "POST",
						Path:        "/login",
						Input:       "LoginInput",
						Output:      "TokenPair",
						Description: "Authenticate and receive JWT tokens",
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
	tmpDir, err := os.MkdirTemp("", "veld-sdk-node-test-*")
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
		"sdk/iam/client.ts",
		"sdk/iam/types.ts",
		"sdk/iam/index.ts",
		"sdk/index.ts",
	}
	for _, f := range expectedFiles {
		path := filepath.Join(tmpDir, f)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			t.Errorf("expected file %s does not exist", f)
		}
	}

	// Check client.ts content.
	clientBytes, err := os.ReadFile(filepath.Join(tmpDir, "sdk/iam/client.ts"))
	if err != nil {
		t.Fatal(err)
	}
	client := string(clientBytes)

	// Must contain class name.
	if !strings.Contains(client, "class IamClient") {
		t.Error("client.ts missing IamClient class")
	}
	// Must contain env var fallback.
	if !strings.Contains(client, "VELD_IAM_URL") {
		t.Error("client.ts missing VELD_IAM_URL env var")
	}
	// Must contain baked-in base URL.
	if !strings.Contains(client, "http://iam-service:3001") {
		t.Error("client.ts missing baked-in baseUrl")
	}
	// Must contain login method.
	if !strings.Contains(client, "login(input: LoginInput)") {
		t.Error("client.ts missing login method")
	}
	// Must contain path param interpolation for GetUser.
	if !strings.Contains(client, "${id}") {
		t.Error("client.ts missing path param interpolation")
	}
	// Must contain getProfile method.
	if !strings.Contains(client, "getProfile()") {
		t.Error("client.ts missing getProfile method")
	}
	// Must contain VeldApiError.
	if !strings.Contains(client, "VeldApiError") {
		t.Error("client.ts missing VeldApiError")
	}

	// Check types.ts content.
	typesBytes, err := os.ReadFile(filepath.Join(tmpDir, "sdk/iam/types.ts"))
	if err != nil {
		t.Fatal(err)
	}
	types := string(typesBytes)
	if !strings.Contains(types, "export interface User") {
		t.Error("types.ts missing User interface")
	}
	if !strings.Contains(types, "export interface LoginInput") {
		t.Error("types.ts missing LoginInput interface")
	}
	if !strings.Contains(types, "export interface TokenPair") {
		t.Error("types.ts missing TokenPair interface")
	}
}

func TestEmitServiceSdkNoBaseUrl(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "veld-sdk-node-nobase-*")
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
				BaseUrl: "", // no default baseUrl
			},
		},
	}

	if err := e.Emit(ast.AST{}, tmpDir, opts); err != nil {
		t.Fatalf("Emit failed: %v", err)
	}

	clientBytes, err := os.ReadFile(filepath.Join(tmpDir, "sdk/iam/client.ts"))
	if err != nil {
		t.Fatal(err)
	}
	client := string(clientBytes)

	// When no baseUrl, should throw error if not provided.
	if !strings.Contains(client, "No base URL for iam") {
		t.Error("client.ts should throw when no baseUrl is provided")
	}
}
