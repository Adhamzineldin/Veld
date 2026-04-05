// Package sdkhelpers provides shared utilities for all service SDK emitters.
// These functions handle common concerns like base URL env-var naming,
// method signature building, and URL interpolation across languages.
package sdkhelpers

import (
	"strings"
	"unicode"
)

// EnvVarName returns the conventional environment variable name for a service's
// base URL: VELD_<UPPER_SNAKE>_URL. e.g. "iam" → "VELD_IAM_URL",
// "card-service" → "VELD_CARD_SERVICE_URL".
func EnvVarName(serviceName string) string {
	upper := strings.ToUpper(serviceName)
	upper = strings.ReplaceAll(upper, "-", "_")
	upper = strings.ReplaceAll(upper, " ", "_")
	return "VELD_" + upper + "_URL"
}

// ServiceClassName returns a PascalCase class/struct name for a service client.
// e.g. "iam" → "IAMClient", "card-service" → "CardServiceClient".
func ServiceClassName(serviceName string) string {
	return toPascalCase(serviceName) + "Client"
}

// ServiceFileName returns a snake_case file name stem for a service client.
// e.g. "iam" → "iam", "card-service" → "card_service".
func ServiceFileName(serviceName string) string {
	return strings.ReplaceAll(strings.ToLower(serviceName), "-", "_")
}

// toPascalCase converts a kebab-case or snake_case name to PascalCase.
// e.g. "card-service" → "CardService", "iam" → "Iam", "my_service" → "MyService".
func toPascalCase(s string) string {
	var b strings.Builder
	upper := true
	for _, r := range s {
		if r == '-' || r == '_' || r == ' ' {
			upper = true
			continue
		}
		if upper {
			b.WriteRune(unicode.ToUpper(r))
			upper = false
		} else {
			b.WriteRune(r)
		}
	}
	return b.String()
}
