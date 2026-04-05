package validator

import (
	"testing"

	"github.com/Adhamzineldin/Veld/internal/config"
)

func TestValidateWorkspaceConsumes_SelfConsumption(t *testing.T) {
	entries := []config.WorkspaceEntry{
		{Name: "iam", Consumes: []string{"iam"}},
	}
	errs, _ := ValidateWorkspaceConsumes(entries)
	if len(errs) == 0 {
		t.Fatal("expected error for self-consumption")
	}
}

func TestValidateWorkspaceConsumes_UnknownService(t *testing.T) {
	entries := []config.WorkspaceEntry{
		{Name: "iam"},
		{Name: "transactions", Consumes: []string{"auth"}},
	}
	errs, _ := ValidateWorkspaceConsumes(entries)
	if len(errs) == 0 {
		t.Fatal("expected error for unknown service 'auth'")
	}
}

func TestValidateWorkspaceConsumes_CircularDependency(t *testing.T) {
	entries := []config.WorkspaceEntry{
		{Name: "a", Consumes: []string{"b"}},
		{Name: "b", Consumes: []string{"a"}},
	}
	errs, _ := ValidateWorkspaceConsumes(entries)
	if len(errs) == 0 {
		t.Fatal("expected error for circular dependency")
	}
}

func TestValidateWorkspaceConsumes_Valid(t *testing.T) {
	entries := []config.WorkspaceEntry{
		{Name: "iam", BaseUrl: "http://iam:3001"},
		{Name: "accounts", BaseUrl: "http://accounts:3002", Consumes: []string{"iam"}},
		{Name: "transactions", BaseUrl: "http://tx:3003", Consumes: []string{"iam", "accounts"}},
	}
	errs, warns := ValidateWorkspaceConsumes(entries)
	if len(errs) > 0 {
		t.Fatalf("unexpected errors: %v", errs)
	}
	if len(warns) > 0 {
		t.Fatalf("unexpected warnings: %v", warns)
	}
}

func TestValidateWorkspaceConsumes_NoBaseUrlWarning(t *testing.T) {
	entries := []config.WorkspaceEntry{
		{Name: "iam"}, // no baseUrl
		{Name: "transactions", BaseUrl: "http://tx:3003", Consumes: []string{"iam"}},
	}
	errs, warns := ValidateWorkspaceConsumes(entries)
	if len(errs) > 0 {
		t.Fatalf("unexpected errors: %v", errs)
	}
	if len(warns) == 0 {
		t.Fatal("expected warning for missing baseUrl on consumed service")
	}
}

func TestValidateWorkspaceConsumes_TransitiveCircle(t *testing.T) {
	entries := []config.WorkspaceEntry{
		{Name: "a", Consumes: []string{"b"}},
		{Name: "b", Consumes: []string{"c"}},
		{Name: "c", Consumes: []string{"a"}},
	}
	errs, _ := ValidateWorkspaceConsumes(entries)
	if len(errs) == 0 {
		t.Fatal("expected error for transitive circular dependency a→b→c→a")
	}
}
