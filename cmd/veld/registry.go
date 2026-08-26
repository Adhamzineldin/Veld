package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/config"
	"github.com/Adhamzineldin/Veld/internal/diff"
	"github.com/Adhamzineldin/Veld/internal/docsgen"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/loader"
	"github.com/Adhamzineldin/Veld/internal/registry"
	"github.com/Adhamzineldin/Veld/internal/setup"
	"github.com/spf13/cobra"
	// Register all emitters via init(). To add a new emitter, add one line here.
	// Register tool emitters (auxiliary generators — NOT backends).
)

func newLoginCmd() *cobra.Command {
	var registryURL, token string
	cmd := &cobra.Command{
		Use:   "login",
		Short: "Authenticate with a Veld Registry using an API token",
		Long: `Authenticate the CLI with a Veld Registry using an API token.

To create a token:
  1. Open the registry web UI in your browser
  2. Go to Settings → API Tokens → New Token
  3. Copy the generated token (it is only shown once)
  4. Run: veld registry login --registry <url> --token vtk_...`,
		Example: "  veld registry login --registry https://registry.veld.dev --token vtk_...\n" +
			"  veld registry login --registry http://localhost:8080 --token vtk_...",
		RunE: func(cmd *cobra.Command, args []string) error {
			if registryURL == "" {
				return fmt.Errorf("--registry is required (e.g. --registry https://registry.veld.dev)")
			}
			if token == "" {
				registryBase := strings.TrimRight(registryURL, "/")
				fmt.Printf("To log in, create an API token in the web UI:\n")
				fmt.Printf("  %s/#/tokens\n\n", registryBase)
				fmt.Printf("Then run:\n")
				fmt.Printf("  veld registry login --registry %s --token vtk_...\n", registryURL)
				return nil
			}
			client := registry.NewClient(registryURL, token)
			me, err := client.Me()
			if err != nil {
				return fmt.Errorf("token validation failed: %w", err)
			}
			username, _ := me["username"].(string)
			if err := registry.SetToken(registryURL, token, username); err != nil {
				return err
			}
			fmt.Printf(green("✓")+" Logged in to %s as %s\n", registryURL, username)
			return nil
		},
	}
	cmd.Flags().StringVar(&registryURL, "registry", "", "registry URL")
	cmd.Flags().StringVar(&token, "token", "", "API token (vtk_...)")
	return cmd
}

// ── logout ────────────────────────────────────────────────────────────────────

func newLogoutCmd() *cobra.Command {
	var registryURL string
	cmd := &cobra.Command{
		Use:   "logout",
		Short: "Remove stored credentials for a registry",
		RunE: func(cmd *cobra.Command, args []string) error {
			if registryURL == "" {
				registryURL = registry.DefaultRegistry()
			}
			if registryURL == "" {
				return fmt.Errorf("no registry configured")
			}
			if err := registry.ClearToken(registryURL); err != nil {
				return err
			}
			fmt.Printf(green("✓")+" Logged out from %s\n", registryURL)
			return nil
		},
	}
	cmd.Flags().StringVar(&registryURL, "registry", "", "registry URL (defaults to current)")
	return cmd
}

// ── push ──────────────────────────────────────────────────────────────────────

