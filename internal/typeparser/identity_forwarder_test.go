package typeparser

import (
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

func TestUnwrapIdentityForwarder(t *testing.T) {
	t.Parallel()

	source := `
declare const call: (value: unknown) => unknown
declare const other: (value: unknown) => unknown
declare const genericCall: <A>(value: unknown) => A

const direct = call
const arrow = (value: unknown) => call(value)
const arrowBlock = (value: unknown) => { return call(value) }
const functionExpression = function(value: unknown) { return call(value) }
const parenthesized = (value: unknown) => (call)((value))
const differentCallee = (value: unknown) => other(value)
const typeArguments = (value: unknown) => genericCall<string>(value)

const wrapped = (value: unknown) => call(String(value))
const differentValue = (value: unknown) => call(undefined)
const shadowed = (value: unknown) => { const other = value; return call(other) }
const defaulted = (value: unknown = "fallback") => call(value)
const rest = (...values: Array<unknown>) => call(values)
const twoParameters = (value: unknown, other: unknown) => call(value)
const asynchronous = async (value: unknown) => call(value)
const generator = function*(value: unknown) { return call(value) }
const notACall = (value: unknown) => value
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, source)
	t.Cleanup(done)

	tests := []struct {
		name              string
		wantTarget        string
		wantUnwrap        bool
		wantTypeArguments int
	}{
		{name: "direct", wantTarget: "call"},
		{name: "arrow", wantTarget: "call", wantUnwrap: true},
		{name: "arrowBlock", wantTarget: "call", wantUnwrap: true},
		{name: "functionExpression", wantTarget: "call", wantUnwrap: true},
		{name: "parenthesized", wantTarget: "call", wantUnwrap: true},
		{name: "differentCallee", wantTarget: "other", wantUnwrap: true},
		{name: "typeArguments", wantTarget: "genericCall", wantUnwrap: true, wantTypeArguments: 1},
		{name: "wrapped"},
		{name: "differentValue"},
		{name: "shadowed"},
		{name: "defaulted"},
		{name: "rest"},
		{name: "twoParameters"},
		{name: "asynchronous"},
		{name: "generator"},
		{name: "notACall"},
	}

	for _, test := range tests {
		initializer := findVariableInitializer(t, sf, test.name)
		original := ast.SkipParentheses(initializer)
		got, typeArguments := tp.UnwrapIdentityForwarder(initializer)
		gotTypeArguments := 0
		if typeArguments != nil {
			gotTypeArguments = len(typeArguments.Nodes)
		}
		if gotTypeArguments != test.wantTypeArguments {
			t.Fatalf("UnwrapIdentityForwarder(%s) returned %d type arguments, want %d", test.name, gotTypeArguments, test.wantTypeArguments)
		}
		if test.wantUnwrap {
			if got == original {
				t.Fatalf("UnwrapIdentityForwarder(%s) did not unwrap", test.name)
			}
			if gotText := scanner.GetTextOfNode(got); gotText != test.wantTarget {
				t.Fatalf("UnwrapIdentityForwarder(%s) = %q, want %q", test.name, gotText, test.wantTarget)
			}
		} else if got != original {
			t.Fatalf("UnwrapIdentityForwarder(%s) unexpectedly unwrapped to %q", test.name, scanner.GetTextOfNode(got))
		}
	}
}

func findVariableInitializer(t *testing.T, sf *ast.SourceFile, name string) *ast.Node {
	t.Helper()

	var result *ast.Node
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil || result != nil {
			return true
		}
		if node.Kind == ast.KindVariableDeclaration {
			declaration := node.AsVariableDeclaration()
			if declaration != nil && declaration.Name() != nil && declaration.Name().Kind == ast.KindIdentifier &&
				declaration.Name().Text() == name {
				result = declaration.Initializer
				return true
			}
		}
		node.ForEachChild(walk)
		return false
	}
	walk(sf.AsNode())
	if result == nil {
		t.Fatalf("variable initializer %q not found", name)
	}
	return result
}
