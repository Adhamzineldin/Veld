package main

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"

	"github.com/Adhamzineldin/Veld/internal/language"
)

// GenerateLanguageFiles generates language definition files for plugins
func main() {
	spec := language.GetLanguageSpec()

	// Generate JSON file for tools/config
	generateJSON(spec)

	// Generate TypeScript file for VS Code plugin
	generateTypeScript(spec)

	// Generate Kotlin file for JetBrains plugin
	generateKotlin(spec)

	fmt.Println("✅ Language files generated successfully")
}

func generateJSON(spec *language.VeldLanguageSpec) {
	data, err := json.MarshalIndent(spec, "", "  ")
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		return
	}

	outputPath := filepath.Join(".", "veld-language.json")
	err = ioutil.WriteFile(outputPath, data, 0644)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error writing JSON: %v\n", err)
		return
	}

	fmt.Printf("✅ Generated %s\n", outputPath)
}

func generateTypeScript(spec *language.VeldLanguageSpec) {
	content := `/**
 * Veld Language Specification
 * AUTO-GENERATED - DO NOT EDIT
 * Generated from: internal/language/constants.go
 * Version: ` + spec.Version + `
 */

export const VELD_SPEC = {
  version: "` + spec.Version + `",
  keywords: [` + formatStringArray(spec.Keywords) + `],
  httpMethods: [` + formatStringArray(spec.HttpMethods) + `],
  builtinTypes: [` + formatStringArray(spec.BuiltinTypes) + `],
  directives: [` + formatStringArray(spec.Directives) + `],
  specialTypes: [` + formatStringArray(spec.SpecialTypes) + `],
  annotations: [` + formatStringArray(spec.Annotations) + `],
  configKeys: {
` + formatConfigKeysTS(spec.ConfigKeys) + `  },
};

export const KEYWORDS = new Set(VELD_SPEC.keywords);
export const HTTP_METHODS = new Set(VELD_SPEC.httpMethods);
export const BUILTIN_TYPES = new Set(VELD_SPEC.builtinTypes);
export const DIRECTIVES = new Set(VELD_SPEC.directives);
export const SPECIAL_TYPES = new Set(VELD_SPEC.specialTypes);
export const ANNOTATIONS = new Set(VELD_SPEC.annotations);
export const CONFIG_KEYS = VELD_SPEC.configKeys;
`

	outputPath := filepath.Join("editors", "vscode", "src", "veld-language-spec.ts")
	err := ioutil.WriteFile(outputPath, []byte(content), 0644)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error writing TypeScript: %v\n", err)
		return
	}

	fmt.Printf("✅ Generated %s\n", outputPath)
}

func generateKotlin(spec *language.VeldLanguageSpec) {
	content := `/**
 * Veld Language Specification
 * AUTO-GENERATED - DO NOT EDIT
 * Generated from: internal/language/constants.go
 * Version: ` + spec.Version + `
 */

package dev.veld.jetbrains

object VeldLanguageSpec {
    const val VERSION = "` + spec.Version + `"
    
    val KEYWORDS = setOf(` + formatStringArrayKotlin(spec.Keywords) + `)
    val HTTP_METHODS = setOf(` + formatStringArrayKotlin(spec.HttpMethods) + `)
    val BUILTIN_TYPES = setOf(` + formatStringArrayKotlin(spec.BuiltinTypes) + `)
    val DIRECTIVES = setOf(` + formatStringArrayKotlin(spec.Directives) + `)
    val SPECIAL_TYPES = setOf(` + formatStringArrayKotlin(spec.SpecialTypes) + `)
    val KNOWN_ANNOTATIONS = setOf(` + formatStringArrayKotlin(spec.Annotations) + `)

    val CONFIG_KEYS = mapOf(
` + formatConfigKeysKotlin(spec.ConfigKeys) + `    )

    fun isKeyword(word: String) = KEYWORDS.contains(word)
    fun isHttpMethod(word: String) = HTTP_METHODS.contains(word)
    fun isBuiltinType(word: String) = BUILTIN_TYPES.contains(word)
    fun isDirective(word: String) = DIRECTIVES.contains(word)
    fun isSpecialType(word: String) = SPECIAL_TYPES.contains(word)
}
`

	outputPath := filepath.Join("editors", "jetbrains", "src", "main", "kotlin", "dev", "veld", "jetbrains", "VeldLanguageSpec.kt")
	err := ioutil.WriteFile(outputPath, []byte(content), 0644)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error writing Kotlin: %v\n", err)
		return
	}

	fmt.Printf("✅ Generated %s\n", outputPath)
}

// formatConfigKeysTS renders config keys as TypeScript object entries.
func formatConfigKeysTS(keys []language.ConfigKey) string {
	var b strings.Builder
	for _, k := range keys {
		b.WriteString(fmt.Sprintf("    %-20s %s,\n", `"`+k.Key+`":`, `"`+escapeQuotes(k.Description)+`"`))
	}
	return b.String()
}

// formatConfigKeysKotlin renders config keys as Kotlin mapOf entries.
func formatConfigKeysKotlin(keys []language.ConfigKey) string {
	var b strings.Builder
	for i, k := range keys {
		comma := ","
		if i == len(keys)-1 {
			comma = ""
		}
		b.WriteString(fmt.Sprintf("        %-22s to %s%s\n",
			`"`+kotlinEscape(k.Key)+`"`, `"`+kotlinEscape(k.Description)+`"`, comma))
	}
	return b.String()
}

func escapeQuotes(s string) string {
	return strings.ReplaceAll(s, `"`, `\"`)
}

// kotlinEscape escapes a Go string for a Kotlin double-quoted literal.
// Kotlin treats `$` as string-template interpolation, so a literal `$`
// (e.g. the "$schema" config key) must be backslash-escaped or the
// generated plugin source will not compile.
func kotlinEscape(s string) string {
	s = strings.ReplaceAll(s, `\`, `\\`)
	s = strings.ReplaceAll(s, `"`, `\"`)
	return strings.ReplaceAll(s, `$`, `\$`)
}

func formatStringArray(arr []string) string {
	var quoted []string
	for _, s := range arr {
		quoted = append(quoted, `"`+s+`"`)
	}
	return strings.Join(quoted, ", ")
}

func formatStringArrayKotlin(arr []string) string {
	var quoted []string
	for _, s := range arr {
		quoted = append(quoted, `"`+kotlinEscape(s)+`"`)
	}
	return strings.Join(quoted, ", ")
}
