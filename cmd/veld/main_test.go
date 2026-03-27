package main_test

import (
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"testing"
)

// ── Build the binary once for all tests ─────────────────────────────────────

var (
	binaryPath string
	buildOnce  sync.Once
	buildErr   error
)

func veldBinary(t *testing.T) string {
	t.Helper()
	buildOnce.Do(func() {
		tmp := t.TempDir() // first caller owns the dir; that's fine
		ext := ""
		if runtime.GOOS == "windows" {
			ext = ".exe"
		}
		binaryPath = filepath.Join(tmp, "veld"+ext)
		cmd := exec.Command("go", "build", "-o", binaryPath, "./...")
		cmd.Dir = filepath.Join(projectRoot())
		out, err := cmd.CombinedOutput()
		if err != nil {
			buildErr = err
			t.Fatalf("failed to build veld binary: %v\n%s", err, out)
		}
	})
	if buildErr != nil {
		t.Fatalf("veld binary build failed earlier: %v", buildErr)
	}
	return binaryPath
}

func projectRoot() string {
	// This file lives at cmd/veld/main_test.go — go up two levels.
	_, file, _, _ := runtime.Caller(0)
	return filepath.Join(filepath.Dir(file), "..", "..")
}

// ── Run the binary and capture output ───────────────────────────────────────

func runVeld(t *testing.T, dir string, args ...string) (stdout, stderr string, exitCode int) {
	t.Helper()
	bin := veldBinary(t)
	cmd := exec.Command(bin, args...)
	cmd.Dir = dir

	var outBuf, errBuf strings.Builder
	cmd.Stdout = &outBuf
	cmd.Stderr = &errBuf

	err := cmd.Run()
	exitCode = 0
	if err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok {
			exitCode = exitErr.ExitCode()
		} else {
			t.Fatalf("failed to run veld: %v", err)
		}
	}
	return outBuf.String(), errBuf.String(), exitCode
}

// ── Test fixtures ───────────────────────────────────────────────────────────

const testContract = `model User {
  id: uuid
  email: string
  name: string
}

model CreateUserInput {
  email: string
  name: string
}

enum Role { admin user guest }

module Users {
  prefix: /api/v1

  action ListUsers {
    method: GET
    path: /users
    output: User[]
  }

  action CreateUser {
    method: POST
    path: /users
    input: CreateUserInput
    output: User
  }

  action DeleteUser {
    method: DELETE
    path: /users/:id
  }
}
`

const testConfig = `{
  "input": "app.veld",
  "backend": "node-ts",
  "frontend": "typescript",
  "out": "../generated"
}`

func setupTestProject(t *testing.T) string {
	t.Helper()
	dir := t.TempDir()

	veldDir := filepath.Join(dir, "veld")
	if err := os.MkdirAll(veldDir, 0o755); err != nil {
		t.Fatalf("mkdir veld: %v", err)
	}
	if err := os.WriteFile(filepath.Join(veldDir, "app.veld"), []byte(testContract), 0o644); err != nil {
		t.Fatalf("write app.veld: %v", err)
	}
	if err := os.WriteFile(filepath.Join(veldDir, "veld.config.json"), []byte(testConfig), 0o644); err != nil {
		t.Fatalf("write config: %v", err)
	}
	return dir
}

// ── Tests ───────────────────────────────────────────────────────────────────

func TestCLIVersion(t *testing.T) {
	dir := t.TempDir()
	stdout, _, code := runVeld(t, dir, "--version")
	if code != 0 {
		t.Fatalf("expected exit 0, got %d", code)
	}
	if !strings.Contains(stdout, "veld version") {
		t.Errorf("expected version output, got: %s", stdout)
	}
}

func TestCLIHelp(t *testing.T) {
	dir := t.TempDir()
	stdout, _, code := runVeld(t, dir, "--help")
	if code != 0 {
		t.Fatalf("expected exit 0, got %d", code)
	}
	if !strings.Contains(stdout, "Available Commands") {
		t.Errorf("expected 'Available Commands' in help output, got: %s", stdout)
	}
}

