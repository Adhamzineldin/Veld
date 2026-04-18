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

	// ObjectMapper must register JavaTimeModule and disable date-as-timestamps
	// so java.time.LocalDate / LocalDateTime fields round-trip correctly.
	// Without this, Jackson serialises them as numeric arrays / throws on read.
	if !strings.Contains(client, "import com.fasterxml.jackson.datatype.jsr310.JavaTimeModule;") {
		t.Error("client missing JavaTimeModule import")
	}
	if !strings.Contains(client, "import com.fasterxml.jackson.databind.SerializationFeature;") {
		t.Error("client missing SerializationFeature import")
	}
	if !strings.Contains(client, "registerModule(new JavaTimeModule())") {
		t.Error("ObjectMapper not configured with JavaTimeModule")
	}
	if !strings.Contains(client, "disable(SerializationFeature.WRITE_DATES_AS_TIMESTAMPS)") {
		t.Error("ObjectMapper not configured to write ISO-8601 dates")
	}

	// Models are now organised per-module: models/{moduleLower}/{Model}.java
	// The IAM module owns User and LoginInput, so they land in models/iam/
	typesPath := filepath.Join(tmp, "src", "main", "java", "maayn", "veld", "generated", "sdk", "iam", "models", "iam", "User.java")
	if _, err := os.Stat(typesPath); os.IsNotExist(err) {
		t.Error("User.java not generated in models/iam/")
	}
}

// TestEmitServiceSdkClientImportsMatchModels verifies that every model sub-package
// imported by the client actually exists on disk (models are not in "shared" when
// the client expects them in the module folder).
func TestEmitServiceSdkClientImportsMatchModels(t *testing.T) {
	// Simulate consumed "account" service with Account module — models used by
	// actions must NOT end up in shared/.
	accountAST := ast.AST{
		Models: []ast.Model{
			{Name: "Account", Fields: []ast.Field{
				{Name: "id", Type: "uuid"},
				{Name: "name", Type: "string"},
				{Name: "balance", Type: "float"},
			}},
			{Name: "CreateAccountInput", Fields: []ast.Field{
				{Name: "name", Type: "string"},
			}},
			{Name: "AccountResponse", Fields: []ast.Field{
				{Name: "account", Type: "Account"},
			}},
		},
		Modules: []ast.Module{{
			Name: "Account", Prefix: "/api/account",
			Actions: []ast.Action{
				{Name: "CreateAccount", Method: "POST", Path: "/", Input: "CreateAccountInput", Output: "AccountResponse"},
				{Name: "GetAccount", Method: "GET", Path: "/:id", Output: "Account"},
			},
		}},
	}

	tmp := t.TempDir()
	e := New()
	consumed := []emitter.ConsumedServiceInfo{{Name: "account", AST: accountAST, BaseUrl: "http://account:3002"}}

	if err := e.EmitServiceSdk(consumed, tmp, emitter.EmitOptions{}); err != nil {
		t.Fatal(err)
	}

	// Read the client
	clientPath := filepath.Join(tmp, "src", "main", "java", "maayn", "veld", "generated", "sdk", "account", "AccountClient.java")
	data, err := os.ReadFile(clientPath)
	if err != nil {
		t.Fatalf("AccountClient.java not found: %v", err)
	}
	client := string(data)

	// Verify the models exist at the package the client imports.
	// The client should import the "account" module sub-package, not "shared".
	if strings.Contains(client, "models.shared") {
		t.Error("client imports models.shared — models should be in models/account/ not models/shared/")
	}

	// Every model/*.java imported must exist on disk
	sdkBase := filepath.Join(tmp, "src", "main", "java", "maayn", "veld", "generated", "sdk", "account")
	for _, modelName := range []string{"Account", "CreateAccountInput", "AccountResponse"} {
		// Should be in models/account/ (module-name based)
		modelsDir := filepath.Join(sdkBase, "models", "account")
		modelFile := filepath.Join(modelsDir, modelName+".java")
		if _, err := os.Stat(modelFile); os.IsNotExist(err) {
			// Check if it ended up in shared instead
			sharedFile := filepath.Join(sdkBase, "models", "shared", modelName+".java")
			if _, err2 := os.Stat(sharedFile); err2 == nil {
				t.Errorf("%s.java ended up in models/shared/ instead of models/account/", modelName)
			} else {
				t.Errorf("%s.java not found anywhere in SDK output", modelName)
			}
		}
	}

	// Verify no shared folder at all — all models belong to the Account module
	sharedDir := filepath.Join(sdkBase, "models", "shared")
	if _, err := os.Stat(sharedDir); err == nil {
		entries, _ := os.ReadDir(sharedDir)
		if len(entries) > 0 {
			var names []string
			for _, e := range entries {
				names = append(names, e.Name())
			}
			t.Errorf("unexpected files in models/shared/: %v", names)
		}
	}
}

