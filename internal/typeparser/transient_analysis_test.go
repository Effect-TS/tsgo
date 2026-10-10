package typeparser

import (
	"context"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

func TestTransientAnalysisCacheLifetime(t *testing.T) {
	t.Parallel()
	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/api.ts": `/** @stability experimental */
export declare function make<T>(value: T): T`,
		"/.src/first.ts": `import { make } from "./api.js"
export const first = make({ value: 1 })`,
		"/.src/second.ts": `import { make } from "./api.js"
export const second = make({ value: 2 })`,
	})
	defer done()
	first, second := files["/.src/first.ts"], files["/.src/second.ts"]
	for _, sf := range []*ast.SourceFile{first, second} {
		if diagnostics := c.GetDiagnostics(context.Background(), sf); len(diagnostics) != 0 {
			t.Fatalf("unexpected compiler diagnostics: %v", diagnostics)
		}
	}
	firstCall := findVariableInitializerCallByName(t, first, "first").AsNode()
	secondCall := findVariableInitializerCallByName(t, second, "second").AsNode()
	warm := func(parser *TypeParser, sf *ast.SourceFile, call *ast.Node) {
		parser.GetTypeAtLocation(call)
		parser.ReferenceSymbolAtNode(call.Expression())
		parser.ExpectedAndRealTypes(sf)
		parser.PipingFlows(sf, false)
		parser.GetEffectContextFlags(sf.AsNode())
		parser.ApiStabilityUsages(sf)
	}
	check := func(sf *ast.SourceFile, call *ast.Node, want bool) {
		t.Helper()
		for name, got := range map[string]bool{
			"type":      tp.links.TypeAtLocation.Has(call),
			"reference": tp.links.ReferenceSymbol.Has(call.Expression()),
			"expected":  tp.links.ExpectedAndRealTypes.Has(sf),
			"piping":    tp.links.PipingFlowsWithoutEffectFn.Has(sf),
			"context":   tp.links.EffectContextAnalyzed.Has(sf),
			"stability": tp.links.ApiStabilityUsages.Has(sf),
		} {
			if got != want {
				t.Errorf("%s %s cache present=%t, want %t", sf.FileName(), name, got, want)
			}
		}
	}
	finishOuter := tp.BeginTransientAnalysis()
	warm(tp, first, firstCall)
	firstType := c.TypeToString(tp.GetTypeAtLocation(firstCall))
	symbol := tp.ReferenceSymbolAtNode(firstCall.Expression())
	declared := tp.DeclaredApiStabilityOfSymbol(symbol)
	if declared.Level != ApiStabilityExperimental {
		t.Fatalf("unexpected declared stability: %v", declared)
	}
	other := NewTypeParser(c.Program(), c)
	finishInner := other.BeginTransientAnalysis()
	warm(other, second, secondCall)
	finishInner()
	check(first, firstCall, true)
	check(second, secondCall, true)
	finishOuter()
	check(first, firstCall, false)
	check(second, secondCall, false)
	if !tp.links.ApiStabilityDeclaredSymbol.Has(symbol) {
		t.Fatal("cross-file declared-stability metadata was discarded")
	}
	if got := tp.DeclaredApiStabilityOfSymbol(symbol); got != declared {
		t.Fatalf("declared stability changed: got %v, want %v", got, declared)
	}
	warm(other, first, firstCall)
	check(first, firstCall, true)
	if got := c.TypeToString(other.GetTypeAtLocation(firstCall)); got != firstType {
		t.Fatalf("recomputed type = %q, want %q", got, firstType)
	}
}
