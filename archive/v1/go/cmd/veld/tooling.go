package main

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/Adhamzineldin/Veld/internal/config"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/lint"
	"github.com/Adhamzineldin/Veld/internal/loader"
	"github.com/Adhamzineldin/Veld/internal/lsp"
	"github.com/Adhamzineldin/Veld/internal/setup"
	"github.com/Adhamzineldin/Veld/internal/validator"
	"github.com/spf13/cobra"
	// Register all emitters via init(). To add a new emitter, add one line here.
	// Register tool emitters (auxiliary generators — NOT backends).
)

func printSetupResults(results []setup.Result) {
	if len(results) == 0 {
		return
	}
	fmt.Println()
	fmt.Println(dim("  Setup:"))
	for _, r := range results {
		switch r.Action {
		case "patched":
			fmt.Printf("  %s %s — %s\n", green("✓"), r.File, r.Detail)
		case "skipped":
			fmt.Printf("  %s %s — %s\n", dim("·"), r.File, dim(r.Detail))
		case "not-found":
			fmt.Printf("  %s %s — %s\n", yellow("!"), r.File, r.Detail)
		case "manual":
			fmt.Printf("  %s %s — %s\n", dim("→"), r.File, r.Detail)
		}
	}
}

func newSetupCmd() *cobra.Command {
	var backendDirFlag, frontendDirFlag string
	var backendFlag, frontendFlag, inputFlag, outFlag string

	cmd := &cobra.Command{
		Use:   "setup",
		Short: "Auto-configure project files for seamless imports",
		Long: "Patches config files (tsconfig.json, pubspec.yaml, go.mod, etc.) so that\n" +
			"generated code can be imported without manual edits.\n\n" +
			"Reads backend/frontend from veld.config.json and applies the appropriate patches.\n" +
			"If the generated output path has changed, existing entries are updated in place.\n\n" +
			"Use --backend-dir / --frontend-dir to point at project folders outside the\n" +
			"current directory, so you don't need a config file in each folder.",
		Example: "  veld setup\n" +
			"  veld setup --backend-dir=../server --frontend-dir=../client\n" +
			"  veld setup --out=./output --backend=go --frontend=react",
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
				return fmt.Errorf("could not load config: %w", err)
			}
			projectDir, _ := os.Getwd()

			opts := setup.Options{}
			if cmd.Flags().Changed("backend-dir") {
				abs, err := filepath.Abs(backendDirFlag)
				if err != nil {
					return fmt.Errorf("invalid --backend-dir: %w", err)
				}
				opts.BackendDir = abs
			} else if rc.BackendDir != "" {
				opts.BackendDir = rc.BackendDir
			}
			if cmd.Flags().Changed("frontend-dir") {
				abs, err := filepath.Abs(frontendDirFlag)
				if err != nil {
					return fmt.Errorf("invalid --frontend-dir: %w", err)
				}
				opts.FrontendDir = abs
			} else if rc.FrontendDir != "" {
				opts.FrontendDir = rc.FrontendDir
			}

			// Workspace mode: run setup for each entry that has a backend or frontend.
			if len(rc.Workspace) > 0 {
				anyPatched := false
				for _, wEntry := range rc.Workspace {
					beTarget := wEntry.Backend
					if beTarget == "" && wEntry.BackendCfg != nil {
						beTarget = wEntry.BackendCfg.Target
					}
					feTarget := wEntry.Frontend
					if feTarget == "" && wEntry.FrontendCfg != nil {
						feTarget = wEntry.FrontendCfg.Target
					}
					if beTarget == "" && feTarget == "" {
						continue
					}
					outDir := wEntry.Out
					if outDir == "" && wEntry.BackendCfg != nil && wEntry.BackendCfg.Out != "" {
						outDir = wEntry.BackendCfg.Out
					}
					if outDir == "" {
						outDir = filepath.Join(rc.ConfigDir, "generated", wEntry.Name)
					} else if !filepath.IsAbs(outDir) {
						outDir = filepath.Clean(filepath.Join(rc.ConfigDir, outDir))
					}
					// Explicit backendConfig.dir / frontendConfig.dir take priority.
					// When absent, setup.Run auto-detects the service root by walking
					// up from outDir looking for pom.xml / package.json / go.mod / etc.
					beDir := ""
					if wEntry.BackendCfg != nil && wEntry.BackendCfg.Dir != "" {
						beDir = filepath.Clean(filepath.Join(rc.ConfigDir, wEntry.BackendCfg.Dir))
					}
					feOutDir := outDir
					if wEntry.FrontendCfg != nil && wEntry.FrontendCfg.Out != "" {
						feOutDir = filepath.Clean(filepath.Join(rc.ConfigDir, wEntry.FrontendCfg.Out))
					}
					feDir := ""
					if wEntry.FrontendCfg != nil && wEntry.FrontendCfg.Dir != "" {
						feDir = filepath.Clean(filepath.Join(rc.ConfigDir, wEntry.FrontendCfg.Dir))
					}

					entryResults := setup.Run(projectDir, beTarget, feTarget, outDir, setup.Options{
						BackendDir:     beDir,
						FrontendDir:    feDir,
						BackendOutDir:  outDir,
						FrontendOutDir: feOutDir,
					})
					if len(entryResults) > 0 {
						fmt.Printf("  %s %s\n", bold("→"), wEntry.Name)
						printSetupResults(entryResults)
						anyPatched = true
					}
				}
				if !anyPatched {
					fmt.Println(dim("  No config files to patch for this workspace"))
				}
				return nil
			}

			results := setup.Run(projectDir, rc.Backend, rc.Frontend, rc.Out, setup.Options{
				BackendDir:     opts.BackendDir,
				FrontendDir:    opts.FrontendDir,
				BackendOutDir:  rc.BackendOut,
				FrontendOutDir: rc.FrontendOut,
			})
			if len(results) == 0 {
				fmt.Println(dim("  No config files to patch for this stack"))
				return nil
			}
			printSetupResults(results)
			return nil
		},
	}
	cmd.Flags().StringVar(&backendDirFlag, "backend-dir", "",
		"directory containing backend project files (default: current directory)")
	cmd.Flags().StringVar(&frontendDirFlag, "frontend-dir", "",
		"directory containing frontend project files (default: current directory)")
	cmd.Flags().StringVar(&backendFlag, "backend", "", "backend target override")
	cmd.Flags().StringVar(&frontendFlag, "frontend", "", "frontend SDK override")
	cmd.Flags().StringVar(&inputFlag, "input", "", "input .veld file")
	cmd.Flags().StringVar(&outFlag, "out", "", "output directory override")
	return cmd
}

