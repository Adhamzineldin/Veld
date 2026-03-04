package schema

import (
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

func TestBuildPrisma(t *testing.T) {
	a := ast.AST{
		Models: []ast.Model{{
			Name: "User",
			Fields: []ast.Field{
				{Name: "id", Type: "uuid"},
				{Name: "email", Type: "string"},
				{Name: "age", Type: "int", Optional: true},
				{Name: "active", Type: "bool", Default: "true"},
			},
		}},
	}
	out := BuildPrisma(a)
	if !strings.Contains(out, "model User") {
		t.Error("should contain model User")
	}
	if !strings.Contains(out, "@id @default(uuid())") {
		t.Error("uuid id should get @id @default(uuid())")
	}
	if !strings.Contains(out, "Int?") {
		t.Error("optional int should be Int?")
	}
	if !strings.Contains(out, "@default(true)") {
		t.Error("bool default should be @default(true)")
	}
	if !strings.Contains(out, "datasource db") {
		t.Error("should contain datasource")
	}
}

func TestBuildPrismaInheritance(t *testing.T) {
	a := ast.AST{
		Models: []ast.Model{
			{Name: "Base", Fields: []ast.Field{{Name: "id", Type: "uuid"}}},
			{Name: "Child", Extends: "Base", Fields: []ast.Field{{Name: "name", Type: "string"}}},
		},
	}
	out := BuildPrisma(a)
	// Child should include inherited id field
	childIdx := strings.Index(out, "model Child")
	if childIdx < 0 {
		t.Fatal("should contain model Child")
	}
	childBlock := out[childIdx:]
	if !strings.Contains(childBlock, "id") {
		t.Error("Child should inherit id from Base")
	}
}

func TestBuildSQL(t *testing.T) {
	a := ast.AST{
		Models: []ast.Model{{
			Name: "User",
			Fields: []ast.Field{
				{Name: "id", Type: "uuid"},
				{Name: "email", Type: "string"},
				{Name: "count", Type: "int"},
				{Name: "score", Type: "float"},
				{Name: "active", Type: "bool"},
				{Name: "created", Type: "datetime"},
				{Name: "bio", Type: "string", Optional: true},
			},
		}},
	}
	out := BuildSQL(a)
	if !strings.Contains(out, "CREATE TABLE") {
		t.Error("should contain CREATE TABLE")
	}
	if !strings.Contains(out, "PRIMARY KEY") {
		t.Error("uuid id should be PRIMARY KEY")
	}
	if !strings.Contains(out, "NOT NULL") {
		t.Error("required fields should be NOT NULL")
	}
}
