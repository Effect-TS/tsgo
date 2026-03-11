// Package rule defines the Rule struct for Effect diagnostic rules.
package rule

import (
	"testing"

	"github.com/microsoft/typescript-go/shim/ast"
)

func TestRuleStruct(t *testing.T) {
	// Test that a Rule can be created with all fields
	dummyRun := func(ctx *Context) []*ast.Diagnostic {
		return nil
	}

	r := Rule{
		Name:        "testRule",
		Description: "A test rule",
		Run:         dummyRun,
	}

	if r.Name != "testRule" {
		t.Errorf("expected Name to be 'testRule', got %q", r.Name)
	}
	if r.Description != "A test rule" {
		t.Errorf("expected Description to be 'A test rule', got %q", r.Description)
	}
	if r.Run == nil {
		t.Error("expected Run to be non-nil")
	}
}

func TestByName(t *testing.T) {
	dummyRun := func(ctx *Context) []*ast.Diagnostic {
		return nil
	}

	rules := []Rule{
		{Name: "rule1", Description: "First rule", Run: dummyRun},
		{Name: "rule2", Description: "Second rule", Run: dummyRun},
	}

	t.Run("finds existing rule", func(t *testing.T) {
		found := ByName(rules, "rule1")
		if found == nil {
			t.Fatal("expected to find rule1")
		}
		if found.Name != "rule1" {
			t.Errorf("expected Name to be 'rule1', got %q", found.Name)
		}
	})

	t.Run("returns nil for non-existent rule", func(t *testing.T) {
		found := ByName(rules, "nonexistent")
		if found != nil {
			t.Errorf("expected nil for non-existent rule, got %+v", found)
		}
	})

	t.Run("returns nil for empty slice", func(t *testing.T) {
		found := ByName([]Rule{}, "rule1")
		if found != nil {
			t.Errorf("expected nil for empty slice, got %+v", found)
		}
	})

	t.Run("returns pointer to actual rule in slice", func(t *testing.T) {
		found := ByName(rules, "rule1")
		if found != &rules[0] {
			t.Error("expected ByName to return pointer to actual rule in slice")
		}
	})

	t.Run("finds second rule correctly", func(t *testing.T) {
		found := ByName(rules, "rule2")
		if found == nil {
			t.Fatal("expected to find rule2")
		}
		if found.Name != "rule2" {
			t.Errorf("expected Name to be 'rule2', got %q", found.Name)
		}
		if found != &rules[1] {
			t.Error("expected ByName to return pointer to second rule in slice")
		}
	})
}