func TestCLIInitAlreadyInitialized(t *testing.T) {
	// Create a project that already has a config, then run init — should fail.
	dir := setupTestProject(t)
	_, _, code := runVeld(t, dir, "init")
	if code == 0 {
		t.Fatalf("expected non-zero exit code for already-initialized project, got 0")
	}
}

func TestCLIValidate(t *testing.T) {
	dir := setupTestProject(t)
	_, stderr, code := runVeld(t, dir, "validate")
	if code != 0 {
		t.Fatalf("expected exit 0, got %d; stderr: %s", code, stderr)
	}
}

func TestCLIValidateInvalid(t *testing.T) {
	dir := t.TempDir()
	veldDir := filepath.Join(dir, "veld")
	if err := os.MkdirAll(veldDir, 0o755); err != nil {
		t.Fatal(err)
	}

	invalidContract := `model Broken {
  id: nonexistenttype
}

module Bad {
  prefix: /x

  action Fail {
    method: GET
    path: /fail
    input: NonExistentModel
    output: AlsoNotReal
  }
}
`
	if err := os.WriteFile(filepath.Join(veldDir, "app.veld"), []byte(invalidContract), 0o644); err != nil {
		t.Fatal(err)
	}
	cfg := `{"input": "app.veld", "backend": "node-ts", "frontend": "typescript", "out": "../generated"}`
	if err := os.WriteFile(filepath.Join(veldDir, "veld.config.json"), []byte(cfg), 0o644); err != nil {
		t.Fatal(err)
	}

	_, _, code := runVeld(t, dir, "validate")
	if code == 0 {
		t.Fatalf("expected non-zero exit for invalid contract, got 0")
	}
}

func TestCLIGenerate(t *testing.T) {
	dir := setupTestProject(t)
	_, stderr, code := runVeld(t, dir, "generate", "--force")
	if code != 0 {
		t.Fatalf("expected exit 0, got %d; stderr: %s", code, stderr)
	}

	generatedDir := filepath.Join(dir, "generated")
	info, err := os.Stat(generatedDir)
	if err != nil || !info.IsDir() {
		t.Fatalf("expected generated/ directory to exist")
	}

	// Check some expected files exist.
	expectedFiles := []string{
		"index.ts",
		"package.json",
	}
	for _, f := range expectedFiles {
		path := filepath.Join(generatedDir, f)
		if _, err := os.Stat(path); err != nil {
			t.Errorf("expected generated file %s to exist: %v", f, err)
		}
	}

	// Check subdirectories.
	expectedDirs := []string{"types", "interfaces", "routes", "schemas", "client"}
	for _, d := range expectedDirs {
		path := filepath.Join(generatedDir, d)
		info, err := os.Stat(path)
		if err != nil || !info.IsDir() {
			t.Errorf("expected generated directory %s to exist", d)
		}
	}
}

func TestCLIGenerateDryRun(t *testing.T) {
	dir := setupTestProject(t)
	stdout, stderr, code := runVeld(t, dir, "generate", "--dry-run")
	if code != 0 {
		t.Fatalf("expected exit 0, got %d; stderr: %s", code, stderr)
	}

	// Dry run should produce output describing what would be generated.
	if len(stdout) == 0 && len(stderr) == 0 {
		t.Error("expected some output from dry-run")
	}

	// The generated directory should NOT exist.
	generatedDir := filepath.Join(dir, "generated")
	if _, err := os.Stat(generatedDir); err == nil {
		t.Fatalf("generated/ directory should not exist after --dry-run")
	}
}

func TestCLIGenerateWithFlags(t *testing.T) {
	dir := setupTestProject(t)
	_, stderr, code := runVeld(t, dir, "generate",
		"--backend=python", "--frontend=dart", "--force")
	if code != 0 {
		t.Fatalf("expected exit 0, got %d; stderr: %s", code, stderr)
	}

	generatedDir := filepath.Join(dir, "generated")
	if _, err := os.Stat(generatedDir); err != nil {
		t.Fatalf("expected generated/ to exist after generate with flags")
	}

	// Python backend generates .py files.
	routesDir := filepath.Join(generatedDir, "routes")
	if info, err := os.Stat(routesDir); err != nil || !info.IsDir() {
		t.Fatalf("expected routes/ directory for python backend")
	}

	// Dart frontend generates a client directory.
	clientDir := filepath.Join(generatedDir, "client")
	if info, err := os.Stat(clientDir); err != nil || !info.IsDir() {
		t.Fatalf("expected client/ directory for dart frontend")
	}
}