func newPushCmd() *cobra.Command {
	var registryURL, orgName, pkgName, version string
	cmd := &cobra.Command{
		Use:   "push",
		Short: "Publish .veld contracts to the registry",
		Example: "  veld registry push\n" +
			"  veld registry push --registry https://registry.veld.dev\n" +
			"  veld registry push --org acme --name auth --version 1.2.0",
		RunE: func(cmd *cobra.Command, args []string) error {
			// Resolve config for org/name/version defaults
			rc, err := config.BuildResolved(config.FlagOverrides{})
			if err != nil {
				return fmt.Errorf("could not load veld.config.json: %w", err)
			}

			// Defaults from config
			if orgName == "" {
				orgName = rc.Registry.Org
			}
			if pkgName == "" {
				pkgName = rc.Registry.Package
			}
			if version == "" {
				version = rc.Registry.Version
			}
			if registryURL == "" {
				registryURL = rc.Registry.URL
			}
			if orgName == "" || pkgName == "" || version == "" {
				return fmt.Errorf("--org, --name, and --version are required (or set registry.org/package/version in veld.config.json)")
			}

			client, err := registry.NewClientFromCreds(registryURL)
			if err != nil {
				return err
			}

			fmt.Printf(dim("⬡")+"  Packing contracts from %s…\n", rc.ConfigDir)
			tarPath, sha, err := registry.Pack(rc.ConfigDir)
			if err != nil {
				return fmt.Errorf("pack failed: %w", err)
			}
			defer func() {
				if err := os.Remove(tarPath); err == nil && verbose {
					fmt.Println(dim("  removed temp tarball"))
				}
			}()

			f, err := os.Open(tarPath)
			if err != nil {
				return err
			}
			defer f.Close()

			fi, _ := f.Stat()
			fmt.Printf(dim("⬡")+"  Publishing @%s/%s@%s (%s)…\n",
				orgName, pkgName, version, fmtBytes(fi.Size()))

			manifestJSON := fmt.Sprintf(`{"org":%q,"name":%q,"version":%q}`, orgName, pkgName, version)
			result, err := client.Publish(manifestJSON, pkgName+"-"+version+".tar.gz", f)
			if err != nil {
				return err
			}
			_ = sha
			fmt.Printf(green("✓")+" Published @%s/%s@%s\n%s\n",
				orgName, pkgName, version, dim(string(result)))
			return nil
		},
	}
	cmd.Flags().StringVar(&registryURL, "registry", "", "registry URL (default: from credentials)")
	cmd.Flags().StringVar(&orgName, "org", "", "organisation name")
	cmd.Flags().StringVar(&pkgName, "name", "", "package name")
	cmd.Flags().StringVar(&version, "version", "", "semver version to publish")
	return cmd
}

// ── pull ──────────────────────────────────────────────────────────────────────

func newPullCmd() *cobra.Command {
	var registryURL, outDir string
	cmd := &cobra.Command{
		Use:   "pull <@org/name[@version]>",
		Short: "Download a contract package from the registry",
		Example: "  veld registry pull @acme/auth\n" +
			"  veld registry pull @acme/auth@1.2.0\n" +
			"  veld registry pull @acme/auth --out veld/packages",
		Args: cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			orgName, pkgName, version, err := parsePackageRef(args[0])
			if err != nil {
				return err
			}

			client, err := registry.NewClientFromCreds(registryURL)
			if err != nil {
				// Allow unauthenticated for public packages
				if registryURL == "" {
					registryURL = registry.DefaultRegistry()
				}
				if registryURL == "" {
					return err
				}
				client = registry.NewClient(registryURL, "")
			}

			// Resolve "latest" version
			if version == "" || version == "latest" {
				versions, err := client.ListPackageVersions(orgName, pkgName)
				if err != nil {
					return fmt.Errorf("could not fetch versions: %w", err)
				}
				if len(versions) == 0 {
					return fmt.Errorf("no versions published for @%s/%s", orgName, pkgName)
				}
				version, _ = versions[0]["version"].(string)
			}

			if outDir == "" {
				outDir = filepath.Join("veld", "packages", "@"+orgName, pkgName)
			}
			if err := os.MkdirAll(outDir, 0755); err != nil {
				return err
			}

			fmt.Printf(dim("⬡")+"  Pulling @%s/%s@%s → %s\n", orgName, pkgName, version, outDir)

			// Stream tarball to temp file, verify SHA, extract
			tmp, err := os.CreateTemp("", "veld-pull-*.tar.gz")
			if err != nil {
				return err
			}
			defer os.Remove(tmp.Name())

			remoteSHA, err := client.Download(orgName, pkgName, version, tmp)
			tmp.Close()
			if err != nil {
				return fmt.Errorf("download failed: %w", err)
			}
			if remoteSHA != "" {
				if err := registry.VerifySHA(tmp.Name(), remoteSHA); err != nil {
					return fmt.Errorf("integrity check failed: %w", err)
				}
			}

			if err := registry.Unpack(tmp.Name(), outDir); err != nil {
				return fmt.Errorf("extract failed: %w", err)
			}

			fmt.Printf(green("✓")+" Pulled @%s/%s@%s\n", orgName, pkgName, version)
			fmt.Printf(dim("   Import with: import @%s/%s/ModelName\n"), orgName, pkgName)
			return nil
		},
	}
	cmd.Flags().StringVar(&registryURL, "registry", "", "registry URL")
	cmd.Flags().StringVar(&outDir, "out", "", "output directory (default: veld/packages/@org/name)")
	return cmd
}

