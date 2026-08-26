package main

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/setup"
	"github.com/spf13/cobra"
	// Register all emitters via init(). To add a new emitter, add one line here.
	// Register tool emitters (auxiliary generators — NOT backends).
)

func newInitCmd() *cobra.Command {
	return &cobra.Command{
		Use:     "init",
		Short:   "Scaffold a new Veld project in the current directory",
		Example: "  mkdir my-api && cd my-api && veld init",
		RunE:    func(cmd *cobra.Command, args []string) error { return runInit() },
	}
}

func runInit() error {
	for _, p := range []string{"veld/veld.config.json", "veld.config.json"} {
		if _, err := os.Stat(p); err == nil {
			fmt.Fprintln(os.Stderr, red("Error:")+" veld project already initialized in this directory")
			os.Exit(1)
		}
	}

	reader := bufio.NewReader(os.Stdin)

	fmt.Println()
	fmt.Println(bold("  Veld") + " — project setup")
	fmt.Println()

	// ── Auto-detect project type ───────────────────────────────────────────
	detectedBackend := ""
	if _, err := os.Stat("package.json"); err == nil {
		detectedBackend = "node-ts"
	}
	if _, err := os.Stat("go.mod"); err == nil {
		detectedBackend = "go"
	}
	if _, err := os.Stat("requirements.txt"); err == nil {
		detectedBackend = "python"
	}
	if _, err := os.Stat("pyproject.toml"); err == nil {
		detectedBackend = "python"
	}
	if _, err := os.Stat("Cargo.toml"); err == nil {
		detectedBackend = "rust"
	}
	if _, err := os.Stat("pom.xml"); err == nil {
		detectedBackend = "java"
	}
	if _, err := os.Stat("build.gradle"); err == nil {
		detectedBackend = "java"
	}
	if _, err := os.Stat("composer.json"); err == nil {
		detectedBackend = "php"
	}
	csprojFiles, _ := filepath.Glob("*.csproj")
	if len(csprojFiles) > 0 {
		detectedBackend = "csharp"
	}
	if detectedBackend != "" {
		fmt.Printf("  %s Detected project type: %s\n\n", dim("ℹ"), bold(detectedBackend))
	}

	// ── Project type selection ─────────────────────────────────────────────
	fmt.Println("  " + bold("◆ Project type"))
	fmt.Printf("    %s1%s  Single service %s\n", colorGreen, colorReset, dim("(monolith — one backend, one frontend)"))
	fmt.Printf("    %s2%s  Microservices workspace %s\n", colorGreen, colorReset, dim("(multiple services with inter-service SDK)"))
	fmt.Print("\n  Choose [1]: ")
	projectTypeIdx := readChoice(reader, 2, 1)
	isMicroservices := projectTypeIdx == 2
	if isMicroservices {
		fmt.Printf("  → %s\n\n", green("microservices workspace"))
	} else {
		fmt.Printf("  → %s\n\n", green("single service"))
	}

	// ── Framework option tables ────────────────────────────────────────────
	type initFWOpt struct{ name, desc string }
	nodeFrameworkOpts := []initFWOpt{
		{"plain", "router: any — wire your own Express / Fastify / Hono / NestJS"},
		{"express", "Express 4.x"},
	}
	backendFrameworkOpts := map[string][]initFWOpt{
		"node-ts": nodeFrameworkOpts,
		"node-js": nodeFrameworkOpts,
		"python": {
			{"plain", "pure typed functions — no HTTP framework"},
			{"flask", "Flask blueprints + jsonify"},
			{"fastapi", "FastAPI router + Pydantic"},
		},
		"go": {
			{"plain", "net/http (Go 1.22+) — zero external dependencies"},
			{"chi", "Chi v5 router"},
			{"gin", "Gin framework"},
		},
		"java": {
			{"plain", "interfaces only — no HTTP framework"},
			{"spring", "Spring Boot 3.x / 4.x controllers + service interfaces"},
		},
		"rust": {
			{"plain", "trait definitions only — no HTTP framework"},
			{"axum", "Axum async handlers + Tokio"},
		},
		"csharp": {
			{"plain", "interfaces only — no HTTP framework"},
			{"aspnet", "ASP.NET Core controllers"},
		},
		"php": {
			{"plain", "interfaces only — no HTTP framework"},
			{"laravel", "Laravel routes + controllers"},
		},
	}
	frontendFrameworkOpts := map[string][]initFWOpt{
		"typescript": {
			{"", "pure fetch SDK — no framework wrapper"},
			{"react", "React Query hooks"},
			{"vue", "Vue 3 composables"},
			{"angular", "Angular services"},
			{"svelte", "Svelte stores"},
		},
		"javascript": {
			{"", "pure fetch SDK — no framework wrapper"},
			{"react", "React hooks (plain JavaScript)"},
		},
	}
	backendDisplayLabel := map[string]string{
		"node-ts": "node         Node.js",
		"python":  "python       Python 3",
		"go":      "go           Go",
		"rust":    "rust         Rust",
		"java":    "java         Java 17+",
		"csharp":  "csharp       C# / .NET 8",
		"php":     "php          PHP 8",
	}

	// ── Backend language selection ─────────────────────────────────────────
	// node-ts and node-js are merged under "node"; language is a sub-prompt.
	allBackends := emitter.ListBackends()
	var backends []string
	for _, b := range allBackends {
		if b != "node-js" {
			backends = append(backends, b)
		}
	}
	fmt.Println("  " + bold("Backend language") + " — which server language?")
	defaultBackendIdx := 1
	for i, b := range backends {
		if b == "node-ts" {
			defaultBackendIdx = i + 1
		}
		if detectedBackend != "" && b == detectedBackend {
			defaultBackendIdx = i + 1
		}
	}
	for i, b := range backends {
		label := backendDisplayLabel[b]
		if label == "" {
			label = b
		}
		if detectedBackend != "" && b == detectedBackend {
			label += dim(" ← detected")
		}
		if i+1 == defaultBackendIdx {
			label += dim(" (default)")
		}
		fmt.Printf("    %s%2d%s  %s\n", colorGreen, i+1, colorReset, label)
	}
	fmt.Printf("\n  Choose [%d]: ", defaultBackendIdx)
	backendChoice := readChoice(reader, len(backends), defaultBackendIdx)
	selectedBackend := backends[backendChoice-1]
	fmt.Printf("  → %s\n\n", green(selectedBackend))

	// ── Node language sub-prompt (TypeScript / JavaScript) ────────────────
	if selectedBackend == "node-ts" {
		fmt.Println("  " + bold("Node language") + " — TypeScript or JavaScript?")
		fmt.Printf("    %s 1%s  TypeScript  %s\n", colorGreen, colorReset, dim("(default)"))
		fmt.Printf("    %s 2%s  JavaScript  %s\n", colorGreen, colorReset, dim("plain JS, JSDoc types"))
		fmt.Print("\n  Choose [1]: ")
		langIdx := readChoice(reader, 2, 1)
		if langIdx == 2 {
			selectedBackend = "node-js"
			fmt.Printf("  → %s\n\n", green("JavaScript"))
		} else {
			fmt.Printf("  → %s\n\n", green("TypeScript"))
		}
	}

	// ── Backend framework selection ────────────────────────────────────────
	selectedBackendFramework := ""
	if fwOpts := backendFrameworkOpts[selectedBackend]; len(fwOpts) > 0 {
		fmt.Println("  " + bold("Backend framework") + " — which HTTP framework? (plain = no framework)")
		for i, fw := range fwOpts {
			lbl := fw.name
			if lbl == "plain" || lbl == "" {
				lbl = "plain"
			}
			fmt.Printf("    %s%2d%s  %-12s %s\n", colorGreen, i+1, colorReset, lbl, dim(fw.desc))
		}
		fmt.Print("\n  Choose [1]: ")
		fwIdx := readChoice(reader, len(fwOpts), 1)
		selectedBackendFramework = fwOpts[fwIdx-1].name
		if selectedBackendFramework == "plain" {
			selectedBackendFramework = ""
		}
		display := fwOpts[fwIdx-1].name
		if display == "" || display == "plain" {
			display = "plain (no framework)"
		}
		fmt.Printf("  → %s\n\n", green(display))
	}

	// ── Frontend language selection ────────────────────────────────────────
	// Show language choices only; framework-specific emitters are sub-options below.
	fwSpecificFrontends := map[string]bool{"react": true, "vue": true, "angular": true, "svelte": true}
	var frontends []string
	for _, f := range append(emitter.ListFrontends(), "none") {
		if !fwSpecificFrontends[f] {
			frontends = append(frontends, f)
		}
	}
	fmt.Println("  " + bold("Frontend language") + " — which client language / SDK?")
	defaultFrontend := 1
	for i, f := range frontends {
		if f == "typescript" {
			defaultFrontend = i + 1
			break
		}
	}
	for i, f := range frontends {
		label := f
		if i+1 == defaultFrontend {
			label += dim(" (default)")
		}
		fmt.Printf("    %s%2d%s  %s\n", colorGreen, i+1, colorReset, label)
	}
	fmt.Printf("\n  Choose [%d]: ", defaultFrontend)
	frontendChoice := readChoice(reader, len(frontends), defaultFrontend)
	selectedFrontend := frontends[frontendChoice-1]
	fmt.Printf("  → %s\n\n", green(selectedFrontend))

	// ── Frontend framework selection ───────────────────────────────────────
	selectedFrontendFramework := ""
	if fwOpts := frontendFrameworkOpts[selectedFrontend]; len(fwOpts) > 0 {
		fmt.Println("  " + bold("Frontend framework") + " — wrap the SDK with a UI framework?")
		for i, fw := range fwOpts {
			lbl := fw.name
			if lbl == "" {
				lbl = "none"
			}
			fmt.Printf("    %s%2d%s  %-12s %s\n", colorGreen, i+1, colorReset, lbl, dim(fw.desc))
		}
		fmt.Print("\n  Choose [1]: ")
		fwIdx := readChoice(reader, len(fwOpts), 1)
		selectedFrontendFramework = fwOpts[fwIdx-1].name
		display := fwOpts[fwIdx-1].name
		if display == "" {
			display = "none (pure SDK)"
		}
		fmt.Printf("  → %s\n\n", green(display))
	}

	// ── Runtime validation ─────────────────────────────────────────────────
	// Only relevant for node and python — statically-typed backends (go, rust,
	// java, csharp) already enforce contract correctness at compile time.
	enableValidate := false
	if selectedBackend == "node-ts" || selectedBackend == "node-js" || selectedBackend == "python" {
		fmt.Println("  " + bold("Runtime validation") + " — validate input/output shapes at runtime?")
		fmt.Printf("    %s1%s  disabled %s\n", colorGreen, colorReset, dim("(default — zero overhead, TypeScript/Python types enforce the contract)"))
		fmt.Printf("    %s2%s  enabled  %s\n", colorGreen, colorReset, dim("(adds zero-dep validators to routes: 400 on bad input, 500 on contract violation)"))
		fmt.Print("\n  Choose [1]: ")
		if readChoice(reader, 2, 1) == 2 {
			enableValidate = true
		}
		if enableValidate {
			fmt.Printf("  → %s\n\n", green("enabled"))
		} else {
			fmt.Printf("  → %s\n\n", dim("disabled"))
		}
	}

	// ── Generate config with selections ────────────────────────────────────
	// For Python, default to "veld_gen" as the output directory name
	// so the folder itself is a valid Python package importable from cwd.
	defaultOut := "../generated"
	if selectedBackend == "python" {
		defaultOut = "../veld_gen"
	}

	// ── Description prompt ──────────────────────────────────────────────────
	fmt.Print("  " + bold("◆ Description") + dim(" (optional)") + ": ")
	descLine, _ := reader.ReadString('\n')
	description := strings.TrimSpace(descLine)
	if description != "" {
		fmt.Printf("  → %s\n\n", dim(description))
	} else {
		fmt.Println()
	}

	type entry struct{ path, content, label string }
	var files []entry

	if isMicroservices {
		// ── Microservices workspace flow ──────────────────────────────────
		fmt.Print("  " + bold("◆ How many backend services?") + " [2]: ")
		numServices := readChoice(reader, 20, 2)
		fmt.Printf("  → %s\n\n", green(fmt.Sprintf("%d services", numServices)))

		// Ask whether all services share the same backend/framework.
		fmt.Print("  " + bold("◆ Are all services the same backend?") + " [Y/n]: ")
		sameLine, _ := reader.ReadString('\n')
		sameLine = strings.TrimSpace(strings.ToLower(sameLine))
		allSameBackend := sameLine == "" || sameLine == "y" || sameLine == "yes"
		if allSameBackend {
			fmt.Printf("  → %s\n\n", green(fmt.Sprintf("yes — all use %s", selectedBackend)))
		} else {
			fmt.Printf("  → %s\n\n", dim("per-service selection"))
		}

		type svcDef struct {
			name, backend, framework, baseUrl string
			consumes                          []string
		}
		var services []svcDef
		allSvcNames := []string{}

		for s := 0; s < numServices; s++ {
			fmt.Printf("  " + bold(fmt.Sprintf("◆ Service %d", s+1)) + "\n")

			fmt.Printf("    Name: ")
			nameLine, _ := reader.ReadString('\n')
			svcName := strings.TrimSpace(nameLine)
			if svcName == "" {
				svcName = fmt.Sprintf("service%d", s+1)
			}
			allSvcNames = append(allSvcNames, svcName)

			svcBackend := selectedBackend
			svcFramework := selectedBackendFramework

			if !allSameBackend {
				// Show the same backend menu as the top-level prompt.
				fmt.Printf("    Backend — choose language:\n")
				for i, b := range backends {
					fmt.Printf("      %s%2d%s  %s\n", colorGreen, i+1, colorReset, b)
				}
				defaultIdx := 1
				for i, b := range backends {
					if b == selectedBackend {
						defaultIdx = i + 1
						break
					}
				}
				fmt.Printf("    Choose [%d]: ", defaultIdx)
				bIdx := readChoice(reader, len(backends), defaultIdx)
				svcBackend = backends[bIdx-1]

				// Node TS/JS sub-choice
				if svcBackend == "node-ts" {
					fmt.Printf("      TypeScript [1] or JavaScript [2]? ")
					if readChoice(reader, 2, 1) == 2 {
						svcBackend = "node-js"
					}
				}

				// Framework for this backend
				svcFramework = ""
				if fwOpts := backendFrameworkOpts[svcBackend]; len(fwOpts) > 0 {
					fmt.Printf("    Framework:\n")
					for i, fw := range fwOpts {
						lbl := fw.name
						if lbl == "" || lbl == "plain" {
							lbl = "plain (no framework)"
						}
						fmt.Printf("      %s%2d%s  %-14s %s\n", colorGreen, i+1, colorReset, lbl, dim(fw.desc))
					}
					fmt.Printf("    Choose [1]: ")
					fwIdx := readChoice(reader, len(fwOpts), 1)
					svcFramework = fwOpts[fwIdx-1].name
					if svcFramework == "plain" {
						svcFramework = ""
					}
				}
				fmt.Printf("    → %s\n", green(svcBackend))
			}

			defaultPort := 3001 + s
			fmt.Printf("    Base URL [http://%s-service:%d]: ", svcName, defaultPort)
			urlLine, _ := reader.ReadString('\n')
			svcUrl := strings.TrimSpace(urlLine)
			if svcUrl == "" {
				svcUrl = fmt.Sprintf("http://%s-service:%d", svcName, defaultPort)
			}

			var svcConsumes []string
			if len(allSvcNames) > 1 {
				fmt.Printf("    Consumes %s: ", dim("(comma-separated, empty for none)"))
				consumesLine, _ := reader.ReadString('\n')
				consumesStr := strings.TrimSpace(consumesLine)
				if consumesStr != "" {
					for _, c := range strings.Split(consumesStr, ",") {
						c = strings.TrimSpace(c)
						if c != "" {
							svcConsumes = append(svcConsumes, c)
						}
					}
				}
			}

			services = append(services, svcDef{
				name: svcName, backend: svcBackend, framework: svcFramework, baseUrl: svcUrl, consumes: svcConsumes,
			})
			fmt.Println()
		}

		// ── Frontend entry prompt ────────────────────────────────────────
		fmt.Print("  " + bold("◆ Include frontend entry?") + " [Y/n]: ")
		feLine, _ := reader.ReadString('\n')
		feLine = strings.TrimSpace(strings.ToLower(feLine))
		includeFrontend := feLine == "" || feLine == "y" || feLine == "yes"
		if includeFrontend {
			fmt.Printf("  → %s\n\n", green("yes — "+selectedFrontend))
		} else {
			fmt.Printf("  → %s\n\n", dim("no frontend"))
		}

		// ── Build workspace config JSON ──────────────────────────────────
		var wsEntries []string
		for _, svc := range services {
			consumesJSON := "[]"
			if len(svc.consumes) > 0 {
				parts := make([]string, len(svc.consumes))
				for i, c := range svc.consumes {
					parts[i] = fmt.Sprintf("%q", c)
				}
				consumesJSON = "[" + strings.Join(parts, ", ") + "]"
			}
			consumesLine := ""
			if len(svc.consumes) > 0 {
				consumesLine = fmt.Sprintf(",\n      \"consumes\": %s", consumesJSON)
			}
			// dir is always the parent of out so veld setup can find pom.xml / package.json / etc.
			svcDir := fmt.Sprintf("../backend/%s-service", svc.name)
			backendCfgJSON := fmt.Sprintf(`{ "target": %q, "dir": %q }`, svc.backend, svcDir)
			if svc.framework != "" {
				backendCfgJSON = fmt.Sprintf(`{ "target": %q, "framework": %q, "dir": %q }`, svc.backend, svc.framework, svcDir)
			}
			wsEntries = append(wsEntries, fmt.Sprintf(`    {
      "name": %q,
      "description": "",
      "input": "services/%s/main.veld",
      "backendConfig": %s,
      "out": "../backend/%s-service/generated",
      "baseUrl": %q%s
    }`, svc.name, svc.name, backendCfgJSON, svc.name, svc.baseUrl, consumesLine))
		}

		if includeFrontend {
			// Frontend auto-consumes all backend services
			parts := make([]string, len(allSvcNames))
			for i, n := range allSvcNames {
				parts[i] = fmt.Sprintf("%q", n)
			}
			consumesAll := "[" + strings.Join(parts, ", ") + "]"
			feTarget := selectedFrontend
			if selectedFrontendFramework != "" {
				feTarget = selectedFrontendFramework
			}
			wsEntries = append(wsEntries, fmt.Sprintf(`    {
      "name": "frontend",
      "description": "Frontend SDK — auto-consumes all backend services",
      "input": "app.veld",
      "frontendConfig": { "target": %q },
      "out": "../frontend/src/generated",
      "baseUrl": "http://localhost:3000",
      "consumes": %s
    }`, feTarget, consumesAll))
		}

		descLine := ""
		if description != "" {
			descLine = fmt.Sprintf("\n  \"description\": %q,", description)
		}
		configJSON := fmt.Sprintf(`{
  "$schema": "https://veld.dev/schemas/veld.config.schema.json",%s

  "workspace": [
%s
  ]
}
`, descLine, strings.Join(wsEntries, ",\n"))

		files = append(files, entry{"veld/veld.config.json", configJSON, "veld/veld.config.json"})

		// ── Create service .veld stubs ────────────────────────────────────
		for _, svc := range services {
			moduleName := strings.ToUpper(svc.name[:1]) + svc.name[1:]

			// main.veld — the entry point that imports models + modules
			mainContent := fmt.Sprintf(`// %s service entry point
// This file is the "input" for the %s workspace entry.
// It imports all models and modules for this service.

import "./models/%s.model.veld"
import "./modules/%s.veld"
`, moduleName, svc.name, svc.name, svc.name)

			// models/<name>.model.veld — domain models
			modelContent := fmt.Sprintf(`// %s domain models

model %sItem {
  description: "A %s entity"
  id:        uuid
  name:      string
  createdAt: datetime
}

model Create%sInput {
  name: string
}
`, moduleName, moduleName, svc.name, moduleName)

			// modules/<name>.veld — module with actions (uses relative import for models)
			moduleContent := fmt.Sprintf(`// %s service module

import "../models/%s.model.veld"

module %s {
  description: "%s service"
  prefix: /api/%s

  action List%s {
    method: GET
    path: /
    output: %sItem[]
  }

  action Get%s {
    method: GET
    path: /:id
    output: %sItem
    errors: [NotFound]
  }

  action Create%s {
    method: POST
    path: /
    input: Create%sInput
    output: %sItem
  }
}
`, moduleName, svc.name, moduleName, moduleName, svc.name,
				moduleName, moduleName,
				moduleName, moduleName,
				moduleName, moduleName, moduleName)

			svcDir := fmt.Sprintf("veld/services/%s", svc.name)
			modelsDir := svcDir + "/models"
			modulesDir := svcDir + "/modules"
			files = append(files,
				entry{svcDir + "/main.veld", mainContent, svcDir + "/main.veld"},
				entry{modelsDir + "/" + svc.name + ".model.veld", modelContent, modelsDir + "/" + svc.name + ".model.veld"},
				entry{modulesDir + "/" + svc.name + ".veld", moduleContent, modulesDir + "/" + svc.name + ".veld"},
			)
		}

		// ── Create top-level app.veld for frontend entry ──────────────────
		// This file imports all service contracts so the frontend SDK covers
		// every service in the workspace.
		var appImports []string
		for _, svc := range services {
			appImports = append(appImports,
				fmt.Sprintf("import \"./services/%s/main.veld\"", svc.name))
		}
		appVeld := "// Frontend entry point — imports all service contracts.\n" +
			"// Used by the frontend workspace entry to generate a unified SDK\n" +
			"// that covers every microservice.\n\n" +
			strings.Join(appImports, "\n") + "\n"
		files = append(files, entry{"veld/app.veld", appVeld, "veld/app.veld"})

		files = append(files, entry{"README.md", initReadmeContent, "README.md"})

	} else {
		// ── Single service flow ──────────────────────────────────────────

		// Build backend config block
		backendFrameworkLine := ""
		if selectedBackendFramework != "" {
			backendFrameworkLine = fmt.Sprintf(",\n    \"framework\": %q", selectedBackendFramework)
		}
		validateLine := ""
		if enableValidate {
			validateLine = ",\n    \"validate\": true"
		}

		// Build frontend config block
		feTarget := selectedFrontend
		if selectedFrontendFramework != "" {
			feTarget = selectedFrontendFramework
		}
		frontendBlock := fmt.Sprintf(`"frontendConfig": {
    "target": %q
  }`, feTarget)
		if selectedFrontend == "none" {
			frontendBlock = `"frontendConfig": { "target": "none" }`
		}

		descLine := ""
		if description != "" {
			descLine = fmt.Sprintf("\n  \"description\": %q,", description)
		}

		configJSON := fmt.Sprintf(`{
  "$schema": "https://veld.dev/schemas/veld.config.schema.json",%s
  "input": "app.veld",

  "backendConfig": {
    "target": %q%s,
    "out": %q%s
  },

  %s,

  "baseUrl": "",
  "aliases": {}
}
`, descLine, selectedBackend, backendFrameworkLine, defaultOut, validateLine, frontendBlock)

		files = []entry{
			{"veld/veld.config.json", configJSON, "veld/veld.config.json"},
			{"veld/app.veld", appVeldContent, "veld/app.veld"},
			{"veld/models/user.model.veld", modelsUserVeldContent, "veld/models/user.model.veld"},
			{"veld/models/auth.model.veld", modelsAuthModelContent, "veld/models/auth.model.veld"},
			{"veld/models/common.model.veld", modelsCommonVeldContent, "veld/models/common.model.veld"},
			{"veld/modules/users.veld", modulesUsersVeldContent, "veld/modules/users.veld"},
			{"veld/modules/auth.veld", modulesAuthVeldContent, "veld/modules/auth.veld"},
			{"README.md", initReadmeContent, "README.md"},
		}
	}

	for _, f := range files {
		if err := os.MkdirAll(filepath.Dir(f.path), 0755); err != nil {
			return err
		}
		if err := os.WriteFile(f.path, []byte(f.content), 0644); err != nil {
			return err
		}
		fmt.Printf("  "+green("✓")+" %s\n", f.label)
	}

	fmt.Println()
	fmt.Println("  " + bold("Veld project ready."))
	if isMicroservices {
		fmt.Printf("    mode:     %s\n", bold("microservices workspace"))
	} else {
		fmt.Printf("    backend:  %s\n", bold(selectedBackend))
		fmt.Printf("    frontend: %s\n", bold(selectedFrontend))
	}
	if description != "" {
		fmt.Printf("    desc:     %s\n", dim(description))
	}
	fmt.Println()

	// ── Setup prompt (single-service only) ────────────────────────────────
	// Microservices workspaces configure paths per-service in the config.
	if !isMicroservices {
		fmt.Print("  Run setup to configure imports? [Y/n]: ")
		setupLine, _ := reader.ReadString('\n')
		setupLine = strings.TrimSpace(strings.ToLower(setupLine))
		if setupLine == "" || setupLine == "y" || setupLine == "yes" {
			var backendDirPath, frontendDirPath string

			// ── Ask for backend project directory ──────────────────────────
			fmt.Println()
			fmt.Print("  " + bold("Backend project directory") + dim(" (leave empty for current dir)") + ": ")
			bLine, _ := reader.ReadString('\n')
			bLine = strings.TrimSpace(bLine)
			if bLine != "" {
				abs, err := filepath.Abs(bLine)
				if err == nil {
					backendDirPath = abs
				}
			}

			// ── Ask for frontend project directory ─────────────────────────
			if selectedFrontend != "none" {
				fmt.Print("  " + bold("Frontend project directory") + dim(" (leave empty for current dir)") + ": ")
				fLine, _ := reader.ReadString('\n')
				fLine = strings.TrimSpace(fLine)
				if fLine != "" {
					abs, err := filepath.Abs(fLine)
					if err == nil {
						frontendDirPath = abs
					}
				}
			}

			// ── Update config file with backendDir / frontendDir ───────────
			if backendDirPath != "" || frontendDirPath != "" {
				cfgDir, _ := filepath.Abs("veld")
				relBackend := ""
				relFrontend := ""
				relBackendOut := ""
				relFrontendOut := ""
				if backendDirPath != "" {
					if r, err := filepath.Rel(cfgDir, backendDirPath); err == nil {
						relBackend = filepath.ToSlash(r)
					} else {
						relBackend = filepath.ToSlash(backendDirPath)
					}
				}
				if frontendDirPath != "" {
					if r, err := filepath.Rel(cfgDir, frontendDirPath); err == nil {
						relFrontend = filepath.ToSlash(r)
					} else {
						relFrontend = filepath.ToSlash(frontendDirPath)
					}
				}

				// When backend and frontend are in different directories, auto-set
				// backendOut / frontendOut so generated code lands inside each project.
				if backendDirPath != "" && frontendDirPath != "" && backendDirPath != frontendDirPath {
					genName := "generated"
					if selectedBackend == "python" {
						genName = "veld_gen"
					}
					relBackendOut = relBackend + "/src/" + genName
					relFrontendOut = relFrontend + "/src/" + genName

					fmt.Println()
					fmt.Println(dim("  Split output detected:"))
					fmt.Printf("    backend output:  %s\n", bold(relBackendOut))
					fmt.Printf("    frontend output: %s\n", bold(relFrontendOut))
				}

				backendOutLine := ""
				frontendOutLine := ""
				if relBackendOut != "" {
					backendOutLine = fmt.Sprintf(",\n    \"out\": %q", relBackendOut)
				}
				if relFrontendOut != "" {
					frontendOutLine = fmt.Sprintf(",\n    \"out\": %q", relFrontendOut)
				}

				backendFWLine := ""
				if selectedBackendFramework != "" {
					backendFWLine = fmt.Sprintf(",\n    \"framework\": %q", selectedBackendFramework)
				}

				feTarget := selectedFrontend
				if selectedFrontendFramework != "" {
					feTarget = selectedFrontendFramework
				}

				descLine := ""
				if description != "" {
					descLine = fmt.Sprintf("\n  \"description\": %q,", description)
				}

				updatedCfg := fmt.Sprintf(`{
  "$schema": "https://veld.dev/schemas/veld.config.schema.json",%s
  "input": "app.veld",

  "backendConfig": {
    "target": %q%s,
    "dir": %q%s
  },

  "frontendConfig": {
    "target": %q,
    "dir": %q%s
  },

  "baseUrl": "",
  "aliases": {}
}
`, descLine, selectedBackend, backendFWLine, relBackend, backendOutLine, feTarget, relFrontend, frontendOutLine)
				_ = os.WriteFile("veld/veld.config.json", []byte(updatedCfg), 0644)
				fmt.Println("  " + green("✓") + " updated veld.config.json with project paths")
			}

			// ── Run setup ──────────────────────────────────────────────────
			projectDir, _ := os.Getwd()
			setupOpts := setup.Options{
				BackendDir:  backendDirPath,
				FrontendDir: frontendDirPath,
			}
			// If split output was detected, compute absolute paths for setup
			if backendDirPath != "" && frontendDirPath != "" && backendDirPath != frontendDirPath {
				genName := "generated"
				if selectedBackend == "python" {
					genName = "veld_gen"
				}
				setupOpts.BackendOutDir = filepath.Join(backendDirPath, "src", genName)
				setupOpts.FrontendOutDir = filepath.Join(frontendDirPath, "src", genName)
			}
			results := setup.Run(projectDir, selectedBackend, selectedFrontend, defaultOut, setupOpts)
			if len(results) > 0 {
				printSetupResults(results)
			}
		}
	} // end if !isMicroservices

	fmt.Println()
	fmt.Println("  Next steps:")
	if isMicroservices {
		fmt.Println("    1. Edit veld/services/<name>/main.veld, models/, and modules/ to define each service's API")
		fmt.Println("    2. Run: " + bold("veld setup") + "  — patch tsconfig / package.json / pom.xml in each service")
		fmt.Println("    3. Run: " + bold("veld generate"))
		fmt.Println("    4. Run: " + bold("veld deps") + " to view the service dependency graph")
	} else {
		fmt.Println("    1. Edit veld/models/ and veld/modules/ to define your API")
		fmt.Println("    2. Run: " + bold("veld generate"))
	}
	return nil
}