func TestCLIAST(t *testing.T) {
	dir := setupTestProject(t)
	stdout, stderr, code := runVeld(t, dir, "ast")
	if code != 0 {
		t.Fatalf("expected exit 0, got %d; stderr: %s", code, stderr)
	}

	// Output should be valid JSON.
	var astData map[string]interface{}
	if err := json.Unmarshal([]byte(stdout), &astData); err != nil {
		t.Fatalf("expected valid JSON from ast command: %v\noutput: %s", err, stdout)
	}

	// Should contain models.
	if _, ok := astData["models"]; !ok {
		t.Error("expected 'models' key in AST output")
	}
	// Should contain modules.
	if _, ok := astData["modules"]; !ok {
		t.Error("expected 'modules' key in AST output")
	}
}

func TestCLIOpenAPI(t *testing.T) {
	dir := setupTestProject(t)
	stdout, stderr, code := runVeld(t, dir, "openapi")
	if code != 0 {
		t.Fatalf("expected exit 0, got %d; stderr: %s", code, stderr)
	}

	var spec map[string]interface{}
	if err := json.Unmarshal([]byte(stdout), &spec); err != nil {
		t.Fatalf("expected valid JSON from openapi command: %v\noutput: %s", err, stdout)
	}

	if _, ok := spec["openapi"]; !ok {
		t.Error("expected 'openapi' key in OpenAPI spec")
	}
}

func TestCLIClean(t *testing.T) {
	dir := setupTestProject(t)

	// Generate first.
	_, stderr, code := runVeld(t, dir, "generate", "--force")
	if code != 0 {
		t.Fatalf("generate failed: exit %d; stderr: %s", code, stderr)
	}

	generatedDir := filepath.Join(dir, "generated")
	if _, err := os.Stat(generatedDir); err != nil {
		t.Fatalf("generated/ should exist before clean")
	}

	// Now clean.
	_, stderr, code = runVeld(t, dir, "clean")
	if code != 0 {
		t.Fatalf("clean failed: exit %d; stderr: %s", code, stderr)
	}

	if _, err := os.Stat(generatedDir); !os.IsNotExist(err) {
		t.Fatalf("generated/ should not exist after clean")
	}
}

func TestCLILint(t *testing.T) {
	dir := setupTestProject(t)
	_, stderr, code := runVeld(t, dir, "lint")
	if code != 0 {
		t.Fatalf("expected exit 0, got %d; stderr: %s", code, stderr)
	}
}

func TestCLIGraphQL(t *testing.T) {
	dir := setupTestProject(t)
	stdout, stderr, code := runVeld(t, dir, "graphql")
	if code != 0 {
		t.Fatalf("expected exit 0, got %d; stderr: %s", code, stderr)
	}

	// GraphQL SDL should contain "type" definitions.
	if !strings.Contains(stdout, "type") {
		t.Errorf("expected 'type' keyword in GraphQL output, got: %s", stdout)
	}
}

func TestCLISchema(t *testing.T) {
	dir := setupTestProject(t)
	stdout, stderr, code := runVeld(t, dir, "schema", "--format=prisma")
	if code != 0 {
		t.Fatalf("expected exit 0, got %d; stderr: %s", code, stderr)
	}

	// Prisma schema should contain "model".
	if !strings.Contains(stdout, "model") {
		t.Errorf("expected 'model' keyword in Prisma schema output, got: %s", stdout)
	}
}

