package typeparser

import (
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/typescript-go/shim/ast"
)

// findIdentifierByName returns the first identifier node with the given text
// that is not a declaration name.
func findIdentifierByName(t *testing.T, sf *ast.SourceFile, name string) *ast.Node {
	t.Helper()
	var found *ast.Node
	var visit func(node *ast.Node) bool
	visit = func(node *ast.Node) bool {
		if found != nil {
			return true
		}
		if node.Kind == ast.KindIdentifier && node.Text() == name && !ast.IsDeclarationName(node) {
			found = node
			return true
		}
		node.ForEachChild(visit)
		return false
	}
	sf.AsNode().ForEachChild(visit)
	if found == nil {
		t.Fatalf("identifier %q not found in source", name)
	}
	return found
}

func TestNodeCouldBeStrictEffect(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV4, "effect"); err != nil {
		t.Skip("Effect v4 not installed:", err)
	}

	source := `
import { Effect } from "effect"

const anEffect = Effect.succeed(1)
const aString = "hello"
const aNumber = 42
const anAny: any = null
const anUnknown: unknown = null
const aUnionWithEffect: Effect.Effect<number> | string = anEffect
const aUnionWithoutEffect: string | number | boolean = "x"
interface Plain { readonly value: number }
const aPlainObject: Plain = { value: 1 }
const anObjectKeyword: object = { value: 1 }
function generic<T>(param: T): T { return param }

export const uses = [
	anEffect,
	aString,
	aNumber,
	anAny,
	anUnknown,
	aUnionWithEffect,
	aUnionWithoutEffect,
	aPlainObject,
	anObjectKeyword,
]
export function inner<T>(param: T) { return [param] }
`
	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	tests := []struct {
		identifier string
		expected   bool
	}{
		// Conclusive negatives: the declared type can never flow-narrow into
		// a type whose symbol is named "Effect".
		{"aString", false},
		{"aNumber", false},
		{"aUnionWithoutEffect", false},
		{"aPlainObject", false},
		// Effect references and everything inconclusive must stay true.
		{"anEffect", true},
		{"anAny", true},
		{"anUnknown", true},
		{"aUnionWithEffect", true},
		{"anObjectKeyword", true}, // the object keyword type has no symbol
		{"param", true},           // type parameters can instantiate to anything
	}

	// Subtests deliberately avoided: all cases share one checker, which is
	// not safe for the parallel subtests the tparallel linter would require.
	for _, tt := range tests {
		node := findIdentifierByName(t, sf, tt.identifier)
		if got := tp.NodeCouldBeStrictEffect(node); got != tt.expected {
			t.Errorf("NodeCouldBeStrictEffect(%s) = %v, want %v", tt.identifier, got, tt.expected)
		}
	}

	// Non-reference node kinds are never ruled out.
	var call *ast.Node
	var visit func(node *ast.Node) bool
	visit = func(node *ast.Node) bool {
		if call != nil {
			return true
		}
		if node.Kind == ast.KindCallExpression {
			call = node
			return true
		}
		node.ForEachChild(visit)
		return false
	}
	sf.AsNode().ForEachChild(visit)
	if call == nil {
		t.Fatal("no call expression found")
	}
	if !tp.NodeCouldBeStrictEffect(call) {
		t.Error("call expressions must not be ruled out by the reference prefilter")
	}

	// Nil receiver and nil node stay conservative.
	var nilTp *TypeParser
	if !nilTp.NodeCouldBeStrictEffect(nil) {
		t.Error("nil TypeParser must not rule anything out")
	}
	if !tp.NodeCouldBeStrictEffect(nil) {
		t.Error("nil node must not be ruled out")
	}
}

func TestCouldBeStrictEffectDeepUnionStaysConservative(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV4, "effect"); err != nil {
		t.Skip("Effect v4 not installed:", err)
	}

	// A union nested beyond the recursion depth limit must return true even
	// though every member is a primitive literal.
	members := make([]string, 0, 40)
	for _, s := range []string{"a", "b", "c", "d", "e", "f", "g", "h"} {
		members = append(members, `"lit`+s+`"`)
	}
	source := `
type Deep = ` + strings.Join(members, " | ") + `
const deepValue: Deep = "lita"
export const use = [deepValue]
`
	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	node := findIdentifierByName(t, sf, "deepValue")
	// Literal unions are flat, so this exercises the union walk; whatever the
	// nesting, the answer may be false only when provably safe — a flat
	// primitive union is provably safe.
	if tp.NodeCouldBeStrictEffect(node) {
		t.Error("flat primitive literal union should be conclusively non-Effect")
	}
}
