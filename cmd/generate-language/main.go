package main

import (
	"encoding/json"
	"fmt"
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"

	"github.com/veld-dev/veld/internal/language"
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
};

export const KEYWORDS = new Set(VELD_SPEC.keywords);
export const HTTP_METHODS = new Set(VELD_SPEC.httpMethods);
export const BUILTIN_TYPES = new Set(VELD_SPEC.builtinTypes);
export const DIRECTIVES = new Set(VELD_SPEC.directives);
export const SPECIAL_TYPES = new Set(VELD_SPEC.specialTypes);
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
		quoted = append(quoted, `"`+s+`"`)
	}
	return strings.Join(quoted, ", ")
}