// ── export (subcommand group) ─────────────────────────────────────────────────

func newExportCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "export",
		Short: "Export contract to other formats (OpenAPI, GraphQL, SQL, docs)",
	}
	cmd.AddCommand(newOpenAPICmd(), newGraphQLCmd(), newSchemaCmd(), newDocsCmd(), newAgentsCmd())
	return cmd
}

func newAgentsCmd() *cobra.Command {
	var outputFlag string
	cmd := &cobra.Command{
		Use:     "agents",
		Short:   "Generate AGENTS.md for AI discoverability",
		Long:    "Generates a compact Markdown file describing the full API contract,\noptimised for AI assistants (Claude, Copilot, Cursor) to ingest in one read.",
		Example: "  veld export agents\n  veld export agents -o AGENTS.md",
		RunE: func(cmd *cobra.Command, args []string) error {
			rc, err := config.BuildResolved(config.FlagOverrides{})
			if err != nil {
				return err
			}
			a, _, err := loader.Parse(rc.Input, rc.Aliases)
			if err != nil {
				return err
			}
			a = emitter.ApplyTopLevelPrefix(a)
			content := docsgen.BuildAgentsMd(a, rc)
			if outputFlag == "" {
				fmt.Print(content)
				return nil
			}
			if err := os.WriteFile(outputFlag, []byte(content), 0644); err != nil {
				return fmt.Errorf("writing %s: %w", outputFlag, err)
			}
			fmt.Printf("  %s %s\n", green("✓"), outputFlag)
			return nil
		},
	}
	cmd.Flags().StringVarP(&outputFlag, "output", "o", "", "write to file instead of stdout")
	return cmd
}

// ── registry (subcommand group) ───────────────────────────────────────────────

func newRegistryCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "registry",
		Short: "Manage registry connections and packages",
	}

	// veld registry info
	cmd.AddCommand(&cobra.Command{
		Use:   "info",
		Short: "Show current registry and logged-in user",
		RunE: func(cmd *cobra.Command, args []string) error {
			url := registry.DefaultRegistry()
			if url == "" {
				fmt.Println("No registry configured. Run: veld registry login --registry <url>")
				return nil
			}
			token := registry.GetToken(url)
			client := registry.NewClient(url, token)
			me, err := client.Me()
			if err != nil {
				fmt.Printf("Registry: %s\nStatus:   %s\n", url, red("not authenticated"))
				return nil
			}
			fmt.Printf("Registry: %s\nUser:     %s (%s)\n", url, me["username"], me["email"])
			return nil
		},
	})

	// veld registry list
	cmd.AddCommand(&cobra.Command{
		Use:   "list",
		Short: "List all configured registries",
		Run: func(cmd *cobra.Command, args []string) {
			registry.ListRegistries()
		},
	})

	// veld registry versions <@org/name>
	versionsCmd := &cobra.Command{
		Use:   "versions <@org/name>",
		Short: "List all published versions of a package",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			orgName, pkgName, _, err := parsePackageRef(args[0])
			if err != nil {
				return err
			}
			client, err := registry.NewClientFromCreds("")
			if err != nil {
				return err
			}
			versions, err := client.ListPackageVersions(orgName, pkgName)
			if err != nil {
				return err
			}
			if len(versions) == 0 {
				fmt.Printf("No versions published for @%s/%s\n", orgName, pkgName)
				return nil
			}
			fmt.Printf("@%s/%s — %d version(s):\n", orgName, pkgName, len(versions))
			for _, v := range versions {
				ver, _ := v["version"].(string)
				dep, _ := v["deprecated"].(string)
				line := "  " + bold("v"+ver)
				if dep != "" {
					line += " " + yellow("[deprecated: "+dep+"]")
				}
				fmt.Println(line)
			}
			return nil
		},
	}
	cmd.AddCommand(versionsCmd)

	// veld registry login / logout / push / pull / serve
	cmd.AddCommand(newLoginCmd(), newLogoutCmd(), newPushCmd(), newPullCmd(), newServeCmd())

	// veld registry init
	cmd.AddCommand(newRegistryInitCmd())

	// veld registry token create
	tokenCmd := &cobra.Command{
		Use:   "token",
		Short: "Manage API tokens",
	}
	var tokenName string
	var tokenScopes []string
	tokenCreateCmd := &cobra.Command{
		Use:   "create",
		Short: "Create a new API token",
		RunE: func(cmd *cobra.Command, args []string) error {
			if tokenName == "" {
				return fmt.Errorf("--name is required")
			}
			client, err := registry.NewClientFromCreds("")
			if err != nil {
				return err
			}
			body := map[string]interface{}{
				"name":   tokenName,
				"scopes": tokenScopes,
			}
			data, err := client.PostJSON("/tokens", body)
			if err != nil {
				return err
			}
			fmt.Printf(green("✓")+" Token created. Copy it now — it will not be shown again:\n\n  %s\n\n", data)
			return nil
		},
	}
	tokenCreateCmd.Flags().StringVar(&tokenName, "name", "", "token name")
	tokenCreateCmd.Flags().StringSliceVar(&tokenScopes, "scopes", []string{"read"}, "comma-separated scopes: read,write,delete")
	tokenCmd.AddCommand(tokenCreateCmd)
	cmd.AddCommand(tokenCmd)

	return cmd
}

// ── ci ────────────────────────────────────────────────────────────────────────

