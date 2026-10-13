package typeparser

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

func firstNodeOfKind(t *testing.T, sf *ast.SourceFile, kind ast.Kind) *ast.Node {
	t.Helper()
	var found *ast.Node
	var visit func(node *ast.Node) bool
	visit = func(node *ast.Node) bool {
		if found == nil && node.Kind == kind {
			found = node
		}
		node.ForEachChild(visit)
		return found != nil
	}
	sf.AsNode().ForEachChild(visit)
	if found == nil {
		t.Fatalf("no %v found", kind)
	}
	return found
}

func TestNodeCannotBeEffectLibraryTypeObjectLiteralsOnlyInV4(t *testing.T) {
	t.Parallel()
	source := `import { Effect } from "effect"
export const value = { a: 1 }
export const effect = Effect.void`
	for _, tt := range []struct {
		version  bundledeffect.EffectVersion
		expected bool
	}{
		{bundledeffect.EffectV4, true},
		{bundledeffect.EffectV3, false},
	} {
		if err := bundledeffect.EnsurePackageInstalled(tt.version, "effect"); err != nil {
			t.Skip("Effect not installed:", err)
		}
		_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectVersionInternal(t, tt.version, source)
		if got := tp.NodeCannotBeEffectLibraryType(firstNodeOfKind(t, sf, ast.KindObjectLiteralExpression)); got != tt.expected {
			t.Errorf("%v: NodeCannotBeEffectLibraryType(object literal) = %v, want %v", tt.version, got, tt.expected)
		}
		done()
	}
}

func TestNodeCouldBeEffectKeepsNarrowedReferences(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV4, "effect"); err != nil {
		t.Skip("Effect v4 not installed:", err)
	}
	source := `
import { Effect } from "effect"
interface Plain { readonly value: number }
declare const guarded: Plain
export const narrowed = Effect.isEffect(guarded) ? guarded : null
`
	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	narrowed := firstNodeOfKind(t, sf, ast.KindConditionalExpression).AsConditionalExpression().WhenTrue
	if tp.EffectType(tp.GetTypeAtLocation(narrowed)) == nil {
		t.Fatal("fixture: the guarded reference should narrow to an Effect")
	}
	if !tp.NodeCouldBeEffect(narrowed) {
		t.Error("a reference narrowed by a type guard must not be ruled out")
	}
}