// ── main ──────────────────────────────────────────────────────────────────────

func newLSPCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "lsp",
		Short: "Start the Veld LSP server (stdin/stdout)",
		RunE: func(cmd *cobra.Command, args []string) error {
			return runLSPServer()
		},
	}
}

func runLSPServer() error {
	server := lsp.NewServer()
	return server.Run()
}

// ── fmt ───────────────────────────────────────────────────────────────────────

func newDoctorCmd() *cobra.Command {
	return &cobra.Command{
		Use:    "doctor",
		Short:  "Diagnose project health and check for common issues",
		Long:   "Runs a series of checks on your Veld project to identify\ncommon misconfigurations, missing files, and environment issues.",
		Hidden: true,
		RunE: func(cmd *cobra.Command, args []string) error {
			fmt.Println()
			fmt.Println(bold("  Veld Doctor"))
			fmt.Println()
			passed, failed := 0, 0

			check := func(name string, fn func() error) {
				if err := fn(); err != nil {
					fmt.Printf("  %s  %s — %s\n", red("✗"), name, err)
					failed++
				} else {
					fmt.Printf("  %s  %s\n", green("✓"), name)
					passed++
				}
			}

			// 1. Config file
			check("Config file found", func() error {
				_, _, err := config.FindConfig()
				if err != nil {
					return err
				}
				return nil
			})

			// 2. Config valid
			check("Config valid", func() error {
				_, err := config.BuildResolved(config.FlagOverrides{})
				return err
			})

			// 3. Input file parses
			check("Contract parses", func() error {
				rc, err := config.BuildResolved(config.FlagOverrides{})
				if err != nil {
					return err
				}
				_, _, err = loader.Parse(rc.Input, rc.Aliases)
				return err
			})

			// 4. Validation passes
			check("Contract validates", func() error {
				rc, err := config.BuildResolved(config.FlagOverrides{})
				if err != nil {
					return err
				}
				a, _, err := loader.Parse(rc.Input, rc.Aliases)
				if err != nil {
					return err
				}
				if errs := validator.Validate(a); len(errs) > 0 {
					return fmt.Errorf("%d validation error(s)", len(errs))
				}
				return nil
			})

			// 5. Backend emitter available
			check("Backend emitter registered", func() error {
				rc, err := config.BuildResolved(config.FlagOverrides{})
				if err != nil {
					return err
				}
				_, _, err = emitter.GetBackendOrTool(rc.Backend)
				return err
			})

			// 6. Frontend emitter available
			check("Frontend emitter registered", func() error {
				rc, err := config.BuildResolved(config.FlagOverrides{})
				if err != nil {
					return err
				}
				_, err = emitter.GetFrontend(rc.Frontend)
				return err
			})

			// 7. Output directory writable
			check("Output directory writable", func() error {
				rc, err := config.BuildResolved(config.FlagOverrides{})
				if err != nil {
					return err
				}
				for _, dir := range rc.OutputDirs() {
					if err := os.MkdirAll(dir, 0755); err != nil {
						return fmt.Errorf("cannot create %s: %w", dir, err)
					}
				}
				return nil
			})

			// 8. Lint check
			check("Lint clean (no errors)", func() error {
				rc, err := config.BuildResolved(config.FlagOverrides{})
				if err != nil {
					return err
				}
				a, _, err := loader.Parse(rc.Input, rc.Aliases)
				if err != nil {
					return err
				}
				issues := lint.Lint(a)
				errCount := 0
				for _, iss := range issues {
					if iss.IsError() {
						errCount++
					}
				}
				if errCount > 0 {
					return fmt.Errorf("%d lint error(s)", errCount)
				}
				return nil
			})

			fmt.Println()
			if failed > 0 {
				fmt.Printf("  %s %d passed, %s %d failed\n", green("✓"), passed, red("✗"), failed)
				return fmt.Errorf("%d check(s) failed", failed)
			}
			fmt.Printf("  %s All %d checks passed\n", green("✓"), passed)
			return nil
		},
	}
}

// ── completion ────────────────────────────────────────────────────────────────

func newCompletionCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "completion [bash|zsh|fish|powershell]",
		Short: "Generate shell completion scripts",
		Long: "Generate shell completion scripts for Veld.\n\n" +
			"  bash:       source <(veld completion bash)\n" +
			"  zsh:        veld completion zsh > \"${fpath[1]}/_veld\"\n" +
			"  fish:       veld completion fish | source\n" +
			"  powershell: veld completion powershell | Out-String | Invoke-Expression",
		Args:      cobra.ExactValidArgs(1),
		ValidArgs: []string{"bash", "zsh", "fish", "powershell"},
		RunE: func(cmd *cobra.Command, args []string) error {
			switch args[0] {
			case "bash":
				return cmd.Root().GenBashCompletion(os.Stdout)
			case "zsh":
				return cmd.Root().GenZshCompletion(os.Stdout)
			case "fish":
				return cmd.Root().GenFishCompletion(os.Stdout, true)
			case "powershell":
				return cmd.Root().GenPowerShellCompletionWithDesc(os.Stdout)
			default:
				return fmt.Errorf("unsupported shell: %s", args[0])
			}
		},
	}
	return cmd
}

// ── init ──────────────────────────────────────────────────────────────────────
