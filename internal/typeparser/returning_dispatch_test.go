package typeparser

import (
	"slices"
	"strings"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

func TestParseReturningDispatchAcceptedShapes(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name          string
		function      string
		tests         []string
		discriminants []string
		results       []string
		fallback      string
	}{
		{
			name:     "arrow conditional",
			function: `(value: unknown) => isA(value) ? first : fallback`,
			tests:    []string{"isA(value)"},
			results:  []string{"first"},
			fallback: "fallback",
		},
		{
			name: "returned conditional chain",
			function: `(value: unknown) => {
				return isA(value) ? first : isB(value) ? second : fallback
			}`,
			tests:    []string{"isA(value)", "isB(value)"},
			results:  []string{"first", "second"},
			fallback: "fallback",
		},
		{
			name: "function expression if with trailing fallback",
			function: `function(value: unknown) {
				if (isA(value)) return first
				return fallback
			}`,
			tests:    []string{"isA(value)"},
			results:  []string{"first"},
			fallback: "fallback",
		},
		{
			name: "switch exposes discriminant and case expressions",
			function: `(value: unknown) => {
				switch (classify(value)) {
					case "A": return first
					case "B": return second
					default: return fallback
				}
			}`,
			tests:         []string{`"A"`, `"B"`},
			discriminants: []string{"classify(value)", "classify(value)"},
			results:       []string{"first", "second"},
			fallback:      "fallback",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			sf, function := parseReturningDispatchTestFunction(t, tt.function)
			dispatch := ParseReturningDispatch(function)
			if dispatch == nil {
				t.Fatal("expected returning dispatch")
			}
			if dispatch.Node != function || dispatch.Body == nil || len(dispatch.Params) != 1 {
				t.Fatalf("unexpected function evidence: %+v", dispatch)
			}

			tests := make([]string, len(dispatch.Branches))
			discriminants := make([]string, len(dispatch.Branches))
			results := make([]string, len(dispatch.Branches))
			for index, branch := range dispatch.Branches {
				tests[index] = returningDispatchNodeText(sf, branch.Test)
				discriminants[index] = returningDispatchNodeText(sf, branch.Discriminant)
				results[index] = returningDispatchNodeText(sf, branch.Result)
				if branch.TestNode == nil {
					t.Fatalf("branch %d is missing its source test node", index)
				}
			}
			if !slices.Equal(tests, tt.tests) {
				t.Fatalf("tests = %v, want %v", tests, tt.tests)
			}
			if !slices.Equal(results, tt.results) {
				t.Fatalf("results = %v, want %v", results, tt.results)
			}
			if len(tt.discriminants) > 0 && !slices.Equal(discriminants, tt.discriminants) {
				t.Fatalf("discriminants = %v, want %v", discriminants, tt.discriminants)
			}
			if got := returningDispatchNodeText(sf, dispatch.Fallback); got != tt.fallback {
				t.Fatalf("fallback = %q, want %q", got, tt.fallback)
			}
		})
	}
}

func TestParseReturningDispatchInputContract(t *testing.T) {
	t.Parallel()
	sf := parseSource(`
		function declaration(value: unknown) {
			return isA(value) ? first : fallback
		}
		const bodyOnly = (value: unknown) => isA(value) ? first : fallback
		const plain = (value: unknown) => first
		const generic = <A>(value: A) => isA(value) ? first : fallback
	`)

	var declaration *ast.Node
	var body *ast.Node
	var plain *ast.Node
	var generic *ast.Node
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		switch GetFunctionLikeName(node) {
		case "declaration":
			declaration = node
		case "bodyOnly":
			body = GetFunctionLikeBody(node)
		case "plain":
			plain = node
		case "generic":
			generic = node
		}
		return node.ForEachChild(walk)
	}
	walk(sf.AsNode())

	for name, node := range map[string]*ast.Node{
		"function declaration": declaration,
		"body node":            body,
		"plain function":       plain,
		"generic function":     generic,
	} {
		if node == nil {
			t.Fatalf("%s fixture was not found", name)
		}
		if dispatch := ParseReturningDispatch(node); dispatch != nil {
			t.Fatalf("%s: expected parse rejection, got %+v", name, dispatch)
		}
	}
}

func parseReturningDispatchTestFunction(t *testing.T, functionSource string) (*ast.SourceFile, *ast.Node) {
	t.Helper()
	sf := parseSource("const decode = " + functionSource)
	var function *ast.Node
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindArrowFunction || node.Kind == ast.KindFunctionExpression {
			function = node
			return true
		}
		return node.ForEachChild(walk)
	}
	walk(sf.AsNode())
	if function == nil {
		t.Fatal("failed to parse returning function")
	}
	return sf, function
}

func returningDispatchNodeText(sf *ast.SourceFile, node *ast.Node) string {
	if sf == nil || node == nil {
		return ""
	}
	return strings.TrimSpace(scanner.GetSourceTextOfNodeFromSourceFile(sf, node, false))
}