// readChoice reads a number from stdin, returning def if the user presses Enter.
func readChoice(reader *bufio.Reader, max, def int) int {
	line, _ := reader.ReadString('\n')
	line = strings.TrimSpace(line)
	if line == "" {
		return def
	}
	n, err := strconv.Atoi(line)
	if err != nil || n < 1 || n > max {
		return def
	}
	return n
}

// ── init templates ────────────────────────────────────────────────────────────

const appVeldContent = `// ── Veld Entry Point ─────────────────────────────────────────────────
//
// This file is the root of your Veld contract. It imports all modules.
// Each module file imports the model files it needs.
//
// How it works:
//   1. Define data types in veld/models/ (models, enums)
//   2. Define API endpoints in veld/modules/ (modules with actions)
//   3. Run "veld generate" to produce typed code in generated/
//
// Import syntax:
//   import @models/user       → loads veld/models/user.veld
//   import @modules/auth      → loads veld/modules/auth.veld
//   import @models/*          → loads all .veld files in veld/models/
//
// Every file must explicitly import the files that define the types it
// uses. Veld will error if a type is referenced but not imported.
//
// Middleware names (like RequireAuth) are just labels — you provide the
// actual middleware functions when you register routes in your app.
//
// Run "veld validate" at any time to check your contract for errors.
// ─────────────────────────────────────────────────────────────────────

import @modules/users
import @modules/auth
`

