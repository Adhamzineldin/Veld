package golang

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
	tmpDir, err := os.MkdirTemp("", "veld-sdk-go-test-*")
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
		"sdk/iam/client.go",
		"sdk/iam/types.go",
		"sdk/iam/doc.go",
	}
	for _, f := range expectedFiles {
		path := filepath.Join(tmpDir, f)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			t.Errorf("expected file %s does not exist", f)
		}
	}

	// Check client.go content.
	clientBytes, err := os.ReadFile(filepath.Join(tmpDir, "sdk/iam/client.go"))
	if err != nil {
		t.Fatal(err)
	}
	client := string(clientBytes)

	if !strings.Contains(client, "package iam") {
		t.Error("client.go wrong package name")
	}
	if !strings.Contains(client, "type Client struct") {
		t.Error("client.go missing Client struct")
	}
	if !strings.Contains(client, "func NewClient(") {
		t.Error("client.go missing NewClient constructor")
	}
	if !strings.Contains(client, "VELD_IAM_URL") {
		t.Error("client.go missing VELD_IAM_URL env var")
	}
	if !strings.Contains(client, "http://iam-service:3001") {
		t.Error("client.go missing baked-in baseUrl")
	}
	if !strings.Contains(client, "func (c *Client) Login(") {
		t.Error("client.go missing Login method")
	}
	if !strings.Contains(client, "func (c *Client) GetProfile(") {
		t.Error("client.go missing GetProfile method")
	}
	if !strings.Contains(client, "func (c *Client) GetUser(") {
		t.Error("client.go missing GetUser method")
	}
	if !strings.Contains(client, "VeldApiError") {
		t.Error("client.go missing VeldApiError")
	}
	// Functional options.
	if !strings.Contains(client, "WithHTTPClient") {
		t.Error("client.go missing WithHTTPClient option")
	}
	if !strings.Contains(client, "WithHeaders") {
		t.Error("client.go missing WithHeaders option")
	}

	// Check types.go content.
	typesBytes, err := os.ReadFile(filepath.Join(tmpDir, "sdk/iam/types.go"))
	if err != nil {
		t.Fatal(err)
	}
	types := string(typesBytes)
	if !strings.Contains(types, "type User struct") {
		t.Error("types.go missing User struct")
	}
	if !strings.Contains(types, "type LoginInput struct") {
		t.Error("types.go missing LoginInput struct")
	}
	if !strings.Contains(types, "type TokenPair struct") {
		t.Error("types.go missing TokenPair struct")
	}
	// Check JSON tags.
	if !strings.Contains(types, "`json:\"email\"`") {
		t.Error("types.go missing json tags")
	}
	// Check optional fields use pointer types.
	if !strings.Contains(types, "*string") {
		t.Error("types.go missing optional pointer type for name field")
	}
}