// newCICmd returns the `veld ci` command — a single non-interactive command
// that runs generate + setup and exits with the correct status code.
//
// Replace this in any Dockerfile, pipeline, or script:
//
//	npx veld generate && npx veld setup
//
// With:
//
//	npx veld ci
func newCICmd() *cobra.Command {
	var backendFlag, frontendFlag, inputFlag, outFlag string
	var strictFlag bool

	cmd := &cobra.Command{
		Use:   "ci",
		Short: "Generate code and configure project paths in one step (non-interactive)",
		Long: `Run veld generate then veld setup in a single non-interactive command.

Reads backend and frontend from veld.config.json automatically.
Never prompts — safe to run in any pipeline, Dockerfile, or script.
Exits 0 on success, 1 on any failure.

Replace this:
  npx veld generate
  npx veld setup

With:
  npx veld ci`,
		Example: "  npx veld ci\n  veld ci\n  veld ci --strict",
		RunE: func(cmd *cobra.Command, args []string) error {
			flags := config.FlagOverrides{
				Backend:     backendFlag,
				Frontend:    frontendFlag,
				Input:       inputFlag,
				Out:         outFlag,
				BackendSet:  cmd.Flags().Changed("backend"),
				FrontendSet: cmd.Flags().Changed("frontend"),
				InputSet:    cmd.Flags().Changed("input"),
				OutSet:      cmd.Flags().Changed("out"),
			}
			rc, err := config.BuildResolved(flags)
			if err != nil {
				return fmt.Errorf("could not load veld.config.json: %w", err)
			}

			// ── step 1: generate ─────────────────────────────────────────────
			fmt.Printf(dim("⬡")+"  Generating (backend=%s frontend=%s)…\n", rc.Backend, rc.Frontend)

			// Check for breaking changes before emitting.
			if preChanges := computePreChanges(rc); diff.HasBreaking(preChanges) {
				printDiffChanges(preChanges)
				if strictFlag {
					fmt.Fprintln(os.Stderr, red("✗")+" Breaking changes detected — aborting (--strict)")
					return fmt.Errorf("breaking changes blocked by --strict")
				}
				// Non-strict: warn but continue — no interactive prompt in CI.
				fmt.Fprintln(os.Stderr, yellow("⚠")+"  Breaking changes detected — continuing (pass --strict to block)")
			}

			opts := emitter.EmitOptions{
				BaseUrl: rc.BaseUrl,
			}
			generatedFiles, _, _, err := runGenerate(rc, false, opts)
			if err != nil {
				return fmt.Errorf("generate failed: %w", err)
			}

			if len(generatedFiles) > 0 {
				fmt.Printf(green("✓")+" Generated %d file(s) → %s\n", len(generatedFiles), rc.Out)
			} else {
				fmt.Printf(green("✓")+" Generated → %s\n", rc.Out)
			}

			runPostGenerate(rc)

			// ── step 2: setup ────────────────────────────────────────────────
			fmt.Printf(dim("⬡") + "  Configuring project paths…\n")

			projectDir, _ := os.Getwd()
			results := setup.Run(projectDir, rc.Backend, rc.Frontend, rc.Out, setup.Options{
				BackendDir:     rc.BackendDir,
				FrontendDir:    rc.FrontendDir,
				BackendOutDir:  rc.BackendOut,
				FrontendOutDir: rc.FrontendOut,
			})

			patched, alreadyOK := 0, 0
			for _, r := range results {
				switch r.Action {
				case "patched":
					patched++
					fmt.Printf("  %s %s — %s\n", green("✓"), r.File, r.Detail)
				case "skipped":
					alreadyOK++
					fmt.Printf("  %s %s — %s\n", dim("·"), r.File, dim(r.Detail))
				case "not-found":
					fmt.Printf("  %s %s — %s\n", yellow("!"), r.File, r.Detail)
				case "manual":
					fmt.Printf("  %s %s — %s\n", dim("→"), r.File, r.Detail)
				}
			}

			switch {
			case patched > 0:
				fmt.Printf(green("✓")+" Setup patched %d file(s)\n", patched)
			case alreadyOK > 0:
				fmt.Printf(green("✓") + " Setup already configured\n")
			default:
				fmt.Printf(dim("·") + "  No config files to patch for this stack\n")
			}

			return nil
		},
	}

	cmd.Flags().StringVar(&backendFlag, "backend", "", "backend override")
	cmd.Flags().StringVar(&frontendFlag, "frontend", "", "frontend override")
	cmd.Flags().StringVar(&inputFlag, "input", "", "input .veld file override")
	cmd.Flags().StringVar(&outFlag, "out", "", "output directory override")
	cmd.Flags().BoolVar(&strictFlag, "strict", false, "exit 1 on breaking changes")
	return cmd
}

// ── registry init ─────────────────────────────────────────────────────────────

