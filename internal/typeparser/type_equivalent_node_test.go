package typeparser

import (
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

func TestTypeEquivalentNodeMatchesCheckerTypes(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV4, "effect"); err != nil {
		t.Skip("Effect v4 not installed:", err)
	}
	source := `
import { Effect } from "effect"
declare const service: { readonly run: Effect.Effect<number, string> }
export const viaAccess = service.run
export const wrapped = (Effect.succeed(1))
export const checked = Effect.fail("x") satisfies Effect.Effect<never, string>
export function f(param: Effect.Effect<number>) { return param }
export const literal = { inner: Effect.void, viaAccess }
export class C { field = Effect.never }
`
	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	found := map[ast.Kind]int{}
	var visit func(node *ast.Node) bool
	visit = func(node *ast.Node) bool {
		if target, ok := TypeEquivalentNode(node); ok {
			found[target.Kind]++
			if got, want := tp.GetTypeAtLocation(node), tp.GetTypeAtLocation(target); got != want {
				t.Errorf("%v at %d: type %v differs from its equivalent %v: %v",
					node.Kind, node.Pos(), tp.checker.TypeToString(got), target.Kind, tp.checker.TypeToString(want))
			}
		}
		node.ForEachChild(visit)
		return false
	}
	sf.AsNode().ForEachChild(visit)

	for kind, want := range map[ast.Kind]int{
		ast.KindPropertyAccessExpression:    5,
		ast.KindVariableDeclaration:         5,
		ast.KindParameter:                   1,
		ast.KindPropertyAssignment:          1,
		ast.KindShorthandPropertyAssignment: 1,
		ast.KindPropertyDeclaration:         1,
		ast.KindCallExpression:              2,
	} {
		if found[kind] != want {
			t.Errorf("equivalents of kind %v = %d, want %d", kind, found[kind], want)
		}
	}
}
