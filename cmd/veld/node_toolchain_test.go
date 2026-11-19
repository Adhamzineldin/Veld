package main_test

// TypeScript compile and Express round-trip tests for the node-ts backend,
// the TypeScript client and the React hooks (B-015, M1 "Express server +
// typed client round trip"). They need Node.js and the packages pinned in
// testdata/node:
//
//	npm ci --prefix cmd/veld/testdata/node
//
// Without them the tests skip and name what is missing (AGENTS.md §4). With
// VELD_REQUIRE_TOOLCHAINS=1, as in CI, a missing toolchain fails instead.

import (
	"context"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

const requireToolchainsEnv = "VELD_REQUIRE_TOOLCHAINS"

// nodeToolchainConfig generates backend and client into ./generated, next to
// roundtrip.ts, which imports from there.
const nodeToolchainConfig = `{
  "input": "app.veld",
  "backend": "node-ts",
  "frontend": "typescript",
  "out": "../generated"
}`

func missingToolchain(t *testing.T, reason string) {
	t.Helper()
	if os.Getenv(requireToolchainsEnv) != "" {
		t.Fatalf("%s (%s is set)", reason, requireToolchainsEnv)
	}
	t.Skip(reason)
}

// nodeHarness returns the harness directory and the node binary, or skips.
func nodeHarness(t *testing.T) (harness, nodeBin string) {
	t.Helper()
	harness = filepath.Join(projectRoot(), "cmd", "veld", "testdata", "node")
	nodeBin, err := exec.LookPath("node")
	if err != nil {
		missingToolchain(t, "Node.js (node) is not on PATH")
	}
	if _, err := os.Stat(filepath.Join(harness, "node_modules", "typescript", "bin", "tsc")); err != nil {
		missingToolchain(t, "TypeScript toolchain is not installed; run: npm ci --prefix cmd/veld/testdata/node")
	}
	return harness, nodeBin
}

// writeStrictTSConfig writes a strict tsconfig for dir that resolves packages
// and types from the harness's node_modules.
func writeStrictTSConfig(t *testing.T, dir, harness string, include []string) {
	t.Helper()
	modules := filepath.ToSlash(filepath.Join(harness, "node_modules"))
	cfg := map[string]any{
		"compilerOptions": map[string]any{
			"target":           "ES2022",
			"module":           "commonjs",
			"moduleResolution": "node",
			"strict":           true,
			"esModuleInterop":  true,
			"skipLibCheck":     true,
			"outDir":           "dist",
			"rootDir":          ".",
			"baseUrl":          ".",
			"typeRoots":        []string{modules + "/@types"},
			"types":            []string{"node"},
			"paths":            map[string][]string{"*": {modules + "/*", modules + "/@types/*"}},
		},
		"include": include,
	}
	data, err := json.MarshalIndent(cfg, "", "  ")
	if err != nil {
		t.Fatalf("encode tsconfig: %v", err)
	}
	writeFile(t, filepath.Join(dir, "tsconfig.json"), string(data))
}

// runTool runs a toolchain command in dir and fails the test with its output.
func runTool(t *testing.T, dir string, env []string, name string, args ...string) string {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Minute)
	defer cancel()
	cmd := exec.CommandContext(ctx, name, args...)
	cmd.Dir = dir
	cmd.Env = append(os.Environ(), env...)
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("%s %s: %v\n%s", filepath.Base(name), strings.Join(args, " "), err, out)
	}
	return string(out)
}

func TestNodeToolchain(t *testing.T) {
	harness, nodeBin := nodeHarness(t)
	tsc := filepath.Join(harness, "node_modules", "typescript", "bin", "tsc")

	variants := []struct {
		name      string
		flags     []string
		roundTrip bool // also run roundtrip.ts against the compiled output
	}{
		{name: "express", flags: []string{"--backend-framework=express"}, roundTrip: true},
		{name: "express with validators", flags: []string{"--backend-framework=express", "--validate"}, roundTrip: true},
		{name: "plain router", roundTrip: true},
		{name: "react hooks", flags: []string{"--backend-framework=express", "--frontend=react"}},
	}
	for _, v := range variants {
		t.Run(v.name, func(t *testing.T) {
			t.Parallel()
			dir := setupTestProject(t)
			writeFile(t, filepath.Join(dir, "veld", "app.veld"), readFile(t, filepath.Join(harness, "app.veld")))
			writeFile(t, filepath.Join(dir, "veld", "veld.config.json"), nodeToolchainConfig)
			args := append([]string{"generate", "--force"}, v.flags...)
			if _, stderr, code := runVeld(t, dir, args...); code != 0 {
				t.Fatalf("veld %s: exit %d; stderr: %s", strings.Join(args, " "), code, stderr)
			}

			include := []string{"generated/**/*.ts"}
			if v.roundTrip {
				writeFile(t, filepath.Join(dir, "roundtrip.ts"), readFile(t, filepath.Join(harness, "roundtrip.ts")))
				include = append(include, "roundtrip.ts")
			}
			writeStrictTSConfig(t, dir, harness, include)

			// F-NODE-06: the generated code compiles under --strict.
			runTool(t, dir, nil, nodeBin, tsc, "-p", dir)

			if v.roundTrip {
				out := runTool(t, dir, []string{"NODE_PATH=" + filepath.Join(harness, "node_modules")},
					nodeBin, filepath.Join(dir, "dist", "roundtrip.js"))
				if !strings.Contains(out, "checks passed") {
					t.Fatalf("round trip did not report success:\n%s", out)
				}
			}
		})
	}
}
