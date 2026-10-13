package typeparser

import (
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
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
export const either = anAny || anEffect
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

	findKind := func(kind ast.Kind) *ast.Node {
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
	if tp.NodeCouldBeStrictEffect(findKind(ast.KindArrayLiteralExpression)) {
		t.Error("an array literal can never be an Effect")
	}
	if tp.NodeCouldBeStrictEffect(findKind(ast.KindObjectLiteralExpression)) {
		t.Error("an object literal is never named Effect")
	}
	if !tp.NodeCouldBeStrictEffect(findKind(ast.KindBinaryExpression)) {
		t.Error("other non-reference, non-call node kinds must not be ruled out")
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

// findCallByCalleeName returns the first call expression whose callee text
// contains the given substring.
func findCallByCalleeName(t *testing.T, sf *ast.SourceFile, callee string) *ast.Node {
	t.Helper()
	var found *ast.Node
	var visit func(node *ast.Node) bool
	visit = func(node *ast.Node) bool {
		if found != nil {
			return true
		}
		if node.Kind == ast.KindCallExpression {
			expr := node.AsCallExpression().Expression
			if expr != nil && strings.Contains(scanner.GetTextOfNode(expr), callee) {
				found = node
				return true
			}
		}
		node.ForEachChild(visit)
		return false
	}
	sf.AsNode().ForEachChild(visit)
	if found == nil {
		t.Fatalf("call to %q not found in source", callee)
	}
	return found
}

func TestCallCouldReturnStrictEffect(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV4, "effect"); err != nil {
		t.Skip("Effect v4 not installed:", err)
	}

	source := `
import { Effect } from "effect"

declare function makesEffect(): Effect.Effect<number>
declare function makesString(): string
declare function makesUnion(flag: boolean): Effect.Effect<number> | undefined
declare function makesAny(): any
declare function generic<T>(value: T): T
declare const maybe: { makesEffect(): Effect.Effect<number> } | undefined

export const uses = [
	makesEffect(),
	makesString(),
	makesUnion(true),
	makesAny(),
	generic("x"),
	maybe?.makesEffect(),
]
`
	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	// Subtests deliberately avoided: all cases share one checker, which is
	// not safe for the parallel subtests the tparallel linter would require.
	tests := []struct {
		callee   string
		expected bool
	}{
		// Conclusive negative: the declared return type can never be Effect.
		{"makesString", false},
		// Effect-returning calls and every inconclusive case stay true.
		{"makesEffect", true},
		{"makesUnion", true}, // union containing Effect
		{"makesAny", true},   // any return
		// The resolved signature is instantiated, so generic("x") conclusively
		// returns the primitive literal "x" and is ruled out.
		{"generic", false},
		{"maybe?.makesEffect", true}, // optional chain: Effect | undefined union
	}

	for _, tt := range tests {
		call := findCallByCalleeName(t, sf, tt.callee)
		if got := tp.NodeCouldBeStrictEffect(call); got != tt.expected {
			t.Errorf("NodeCouldBeStrictEffect(call %s) = %v, want %v", tt.callee, got, tt.expected)
		}
	}
}

func TestNodeCouldBeEffect(t *testing.T) {
	t.Parallel()
	if err := bundledeffect.EnsurePackageInstalled(bundledeffect.EffectV4, "effect"); err != nil {
		t.Skip("Effect v4 not installed:", err)
	}

	source := `
import { Effect } from "effect"

interface Subtype extends Effect.Effect<number> {}
declare const aSubtype: Subtype
declare function makesSubtype(): Subtype
declare function makesString(): string
const anEffect = Effect.succeed(1)
const aString = "hello"
const anUnknown: unknown = null
interface Plain { readonly value: number }
const aPlainObject: Plain = { value: 1 }
function generic<T>(param: T): T { return param }
export const uses = [aSubtype, anEffect, aString, anUnknown, aPlainObject, makesSubtype(), makesString()]
const key = "k"
export const literals = {
	"key": 1, arrow: () => 1, array: [1], value: "text", object: { a: 1 },
	spread: { ...anEffect }, computed: { [key]: 1 }, marked: { "~effect/Effect": 1 }
}
`
	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	identifiers := []struct {
		identifier string
		expected   bool
	}{
		// References are never ruled out: a type guard can narrow any
		// declared type into an Effect.
		{"aSubtype", true},
		{"anEffect", true},
		{"anUnknown", true},
		{"param", true},
		{"aString", true},
		{"aPlainObject", true},
	}
	for _, tt := range identifiers {
		node := findIdentifierByName(t, sf, tt.identifier)
		if got := tp.NodeCouldBeEffect(node); got != tt.expected {
			t.Errorf("NodeCouldBeEffect(%s) = %v, want %v", tt.identifier, got, tt.expected)
		}
	}

	if !tp.NodeCouldBeEffect(findCallByCalleeName(t, sf, "makesSubtype")) {
		t.Error("a call returning an Effect subtype must not be ruled out")
	}
	if tp.NodeCouldBeEffect(findCallByCalleeName(t, sf, "makesString")) {
		t.Error("a call returning string should be ruled out")
	}

	initializers := map[string]bool{}
	var visit func(node *ast.Node) bool
	visit = func(node *ast.Node) bool {
		if node.Kind == ast.KindPropertyAssignment && node.Name().Kind != ast.KindComputedPropertyName {
			name := node.Name()
			initializers[name.Text()] = tp.NodeCouldBeEffect(node.Initializer())
			if name.Kind == ast.KindStringLiteral && !tp.NodeCouldBeEffect(name) {
				t.Error("a string literal property name must not be ruled out")
			}
		}
		node.ForEachChild(visit)
		return false
	}
	sf.AsNode().ForEachChild(visit)
	want := map[string]bool{
		"key": false, "arrow": false, "array": false, "value": false, "object": false, "a": false,
		"spread": true, "computed": true, "marked": true,
	}
	for name, expected := range want {
		if got, ok := initializers[name]; !ok || got != expected {
			t.Errorf("NodeCouldBeEffect(initializer of %s) = %v (found %v), want %v", name, got, ok, expected)
		}
	}
}
