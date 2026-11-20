package main

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	// Register all emitters via init(). To add a new emitter, add one line here.
	// Register tool emitters (auxiliary generators — NOT backends).
)

func green(s string) string  { return colorGreen + s + colorReset }
func red(s string) string    { return colorRed + s + colorReset }
func yellow(s string) string { return colorYellow + s + colorReset }
func dim(s string) string    { return colorDim + s + colorReset }
func bold(s string) string   { return colorBold + s + colorReset }

// ── shared generation logic ───────────────────────────────────────────────────

// runPostGenerate executes the postGenerate hook if configured.
func promptContinue(question string) bool {
	fmt.Printf("%s  %s [y/N]: ", yellow("⚠"), question)
	reader := bufio.NewReader(os.Stdin)
	line, _ := reader.ReadString('\n')
	return strings.ToLower(strings.TrimSpace(line)) == "y"
}

// ── watch ─────────────────────────────────────────────────────────────────────

func dedup(ss []string) []string {
	seen := make(map[string]bool, len(ss))
	out := make([]string, 0, len(ss))
	for _, s := range ss {
		if !seen[s] {
			seen[s] = true
			out = append(out, s)
		}
	}
	return out
}

// ── clean ─────────────────────────────────────────────────────────────────────

func printValidationErrors(errs []error, veldFiles []string) {
	// Cache file contents for context printing
	fileCache := make(map[string][]string)
	for _, f := range veldFiles {
		data, err := os.ReadFile(f)
		if err == nil {
			fileCache[filepath.Base(f)] = strings.Split(string(data), "\n")
		}
	}

	for _, e := range errs {
		msg := e.Error()
		fmt.Fprintln(os.Stderr, red("  error: ")+msg)

		// Try to extract file:line from the message
		parts := strings.SplitN(msg, ":", 3)
		if len(parts) >= 3 {
			fileName := parts[0]
			lineStr := parts[1]
			var lineNum int
			if _, err := fmt.Sscanf(lineStr, "%d", &lineNum); err == nil && lineNum > 0 {
				if lines, ok := fileCache[fileName]; ok && lineNum <= len(lines) {
					line := lines[lineNum-1]
					fmt.Fprintf(os.Stderr, "  %s │\n", dim(fmt.Sprintf("%4d", lineNum)))
					fmt.Fprintf(os.Stderr, "  %s │ %s\n", dim(fmt.Sprintf("%4d", lineNum)), line)
					fmt.Fprintf(os.Stderr, "  %s │\n", dim("    "))
				}
			}
		}
	}
}

// ── writeGeneratedReadme ─────────────────────────────────────────────────────

func simpleDiff(oldLines, newLines []string, filename string) []string {
	var result []string
	result = append(result, dim(fmt.Sprintf("--- a/%s", filename)))
	result = append(result, dim(fmt.Sprintf("+++ b/%s", filename)))

	maxLen := len(oldLines)
	if len(newLines) > maxLen {
		maxLen = len(newLines)
	}

	for i := 0; i < maxLen; i++ {
		oldLine := ""
		newLine := ""
		if i < len(oldLines) {
			oldLine = oldLines[i]
		}
		if i < len(newLines) {
			newLine = newLines[i]
		}
		if oldLine != newLine {
			if i < len(oldLines) {
				result = append(result, red("-"+oldLine))
			}
			if i < len(newLines) {
				result = append(result, green("+"+newLine))
			}
		}
	}
	return result
}

// ── docs ──────────────────────────────────────────────────────────────────────

func parsePackageRef(ref string) (org, name, version string, err error) {
	// strip leading @
	s := strings.TrimPrefix(ref, "@")
	// split version
	parts := strings.SplitN(s, "@", 2)
	if len(parts) == 2 {
		version = parts[1]
	}
	// split org/name
	slash := strings.Index(parts[0], "/")
	if slash < 0 {
		err = fmt.Errorf("invalid package reference %q — expected @org/name[@version]", ref)
		return
	}
	org = parts[0][:slash]
	name = parts[0][slash+1:]
	if org == "" || name == "" {
		err = fmt.Errorf("invalid package reference %q — org and name must not be empty", ref)
	}
	return
}

func fmtBytes(n int64) string {
	switch {
	case n >= 1<<20:
		return fmt.Sprintf("%.1f MB", float64(n)/(1<<20))
	case n >= 1<<10:
		return fmt.Sprintf("%.1f kB", float64(n)/(1<<10))
	default:
		return fmt.Sprintf("%d B", n)
	}
}
