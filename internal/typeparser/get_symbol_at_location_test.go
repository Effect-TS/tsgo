package typeparser

import (
	"context"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

func findDeleteExpression(t *testing.T, sf *ast.SourceFile) *ast.Node {
	t.Helper()
	var found *ast.Node
	var visit func(*ast.Node) bool
	visit = func(node *ast.Node) bool {
		if node.Kind == ast.KindDeleteExpression {
			found = node
		}
		node.ForEachChild(visit)
		return false
	}
	sf.AsNode().ForEachChild(visit)
	if found == nil {
		t.Fatal("delete expression not found")
	}
	return found
}

func TestGetSymbolAtLocationDeleteIndexDiagnostics(t *testing.T) {
	t.Parallel()
	for _, tc := range []struct {
		name       string
		expression string
		property   bool
	}{
		{"plain", "map.x", true},
		{"parenthesized", "(map.x)", true},
		{"multiply_parenthesized", "(((map.x)))", true},
		{"optional", "map?.x", true},
		{"optional_parenthesized", "(map?.x)", true},
		{"optional_multiply_parenthesized", "(((map?.x)))", true},
		{"element", `map["x"]`, false},
		{"element_parenthesized", `(map["x"])`, false},
		{"element_optional", `map?.["x"]`, false},
	} {
		queries := []string{"access"}
		if tc.property {
			queries = append(queries, "name")
		}
		for _, query := range queries {
			t.Run(tc.name+"/"+query, func(t *testing.T) {
				t.Parallel()
				c, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t,
					"declare const map: { [key: string]: number }; delete "+tc.expression+"; export {};")
				defer done()
				if diagnostics := c.GetDiagnostics(context.Background(), sf); len(diagnostics) != 0 {
					t.Fatalf("expected no initial diagnostics, got %v", diagnostics)
				}
				deleteExpression := findDeleteExpression(t, sf)
				location := ast.SkipParentheses(deleteExpression.Expression())
				if query == "name" {
					location = location.Name()
				}
				tp.GetSymbolAtLocation(location)
				if typ := tp.GetTypeAtLocation(deleteExpression); typ == nil || c.TypeToString(typ) != "boolean" {
					t.Fatalf("expected delete type boolean, got %v", typ)
				}
				if diagnostics := c.GetDiagnostics(context.Background(), sf); len(diagnostics) != 0 {
					for _, diagnostic := range diagnostics {
						t.Errorf("unexpected diagnostic after symbol and delete type lookup: TS%d %v", diagnostic.Code(), diagnostic)
					}
				}
			})
		}
	}
}

func TestGetSymbolAtLocationDeletePreservesOtherSymbols(t *testing.T) {
	t.Parallel()
	for _, tc := range []struct {
		name       string
		source     string
		identifier string
		occurrence int
		parent     bool
		literal    bool
		wantSymbol string
	}{
		{"receiver", `declare const map: { [key: string]: number }; delete map.x;`, "map", 1, false, false, "map"},
		{"parenthesized_receiver", `declare const map: { [key: string]: number }; delete (((map.x)));`, "map", 1, false, false, "map"},
		{"optional_receiver", `declare const map: { [key: string]: number }; delete map?.x;`, "map", 1, false, false, "map"},
		{"nested_receiver_name", `declare const root: { inner: { [key: string]: number } }; delete root.inner.x;`, "inner", 1, false, false, "inner"},
		{"nested_receiver_access", `declare const root: { inner: { [key: string]: number } }; delete root.inner.x;`, "inner", 1, true, false, "inner"},
		{"computed_key_name", `declare const map: { [key: string]: number }; declare const keys: { current: string }; delete map[keys.current];`, "current", 1, false, false, "current"},
		{"computed_key_access", `declare const map: { [key: string]: number }; declare const keys: { current: string }; delete map[keys.current];`, "current", 1, true, false, "current"},
		{"optional_computed_key_access", `declare const map: { [key: string]: number }; declare const keys: { current: string }; delete map?.[keys.current];`, "current", 1, true, false, "current"},
		{"literal_key", `declare const map: { x?: number }; delete map["x"];`, "", 0, false, true, "x"},
		{"optional_literal_key", `declare const map: { x?: number }; delete map?.["x"];`, "", 0, false, true, "x"},
		{"non_delete_name", `declare const map: { x: number; y?: number }; map.x; delete map.y;`, "x", 1, false, false, "x"},
		{"non_delete_access", `declare const map: { x: number; y?: number }; map.x; delete map.y;`, "x", 1, true, false, "x"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			c, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, tc.source+" export {};")
			defer done()
			if diagnostics := c.GetDiagnostics(context.Background(), sf); len(diagnostics) != 0 {
				t.Fatalf("expected no initial diagnostics, got %v", diagnostics)
			}
			deleteExpression := findDeleteExpression(t, sf)
			var location *ast.Node
			if tc.literal {
				location = deleteExpression.Expression().AsElementAccessExpression().ArgumentExpression
			} else {
				location = findIdentifierByText(t, sf, tc.identifier, tc.occurrence)
				if tc.parent {
					location = location.Parent
				}
			}
			if symbol := tp.GetSymbolAtLocation(location); symbol == nil || symbol.Name != tc.wantSymbol {
				t.Fatalf("expected symbol %q, got %v", tc.wantSymbol, symbol)
			}
			if typ := tp.GetTypeAtLocation(deleteExpression); typ == nil || c.TypeToString(typ) != "boolean" {
				t.Fatalf("expected delete type boolean, got %v", typ)
			}
			if diagnostics := c.GetDiagnostics(context.Background(), sf); len(diagnostics) != 0 {
				t.Fatalf("expected no diagnostics after symbol and delete type lookup, got %v", diagnostics)
			}
		})
	}
}

func TestGetSymbolAtLocationDeleteRequiredPropertyDiagnostics(t *testing.T) {
	t.Parallel()
	for _, query := range []string{"access", "name"} {
		t.Run(query, func(t *testing.T) {
			t.Parallel()
			c, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t,
				`declare const map: { x: number }; delete map.x; export {};`)
			defer done()
			if diagnostics := c.GetDiagnostics(context.Background(), sf); len(diagnostics) != 1 || diagnostics[0].Code() != 2790 {
				t.Fatalf("expected initial TS2790, got %v", diagnostics)
			}
			deleteExpression := findDeleteExpression(t, sf)
			location := deleteExpression.Expression()
			if query == "name" {
				location = location.Name()
			}
			tp.GetSymbolAtLocation(location)
			if typ := tp.GetTypeAtLocation(deleteExpression); typ == nil || c.TypeToString(typ) != "boolean" {
				t.Fatalf("expected delete type boolean, got %v", typ)
			}
			if diagnostics := c.GetDiagnostics(context.Background(), sf); len(diagnostics) != 1 || diagnostics[0].Code() != 2790 {
				t.Fatalf("expected TS2790 after symbol and delete type lookup, got %v", diagnostics)
			}
		})
	}
}