const modelsUserVeldContent = `// User domain models and enums.

enum UserRole {
  admin
  user
  guest
}

model User {
  description: "A platform user"
  id:        uuid
  email:     string
  name:      string
  bio?:      string
  role:      UserRole   @default(user)
  verified:  bool       @default(false)
  createdAt: datetime
}

model CreateUserInput {
  description: "Data required to create a new user"
  email:    string
  name:     string
  password: string
}

model UpdateUserInput {
  description: "Fields that can be updated on a user"
  name?: string
  bio?:  string
  role?: UserRole
}
`

const modelsAuthModelContent = `// Authentication request and response models.

import @models/user.model

model LoginInput {
  description: "Credentials for user login"
  email:    string
  password: string
}

model RegisterInput {
  description: "Data for new account registration"
  email:    string
  name:     string
  password: string
}

model AuthToken {
  description: "Token returned after successful authentication"
  token: string
  user:  User
}
`

const modelsCommonVeldContent = `// Shared types used across multiple modules.

model SuccessMessage {
  description: "Generic success response"
  success: bool
  message?: string
}

model ListQuery {
  description: "Common query parameters for list endpoints"
  search?: string
  limit?:  int
  offset?: int
}
`

const modulesUsersVeldContent = `// Users module — CRUD endpoints for user management.

import @models/user.model
import @models/common.model

module Users {
  description: "User management"
  prefix:      /api/users

  action ListUsers {
    description: "List all users with optional filters"
    method:      GET
    path:        /
    query:       ListQuery
    output:      User[]
  }

  action GetUser {
    description: "Get a single user by ID"
    method:      GET
    path:        /:id
    output:      User
  }

  action CreateUser {
    description: "Create a new user"
    method:      POST
    path:        /
    input:       CreateUserInput
    output:      User
  }

  action UpdateUser {
    description: "Update an existing user"
    method:      PUT
    path:        /:id
    input:       UpdateUserInput
    output:      User
  }

  action DeleteUser {
    description: "Delete a user"
    method:      DELETE
    path:        /:id
    output:      SuccessMessage
  }
}
`