func TestCLIDocs(t *testing.T) {
	dir := setupTestProject(t)
	outFile := filepath.Join(dir, "docs.html")
	_, stderr, code := runVeld(t, dir, "docs", "-o", outFile)
	if code != 0 {
		t.Fatalf("expected exit 0, got %d; stderr: %s", code, stderr)
	}

	if _, err := os.Stat(outFile); err != nil {
		t.Fatalf("expected docs output file to exist: %v", err)
	}

	content, err := os.ReadFile(outFile)
	if err != nil {
		t.Fatalf("failed to read docs file: %v", err)
	}
	if !strings.Contains(string(content), "Users") {
		t.Errorf("expected docs to mention 'Users' module")
	}
}

func TestCLIDiff(t *testing.T) {
	dir := setupTestProject(t)

	// Generate first so there's something to diff against.
	_, stderr, code := runVeld(t, dir, "generate", "--force")
	if code != 0 {
		t.Fatalf("generate failed: exit %d; stderr: %s", code, stderr)
	}

	// Diff should succeed (no changes).
	_, stderr, code = runVeld(t, dir, "diff")
	if code != 0 {
		t.Fatalf("diff failed: exit %d; stderr: %s", code, stderr)
	}
}

func TestCLIFmt(t *testing.T) {
	dir := setupTestProject(t)
	veldFile := filepath.Join(dir, "veld", "app.veld")

	// Run fmt without --write, should output formatted content.
	stdout, stderr, code := runVeld(t, dir, "fmt", veldFile)
	if code != 0 {
		t.Fatalf("expected exit 0, got %d; stderr: %s", code, stderr)
	}

	// Should contain the model definition.
	if !strings.Contains(stdout, "model User") {
		t.Errorf("expected formatted output to contain 'model User', got: %s", stdout)
	}
}

func TestCLIDoctor(t *testing.T) {
	dir := setupTestProject(t)
	_, _, code := runVeld(t, dir, "doctor")
	// Doctor may exit 0 or non-zero depending on project health checks.
	// Just verify it doesn't crash (exit code should be 0 or 1, not a panic).
	if code > 1 {
		t.Fatalf("doctor exited with unexpected code %d", code)
	}
}

func TestCLIUnknownCommand(t *testing.T) {
	dir := t.TempDir()
	_, _, code := runVeld(t, dir, "nonexistent-command")
	if code == 0 {
		t.Fatalf("expected non-zero exit for unknown command")
	}
}

func TestCLIGenerateMultipleBackends(t *testing.T) {
	// Test that generating with different backends produces different output.
	backends := []struct {
		name string
		ext  string
	}{
		{"node-ts", ".ts"},
		{"python", ".py"},
	}

	for _, b := range backends {
		t.Run(b.name, func(t *testing.T) {
			dir := setupTestProject(t)
			_, stderr, code := runVeld(t, dir, "generate",
				"--backend="+b.name, "--frontend=none", "--force")
			if code != 0 {
				t.Fatalf("generate --backend=%s failed: exit %d; stderr: %s", b.name, code, stderr)
			}

			generatedDir := filepath.Join(dir, "generated")
			if _, err := os.Stat(generatedDir); err != nil {
				t.Fatalf("expected generated/ to exist for backend %s", b.name)
			}

			// Verify that files with the expected extension exist somewhere in generated/.
			found := false
			filepath.Walk(generatedDir, func(path string, info os.FileInfo, err error) error {
				if err != nil {
					return err
				}
				if strings.HasSuffix(path, b.ext) {
					found = true
				}
				return nil
			})
			if !found {
				t.Errorf("expected %s files in generated/ for backend %s", b.ext, b.name)
			}
		})
	}
}

func TestCLIOpenAPIToFile(t *testing.T) {
	dir := setupTestProject(t)
	outFile := filepath.Join(dir, "openapi.json")
	_, stderr, code := runVeld(t, dir, "openapi", "-o", outFile)
	if code != 0 {
		t.Fatalf("expected exit 0, got %d; stderr: %s", code, stderr)
	}

	content, err := os.ReadFile(outFile)
	if err != nil {
		t.Fatalf("failed to read openapi output file: %v", err)
	}

	var spec map[string]interface{}
	if err := json.Unmarshal(content, &spec); err != nil {
		t.Fatalf("expected valid JSON in openapi file: %v", err)
	}
	if _, ok := spec["openapi"]; !ok {
		t.Error("expected 'openapi' key in spec file")
	}
}
