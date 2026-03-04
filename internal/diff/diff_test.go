package diff

import (
	"testing"

	"github.com/Adhamzineldin/Veld/internal/ast"
)

func TestDiffRemovedModule(t *testing.T) {
	old := ast.AST{Modules: []ast.Module{{Name: "Users", Actions: []ast.Action{
		{Name: "Get", Method: "GET", Path: "/", Middleware: []string{}},
	}}}}
	new := ast.AST{}

	changes := Diff(old, new)
	if len(changes) != 1 {
		t.Fatalf("expected 1 change, got %d", len(changes))
	}
	if changes[0].Kind != Breaking {
		t.Error("removed module should be breaking")
	}
}

func TestDiffAddedModule(t *testing.T) {
	old := ast.AST{}
	new := ast.AST{Modules: []ast.Module{{Name: "Users", Actions: []ast.Action{
		{Name: "Get", Method: "GET", Path: "/", Middleware: []string{}},
	}}}}

	changes := Diff(old, new)
	if len(changes) != 1 {
		t.Fatalf("expected 1 change, got %d", len(changes))
	}
	if changes[0].Kind != Added {
		t.Error("added module should be non-breaking")
	}
}

func TestDiffNoChanges(t *testing.T) {
	a := ast.AST{Modules: []ast.Module{{Name: "Users", Actions: []ast.Action{
		{Name: "Get", Method: "GET", Path: "/", Output: "User", Middleware: []string{}},
	}}}}

	changes := Diff(a, a)
	if len(changes) != 0 {
		t.Errorf("expected 0 changes, got %d", len(changes))
	}
}

func TestDiffRemovedAction(t *testing.T) {
	old := ast.AST{Modules: []ast.Module{{Name: "Users", Actions: []ast.Action{
		{Name: "Get", Method: "GET", Path: "/", Middleware: []string{}},
		{Name: "Delete", Method: "DELETE", Path: "/:id", Middleware: []string{}},
	}}}}
	new := ast.AST{Modules: []ast.Module{{Name: "Users", Actions: []ast.Action{
		{Name: "Get", Method: "GET", Path: "/", Middleware: []string{}},
	}}}}

	changes := Diff(old, new)
	hasBreaking := false
	for _, c := range changes {
		if c.Kind == Breaking {
			hasBreaking = true
		}
	}
	if !hasBreaking {
		t.Error("removing an action should be breaking")
	}
}

func TestDiffAddedAction(t *testing.T) {
	old := ast.AST{Modules: []ast.Module{{Name: "Users", Actions: []ast.Action{
		{Name: "Get", Method: "GET", Path: "/", Middleware: []string{}},
	}}}}
	new := ast.AST{Modules: []ast.Module{{Name: "Users", Actions: []ast.Action{
		{Name: "Get", Method: "GET", Path: "/", Middleware: []string{}},
		{Name: "Create", Method: "POST", Path: "/", Input: "CreateUser", Middleware: []string{}},
	}}}}

	changes := Diff(old, new)
	hasAdded := false
	for _, c := range changes {
		if c.Kind == Added {
			hasAdded = true
		}
	}
	if !hasAdded {
		t.Error("adding an action should be detected as Added")
	}
}

func TestHasBreaking(t *testing.T) {
	if HasBreaking(nil) {
		t.Error("nil should not have breaking")
	}
	if HasBreaking([]Change{{Kind: Added}}) {
		t.Error("added-only should not have breaking")
	}
	if !HasBreaking([]Change{{Kind: Breaking}}) {
		t.Error("should detect breaking")
	}
}

func TestSaveLockAndLoadLock(t *testing.T) {
	dir := t.TempDir()
	a := ast.AST{
		Modules: []ast.Module{{Name: "Test", Actions: []ast.Action{
			{Name: "Get", Method: "GET", Path: "/", Middleware: []string{}},
		}}},
		Models: []ast.Model{{Name: "User", Fields: []ast.Field{
			{Name: "id", Type: "string"},
		}}},
	}

	if err := SaveLock(dir, a); err != nil {
		t.Fatalf("SaveLock: %v", err)
	}

	loaded, exists, err := LoadLock(dir)
	if err != nil {
		t.Fatalf("LoadLock: %v", err)
	}
	if !exists {
		t.Fatal("lock should exist")
	}
	if len(loaded.Modules) != 1 {
		t.Errorf("expected 1 module, got %d", len(loaded.Modules))
	}
	if len(loaded.Models) != 1 {
		t.Errorf("expected 1 model, got %d", len(loaded.Models))
	}
}

func TestLoadLockMissing(t *testing.T) {
	_, exists, err := LoadLock(t.TempDir())
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if exists {
		t.Error("should not exist")
	}
}

func TestDeleteLock(t *testing.T) {
	dir := t.TempDir()
	a := ast.AST{}
	SaveLock(dir, a)

	if err := DeleteLock(dir); err != nil {
		t.Fatalf("DeleteLock: %v", err)
	}

	_, exists, _ := LoadLock(dir)
	if exists {
		t.Error("lock should be deleted")
	}
}

func TestDeleteLockMissing(t *testing.T) {
	if err := DeleteLock(t.TempDir()); err != nil {
		t.Errorf("deleting nonexistent lock should not error: %v", err)
	}
}