const modulesAuthVeldContent = `// Auth module — authentication and session management.
// Middleware names are labels — you provide the actual functions at runtime.

import @models/user.model
import @models/auth.model
import @models/common.model

module Auth {
  description: "Authentication and session management"
  prefix:      /api/auth

  action Login {
    description: "Log in with credentials"
    method:      POST
    path:        /login
    input:       LoginInput
    output:      AuthToken
    middleware:   RateLimit
  }

  action Register {
    description: "Register a new account"
    method:      POST
    path:        /register
    input:       RegisterInput
    output:      AuthToken
    middleware:   RateLimit
  }

  action GetMe {
    description: "Get the currently authenticated user"
    method:      GET
    path:        /me
    output:      User
    middleware:   RequireAuth
  }

  action Logout {
    description: "Log out and invalidate session"
    method:      POST
    path:        /logout
    output:      SuccessMessage
    middleware:   RequireAuth
  }
}
`

const initReadmeContent = "# My Veld Project\n\n" +
	"## Structure\n\n" +
	"| Path | Purpose |\n" +
	"|------|--------|\n" +
	"| `veld/` | Contract source — models, modules, config |\n" +
	"| `veld/models/` | Data type definitions (models, enums) |\n" +
	"| `veld/modules/` | API endpoint definitions |\n" +
	"| `generated/` | Auto-generated code — do not edit |\n\n" +
	"## Import System\n\n" +
	"Every file must explicitly import the files that define the types it uses:\n\n" +
	"```veld\n" +
	"// veld/app.veld — imports modules\n" +
	"import @modules/users\n" +
	"import @modules/auth\n" +
	"```\n\n" +
	"```veld\n" +
	"// veld/modules/users.veld — imports its own models\n" +
	"import @models/user.model\n" +
	"import @models/common.model\n\n" +
	"module Users { ... }\n" +
	"```\n\n" +
	"Import paths don't include `.veld` — the parser adds it automatically.\n\n" +
	"## Middleware\n\n" +
	"Middleware names (like `RequireAuth`, `RateLimit`) are just labels in the contract.\n" +
	"Veld generates typed middleware interfaces — you provide the implementations\n" +
	"when registering routes in your app.\n\n" +
	"## Commands\n\n" +
	"| Command | Description |\n" +
	"|---------|-------------|\n" +
	"| `veld generate` | Generate typed code |\n" +
	"| `veld validate` | Check contract for errors |\n" +
	"| `veld lint` | Analyse contract quality |\n" +
	"| `veld fmt` | Format .veld files |\n" +
	"| `veld watch` | Auto-regenerate on file save |\n" +
	"| `veld clean` | Remove generated output |\n" +
	"| `veld openapi` | Export OpenAPI 3.0 spec |\n" +
	"| `veld diff` | Show changes since last gen |\n" +
	"| `veld setup` | Auto-configure project imports |\n" +
	"| `veld doctor` | Diagnose project health |\n" +
	"| `veld ast` | Dump AST JSON for debugging |\n"

	// ── login ─────────────────────────────────────────────────────────────────────
