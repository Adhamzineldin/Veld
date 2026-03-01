package database_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
	"github.com/Adhamzineldin/Veld/internal/emitter"
	"github.com/Adhamzineldin/Veld/internal/emitter/database"
)

func minimalAST() ast.AST {
	return ast.AST{
		Models: []ast.Model{
			{Name: "User", Fields: []ast.Field{
				{Name: "id", Type: "uuid"},
				{Name: "email", Type: "string"},
				{Name: "name", Type: "string"},
				{Name: "age", Type: "int", Optional: true},
			}},
		},
		Enums: []ast.Enum{
			{Name: "Role", Values: []string{"admin", "user"}},
		},
		Modules: []ast.Module{},
	}
}

func TestDatabaseEmitCreatesAllFiles(t *testing.T) {
	e := database.New()
	outDir := t.TempDir()

	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	expected := []string{
		filepath.Join(outDir, "database", "schema.prisma"),
		filepath.Join(outDir, "database", "schema.sql"),
		filepath.Join(outDir, "database", "typeorm", "entities.ts"),
		filepath.Join(outDir, "database", "gorm", "models.go"),
		filepath.Join(outDir, "database", "sqlalchemy", "models.py"),
	}
	for _, f := range expected {
		if _, err := os.Stat(f); os.IsNotExist(err) {
			t.Errorf("expected %s to exist", f)
		}
	}
}

func TestDatabaseTypeORMContent(t *testing.T) {
	e := database.New()
	outDir := t.TempDir()
	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "database", "typeorm", "entities.ts"))
	content := string(data)
	for _, needle := range []string{"@Entity", "@Column", "PrimaryGeneratedColumn", "UserEntity"} {
		if !strings.Contains(content, needle) {
			t.Errorf("entities.ts missing %q", needle)
		}
	}
}

func TestDatabaseGORMContent(t *testing.T) {
	e := database.New()
	outDir := t.TempDir()
	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "database", "gorm", "models.go"))
	content := string(data)
	for _, needle := range []string{"package models", "gorm.io/gorm", "type User struct", "gorm:"} {
		if !strings.Contains(content, needle) {
			t.Errorf("models.go missing %q", needle)
		}
	}
}

func TestDatabaseSQLAlchemyContent(t *testing.T) {
	e := database.New()
	outDir := t.TempDir()
	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{}); err != nil {
		t.Fatalf("Emit: %v", err)
	}

	data, _ := os.ReadFile(filepath.Join(outDir, "database", "sqlalchemy", "models.py"))
	content := string(data)
	for _, needle := range []string{"class User(Base)", "__tablename__", "Column", "DeclarativeBase"} {
		if !strings.Contains(content, needle) {
			t.Errorf("models.py missing %q", needle)
		}
	}
}

func TestDatabaseDryRun(t *testing.T) {
	e := database.New()
	outDir := t.TempDir()
	if err := e.Emit(minimalAST(), outDir, emitter.EmitOptions{DryRun: true}); err != nil {
		t.Fatalf("Emit: %v", err)
	}
	entries, _ := os.ReadDir(outDir)
	if len(entries) != 0 {
		t.Error("dry-run should write no files")
	}
}