// TestEmitServiceSdkServiceNameDiffersFromModule verifies the scenario where
// the workspace service name differs from the module name (e.g. service "account"
// but module "Accounts"). The client imports must match the actual model locations.
// Regression: enums used by model fields were landing in "shared" but the client
// never imported the shared package → java: package ... does not exist.
func TestEmitServiceSdkServiceNameDiffersFromModule(t *testing.T) {
	// Service name = "account" (singular), module name = "Accounts" (plural)
	// AccountType is used by Account.type field → enum lands in models/accounts/
	// BUT if a second enum (Currency) exists and is NOT used by any model field,
	// it goes to shared → client must also import models.shared.*
	astData := ast.AST{
		Models: []ast.Model{
			{Name: "Account", Fields: []ast.Field{
				{Name: "id", Type: "uuid"},
				{Name: "name", Type: "string"},
				{Name: "type", Type: "AccountType"},
			}},
			{Name: "CreateAccountInput", Fields: []ast.Field{
				{Name: "name", Type: "string"},
			}},
		},
		Enums: []ast.Enum{
			{Name: "AccountType", Values: []string{"checking", "savings"}},
			{Name: "Currency", Values: []string{"USD", "EUR", "GBP"}},
		},
		Modules: []ast.Module{{
			Name: "Accounts", Prefix: "/api/accounts",
			Actions: []ast.Action{
				{Name: "CreateAccount", Method: "POST", Path: "/", Input: "CreateAccountInput", Output: "Account"},
				{Name: "GetAccount", Method: "GET", Path: "/:id", Output: "Account"},
			},
		}},
	}

	tmp := t.TempDir()
	e := New()
	consumed := []emitter.ConsumedServiceInfo{{Name: "account", AST: astData, BaseUrl: "http://account:3002"}}

	if err := e.EmitServiceSdk(consumed, tmp, emitter.EmitOptions{}); err != nil {
		t.Fatal(err)
	}

	sdkBase := filepath.Join(tmp, "src", "main", "java", "maayn", "veld", "generated", "sdk", "account")
	clientPath := filepath.Join(sdkBase, "AccountClient.java")
	data, err := os.ReadFile(clientPath)
	if err != nil {
		t.Fatalf("AccountClient.java not found: %v", err)
	}
	client := string(data)

	// List all generated files for debugging
	filepath.Walk(sdkBase, func(path string, info os.FileInfo, err error) error {
		if err == nil && !info.IsDir() {
			rel, _ := filepath.Rel(sdkBase, path)
			t.Logf("  generated: %s", rel)
		}
		return nil
	})

	// The module name is "Accounts" → models land in models/accounts/
	// The client must import models.accounts.*, NOT models.account.*
	// Every import maayn.veld.generated.sdk.account.models.X.* must have a matching directory
	for _, line := range strings.Split(client, "\n") {
		line = strings.TrimSpace(line)
		if !strings.HasPrefix(line, "import maayn.veld.generated.sdk.account.models.") {
			continue
		}
		// Extract the sub-package: "import maayn.veld.generated.sdk.account.models.FOO.*;" → "FOO"
		rest := strings.TrimPrefix(line, "import maayn.veld.generated.sdk.account.models.")
		rest = strings.TrimSuffix(rest, ".*;")
		rest = strings.TrimSuffix(rest, ";")
		subDir := filepath.Join(sdkBase, "models", rest)
		if _, err := os.Stat(subDir); os.IsNotExist(err) {
			t.Errorf("client imports models.%s.* but directory %s does not exist", rest, subDir)
			modelsDir := filepath.Join(sdkBase, "models")
			entries, _ := os.ReadDir(modelsDir)
			for _, e := range entries {
				t.Logf("  actual models subdir: %s", e.Name())
			}
		}
	}

	// Verify that AccountType.java is in models/accounts/ (used by Account.type)
	if _, err := os.Stat(filepath.Join(sdkBase, "models", "accounts", "AccountType.java")); os.IsNotExist(err) {
		t.Error("AccountType.java not in models/accounts/ — should be there since Account uses it")
	}

	// Verify the unreferenced Currency enum is in shared (fine) AND the client imports it
	if _, err := os.Stat(filepath.Join(sdkBase, "models", "shared", "Currency.java")); os.IsNotExist(err) {
		t.Error("Currency.java not generated at all")
	}
	if !strings.Contains(client, "models.shared.*") {
		t.Error("client does NOT import models.shared.* — unreferenced enums would be invisible")
	}
}