func newRegistryInitCmd() *cobra.Command {
	var flagAddr, flagDSN, flagSecret, flagStorage, flagBaseURL, flagOut string

	cmd := &cobra.Command{
		Use:   "init",
		Short: "Scaffold a registry.config.json for self-hosting",
		Long: `Create a registry.config.json file with your server configuration.

Missing required values (DSN, secret) will be prompted interactively.
Pass --yes to skip prompts and write defaults (useful in scripts).`,
		Example: `  veld registry init
  veld registry init --dsn "postgres://localhost/veld?sslmode=disable" --secret $(openssl rand -hex 32)
  veld registry init --out /etc/veld/registry.config.json`,
		RunE: func(cmd *cobra.Command, args []string) error {
			outPath := flagOut
			if outPath == "" {
				outPath = "registry.config.json"
			}

			// Don't overwrite
			if _, err := os.Stat(outPath); err == nil {
				return fmt.Errorf("%s already exists — delete it first or use --out to choose a different path", outPath)
			}

			reader := bufio.NewReader(os.Stdin)

			prompt := func(label, def string) string {
				if def != "" {
					fmt.Printf("  %s [%s]: ", label, dim(def))
				} else {
					fmt.Printf("  %s: ", label)
				}
				line, _ := reader.ReadString('\n')
				line = strings.TrimSpace(line)
				if line == "" {
					return def
				}
				return line
			}

			fmt.Println(bold("Veld Registry — configuration setup"))
			fmt.Println()

			addr := flagAddr
			if addr == "" {
				addr = prompt("Listen address", ":8080")
			}

			dsn := flagDSN
			if dsn == "" {
				dsn = prompt("PostgreSQL DSN", "postgres://localhost/veld?sslmode=disable")
			}
			if dsn == "" {
				return fmt.Errorf("DSN is required")
			}

			secret := flagSecret
			if secret == "" {
				fmt.Println()
				fmt.Println(dim("  Tip: generate a secret with: openssl rand -hex 32"))
				secret = prompt("JWT secret (min 16 chars)", "")
			}
			if secret == "" {
				return fmt.Errorf("JWT secret is required")
			}
			if len(secret) < 16 {
				return fmt.Errorf("JWT secret must be at least 16 characters")
			}

			storage := flagStorage
			if storage == "" {
				storage = prompt("Tarball storage path", "./packages")
			}

			baseURL := flagBaseURL
			if baseURL == "" {
				baseURL = prompt("Public base URL (optional)", "")
			}

			cfg := map[string]interface{}{
				"addr":    addr,
				"dsn":     dsn,
				"secret":  secret,
				"storage": storage,
			}
			if baseURL != "" {
				cfg["base_url"] = baseURL
			}
			cfg["smtp"] = map[string]interface{}{
				"host":     "",
				"port":     587,
				"username": "",
				"password": "",
				"from":     "",
			}

			data, err := json.MarshalIndent(cfg, "", "  ")
			if err != nil {
				return err
			}

			if err := os.WriteFile(outPath, data, 0600); err != nil {
				return err
			}

			fmt.Println()
			fmt.Printf(green("✓")+" Created %s\n", bold(outPath))
			fmt.Println()
			fmt.Println("Next steps:")
			fmt.Printf("  1. %s\n", dim("Start the registry:"))
			fmt.Printf("     veld registry serve --config %s\n", outPath)
			fmt.Printf("  2. %s\n", dim("Open the web UI and create your account:"))
			fmt.Printf("     http://localhost%s\n", addr)
			fmt.Printf("  3. %s\n", dim("Log in from the CLI:"))
			fmt.Printf("     veld registry login --registry http://localhost%s --token vtk_...\n", addr)
			return nil
		},
	}

	cmd.Flags().StringVar(&flagAddr, "addr", "", "listen address (default :8080)")
	cmd.Flags().StringVar(&flagDSN, "dsn", "", "PostgreSQL DSN")
	cmd.Flags().StringVar(&flagSecret, "secret", "", "JWT signing secret (min 16 chars)")
	cmd.Flags().StringVar(&flagStorage, "storage", "", "tarball storage path (default ./packages)")
	cmd.Flags().StringVar(&flagBaseURL, "base-url", "", "public base URL (e.g. https://registry.example.com)")
	cmd.Flags().StringVar(&flagOut, "out", "", "output path (default ./registry.config.json)")
	return cmd
}

// ── serve (registry server) ───────────────────────────────────────────────────

// serveRegistryConfig mirrors registry.config.json on disk.
