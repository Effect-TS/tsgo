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

func TestNodeCannotBeEffectLibraryTypePrimitiveExpressions(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV4, "effect"); err != nil {
		t.Skip("Effect v4 not installed:", err)
	}
	source := `import { Effect } from "effect"
declare const x: number
export const trueLiteral = true
export const falseLiteral = false
export const nullLiteral = null
export const regex = /a/
export const not = !x
export const negate = -x
export const voided = void 0
export const typeOf = typeof x
export const undefinedReference = undefined
export const effect = Effect.void`
	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	initializers := map[string]*ast.Node{}
	for _, statement := range sf.Statements.Nodes {
		if statement.Kind != ast.KindVariableStatement {
			continue
		}
		for _, declaration := range statement.AsVariableStatement().DeclarationList.AsVariableDeclarationList().Declarations.Nodes {
			initializers[declaration.Name().Text()] = declaration.Initializer()
		}
	}
	for name, expected := range map[string]bool{
		"trueLiteral":        true,
		"falseLiteral":       true,
		"nullLiteral":        true,
		"regex":              true,
		"not":                true,
		"negate":             true,
		"voided":             true,
		"typeOf":             true,
		"undefinedReference": false,
		"effect":             false,
	} {
		node := initializers[name]
		if got := tp.NodeCannotBeEffectLibraryType(node); got != expected {
			t.Errorf("NodeCannotBeEffectLibraryType(%s) = %v, want %v", name, got, expected)
		}
		if isEffect := tp.EffectType(tp.GetTypeAtLocation(node)) != nil; expected && isEffect {
			t.Errorf("fixture: %s was ruled out but its type is an Effect", name)
		}
	}
}
