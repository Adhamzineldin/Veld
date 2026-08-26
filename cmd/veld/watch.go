package main

import (
	"context"
	"fmt"
	"os"
	"os/signal"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/Adhamzineldin/Veld/internal/config"
	"github.com/Adhamzineldin/Veld/internal/diff"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/spf13/cobra"
	// Register all emitters via init(). To add a new emitter, add one line here.
	// Register tool emitters (auxiliary generators — NOT backends).
)

func newWatchCmd() *cobra.Command {
	var backendFlag, frontendFlag, inputFlag, outFlag string

	cmd := &cobra.Command{
		Use:   "watch",
		Short: "Watch .veld files and auto-regenerate on change",
		Long: "Watches all .veld files and the config for changes, then performs a full\n" +
			"regeneration of all outputs. This ensures shared artifacts (types, barrels,\n" +
			"middleware, _internal.ts) are always consistent. Safe to run during development.",
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
				return err
			}

			fmt.Println(bold("veld watch") + "  •  watching for changes  •  Ctrl-C to stop")
			fmt.Println()

			opts := emitter.EmitOptions{
				BaseUrl:  rc.BaseUrl,
				Validate: rc.Validate,
			}

			// ── clean before initial generation ─────────────────────────
			runClean(rc)

			// ── initial full generation (never incremental) ─────────────
			var regenerated []string
			var initFiles []string
			var changes []diff.Change
			var genErr error
			if len(rc.Workspace) > 0 {
				regenerated, initFiles, changes, genErr = runWorkspaceGenerate(rc, flags, opts)
			} else {
				regenerated, initFiles, changes, genErr = runGenerate(rc, false, opts)
			}
			if genErr != nil {
				fmt.Fprintln(os.Stderr, red("error: ")+genErr.Error())
			} else if len(rc.Workspace) > 0 {
				fmt.Printf(green("✓")+" Ready (%d service(s))\n", len(regenerated))
			} else if rc.SplitOutput() {
				fmt.Printf(green("✓")+" Ready (%d module(s)) → backend: %s, frontend: %s\n",
					len(regenerated), rc.BackendOut, rc.FrontendOut)
			} else {
				fmt.Printf(green("✓")+" Ready (%d module(s)) → %s\n", len(regenerated), rc.Out)
			}
			printDiffChanges(changes)
			runPostGenerate(rc)
			fmt.Println()

			// ── build the watched file set ──────────────────────────────
			// Includes all .veld files + the config file itself.
			// In workspace mode, runGenerate returns no files; collect them here.
			if len(rc.Workspace) > 0 && len(initFiles) == 0 {
				for _, wEntry := range rc.Workspace {
					if wEntry.Input == "" {
						continue
					}
					entryInput := wEntry.Input
					if !filepath.IsAbs(entryInput) {
						entryInput = filepath.Join(rc.ConfigDir, entryInput)
					}
					_ = filepath.Walk(filepath.Dir(entryInput), func(p string, info os.FileInfo, _ error) error {
						if info != nil && !info.IsDir() && strings.HasSuffix(p, ".veld") {
							absP, _ := filepath.Abs(p)
							initFiles = append(initFiles, absP)
						}
						return nil
					})
				}
				// Also include shared files in the config root.
				_ = filepath.Walk(rc.ConfigDir, func(p string, info os.FileInfo, _ error) error {
					if info != nil && !info.IsDir() && strings.HasSuffix(p, ".veld") {
						absP, _ := filepath.Abs(p)
						initFiles = append(initFiles, absP)
					}
					return nil
				})
			}
			var mtimesMu sync.Mutex
			mtimes := make(map[string]int64, len(initFiles)+2)
			for _, f := range initFiles {
				if info, statErr := os.Stat(f); statErr == nil {
					mtimes[f] = info.ModTime().UnixNano()
				}
			}
			// Also watch the config file(s).
			configCandidates := []string{
				filepath.Join(rc.ConfigDir, "veld.config.json"),
				filepath.Join(rc.ConfigDir, "veld", "veld.config.json"),
			}
			for _, cf := range configCandidates {
				if info, statErr := os.Stat(cf); statErr == nil {
					mtimes[cf] = info.ModTime().UnixNano()
				}
			}

			ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt)
			defer stop()

			ticker := time.NewTicker(200 * time.Millisecond)
			defer ticker.Stop()

			var debounceTimer *time.Timer
			var generating atomic.Bool // prevents re-triggering while generation is in progress

			for {
				select {
				case <-ctx.Done():
					fmt.Println("\nWatch stopped.")
					return nil

				case <-ticker.C:
					// Skip change detection while a generation is in flight.
					if generating.Load() {
						continue
					}

					var changedNames []string
					configChanged := false

					mtimesMu.Lock()

					// Check existing tracked files for modifications.
					var deletedFiles []string
					for f, last := range mtimes {
						info, statErr := os.Stat(f)
						if statErr != nil {
							// File was deleted — count as change and mark for removal.
							changedNames = append(changedNames, filepath.Base(f))
							deletedFiles = append(deletedFiles, f)
							continue
						}
						if info.ModTime().UnixNano() != last {
							changedNames = append(changedNames, filepath.Base(f))
							if strings.HasSuffix(f, "veld.config.json") {
								configChanged = true
							}
						}
					}
					// Purge deleted files so they don't re-trigger on every tick.
					for _, f := range deletedFiles {
						delete(mtimes, f)
					}

					// Discover NEW .veld files that didn't exist at startup.
					// Re-scan input directories for any new .veld files.
					// In workspace mode scan each service's input directory.
					var scanDirs []string
					if len(rc.Workspace) > 0 {
						for _, wEntry := range rc.Workspace {
							if wEntry.Input != "" {
								entryInput := wEntry.Input
								if !filepath.IsAbs(entryInput) {
									entryInput = filepath.Join(rc.ConfigDir, entryInput)
								}
								scanDirs = append(scanDirs, filepath.Dir(entryInput))
							}
						}
						// Also scan the veld source root for shared models/enums.
						scanDirs = append(scanDirs, rc.ConfigDir)
					} else if rc.Input != "" {
						scanDirs = append(scanDirs, filepath.Dir(rc.Input))
					}
					for _, scanDir := range scanDirs {
						_ = filepath.Walk(scanDir, func(path string, info os.FileInfo, walkErr error) error {
							if walkErr != nil || info.IsDir() {
								return nil
							}
							if strings.HasSuffix(path, ".veld") {
								absPath, _ := filepath.Abs(path)
								if _, tracked := mtimes[absPath]; !tracked {
									// New file found — treat as a change.
									mtimes[absPath] = info.ModTime().UnixNano()
									changedNames = append(changedNames, filepath.Base(absPath))
								}
							}
							return nil
						})
					}

					if len(changedNames) > 0 {
						// Update all mtimes immediately to avoid re-triggering.
						for f := range mtimes {
							if info, statErr := os.Stat(f); statErr == nil {
								mtimes[f] = info.ModTime().UnixNano()
							}
						}
					}
					mtimesMu.Unlock()

					if len(changedNames) == 0 {
						continue
					}

					// Debounce: reset timer on every change, fire after 300ms of quiet.
					if debounceTimer != nil {
						debounceTimer.Stop()
					}

					// Capture for the closure.
					capturedChanged := changedNames
					capturedConfigChanged := configChanged

					debounceTimer = time.AfterFunc(300*time.Millisecond, func() {
						generating.Store(true)
						defer generating.Store(false)

						ts := dim("[" + time.Now().Format("15:04:05") + "]")

						// ── reload config if it changed ─────────────────
						currentRC := rc
						if capturedConfigChanged {
							fmt.Printf("%s %s config changed, reloading...\n", ts, yellow("⟳"))
							newRC, reloadErr := config.BuildResolved(flags)
							if reloadErr != nil {
								fmt.Fprintf(os.Stderr, "%s %s failed to reload config: %v\n", ts, red("✗"), reloadErr)
								return
							}
							rc = newRC
							currentRC = newRC
							// Update emit options from new config.
							opts = emitter.EmitOptions{
								BaseUrl:  currentRC.BaseUrl,
								Validate: currentRC.Validate,
							}
						}

						// ── always full regeneration ────────────────────
						// Watch mode NEVER does incremental generation.
						// Any .veld change can affect shared types, barrels,
						// middleware interfaces, _internal.ts, error _base.ts,
						// cross-module type imports, and app-level prefix.
						// A full regen takes <100ms for typical projects.
						fmt.Printf("%s %s change in %s — regenerating all...\n",
							ts, yellow("⟳"), strings.Join(dedup(capturedChanged), ", "))

						// Clean before regeneration.
						runClean(currentRC)

						start := time.Now()
						var regen []string
						var newFiles []string
						var changes []diff.Change
						var genErr error
						if len(currentRC.Workspace) > 0 {
							regen, newFiles, changes, genErr = runWorkspaceGenerate(currentRC, flags, opts)
						} else {
							regen, newFiles, changes, genErr = runGenerate(currentRC, false, opts)
						}

						if genErr != nil {
							// Always show the error summary — the user may have
							// changed the file and hit a different error.
							// Validation details are already printed by runGenerate.
							fmt.Fprintf(os.Stderr, "\n%s\n", red("  ── error "+strings.Repeat("─", 44)))
							fmt.Fprintf(os.Stderr, "%s %s %v\n", ts, red("✗"), genErr)
							fmt.Fprintf(os.Stderr, "%s\n\n", red("  ── save to retry "+strings.Repeat("─", 38)))
						} else {
							elapsed := time.Since(start).Round(time.Millisecond)
							if regen == nil || len(regen) == 0 {
								fmt.Printf("%s %s nothing to regenerate (%s)\n", ts, green("✓"), elapsed)
							} else {
								fmt.Printf("%s %s regenerated %s (%s)\n", ts, green("✓"), strings.Join(regen, ", "), elapsed)
								printDiffChanges(changes)
							}
							runPostGenerate(currentRC)
							fmt.Println()
						}

						// Refresh tracked file set — ALWAYS, even on error.
						// If we only refresh on success, failed builds leave stale
						// mtimes that re-trigger on every tick (infinite loop).
						mtimesMu.Lock()
						if newFiles != nil && len(newFiles) > 0 {
							// Rebuild mtimes from scratch with new file list + config.
							fresh := make(map[string]int64, len(newFiles)+2)
							for _, f := range newFiles {
								if info, statErr := os.Stat(f); statErr == nil {
									fresh[f] = info.ModTime().UnixNano()
								}
							}
							for _, cf := range configCandidates {
								if info, statErr := os.Stat(cf); statErr == nil {
									fresh[cf] = info.ModTime().UnixNano()
								}
							}
							mtimes = fresh
						} else {
							// Generation failed or returned no files — refresh
							// all existing entries so the tick doesn't re-trigger.
							for f := range mtimes {
								if info, statErr := os.Stat(f); statErr == nil {
									mtimes[f] = info.ModTime().UnixNano()
								} else {
									delete(mtimes, f)
								}
							}
							for _, cf := range configCandidates {
								if info, statErr := os.Stat(cf); statErr == nil {
									mtimes[cf] = info.ModTime().UnixNano()
								}
							}
						}
						mtimesMu.Unlock()
					})
				}
			}
		},
	}
	cmd.Flags().StringVar(&backendFlag, "backend", "", "backend target ("+strings.Join(emitter.ListBackends(), ", ")+")")
	cmd.Flags().StringVar(&frontendFlag, "frontend", "", "frontend SDK ("+strings.Join(emitter.ListFrontends(), ", ")+", none)")
	cmd.Flags().StringVar(&inputFlag, "input", "", "input .veld file")
	cmd.Flags().StringVar(&outFlag, "out", "", "output directory")
	return cmd
}

// dedup returns a slice with duplicate strings removed, preserving order.
