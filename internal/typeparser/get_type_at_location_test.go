package typeparser

import (
	"context"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

// Import clauses without a default binding declare no symbol; before the
// KindImportClause guard, checker.GetTypeAtLocation panicked on them with a
// nil symbol dereference (recovered silently, see issue #301 investigation).
func TestGetTypeAtLocationImportClause(t *testing.T) {
	t.Parallel()

	sources := map[string]string{
		"/.src/dep.ts": `
export const A = 1
export type T = string
const d = { A }
export default d
`,
		"/.src/main.ts": `
import { A } from "./dep.js"
import * as ns from "./dep.js"
import type { T } from "./dep.js"
import d from "./dep.js"
export const use: T = String(A + ns.A + d.A)
`,
	}

	_, tp, sourceFiles, done := compileAndGetCheckerAndSourceFilesInternal(t, sources)
	defer done()

	var clauses []*ast.Node
	var visit func(node *ast.Node) bool
	visit = func(node *ast.Node) bool {
		if node.Kind == ast.KindImportClause {
			clauses = append(clauses, node)
		}
		node.ForEachChild(visit)
		return false
	}
	sourceFiles["/.src/main.ts"].AsNode().ForEachChild(visit)

	if len(clauses) != 4 {
		t.Fatalf("expected 4 import clauses, got %d", len(clauses))
	}

	for _, clause := range clauses {
		if got := tp.GetTypeAtLocation(clause); got != nil {
			t.Errorf("expected nil type for import clause %q, got %v", clause.Parent.Text(), got)
		}
	}
}

// Re-evaluating these expressions after the check gives a different type than
// the check itself computed: the iterator's return literals were checked while
// the enclosing generic call was still resolving, and the narrowed reference
// was checked with its constraint substituted.
const checkedTypeSource = `
declare const dual: {
  <DataLast extends (...args: Array<any>) => any, DataFirst extends (...args: Array<any>) => any>(
    arity: Parameters<DataFirst>["length"],
    body: DataFirst
  ): DataLast & DataFirst
}
export const takeWhileDefined: {
  <A>(f: (input: A) => A | undefined): (self: Iterable<A>) => Iterable<A>
  <A>(self: Iterable<A>, f: (input: A) => A | undefined): Iterable<A>
} = dual(2, <A>(self: Iterable<A>, f: (input: A) => A | undefined) => ({
  [Symbol.iterator]() {
    const iterator = self[Symbol.iterator]()
    return {
      next() {
        const result = iterator.next()
        if (result.done) {
          return { done: true, value: undefined }
        }
        return { done: false, value: f(result.value) }
      }
    }
  }
}))

export const getRandomValues = <T extends ArrayBufferView | null>(array: T): T => {
  if (array instanceof Uint8Array) {
    array[0] = 1
  }
  return array
}
`

func TestGetTypeAtLocationReturnsCheckedType(t *testing.T) {
	t.Parallel()

	c, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, checkedTypeSource)
	defer done()
	c.GetDiagnostics(context.Background(), sf)

	got := map[string]string{}
	var visit func(node *ast.Node) bool
	visit = func(node *ast.Node) bool {
		switch {
		case ast.IsReturnStatement(node) && node.Expression() != nil && ast.IsObjectLiteralExpression(node.Expression()):
			got["return "+scannerText(sf, node.Expression())] = c.TypeToString(tp.GetTypeAtLocation(node.Expression()))
		case ast.IsBinaryExpression(node) && node.AsBinaryExpression().OperatorToken.Kind == ast.KindInstanceOfKeyword:
			left := node.AsBinaryExpression().Left
			got["instanceof "+scannerText(sf, left)] = c.TypeToString(tp.GetTypeAtLocation(left))
		}
		node.ForEachChild(visit)
		return false
	}
	sf.AsNode().ForEachChild(visit)

	expected := map[string]string{
		"return { done: true, value: undefined }":        "{ done: boolean; value: undefined; }",
		"return { done: false, value: f(result.value) }": "{ done: boolean; value: A | undefined; }",
		"instanceof array":                               "ArrayBufferView<ArrayBufferLike> | null",
	}
	for key, want := range expected {
		if got[key] != want {
			t.Errorf("%s: got %q, want %q", key, got[key], want)
		}
	}
}

func TestGetTypeAtLocationAddsNoDiagnosticsAfterCheck(t *testing.T) {
	t.Parallel()

	c, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, checkedTypeSource+`
declare function takesString<T extends string>(x: T): T
export const bad = takesString(1)
export function inferred() { return takesString(2) }
`)
	defer done()
	before := len(c.GetDiagnostics(context.Background(), sf))
	if before == 0 {
		t.Fatal("expected the source to have type errors")
	}

	var visit func(node *ast.Node) bool
	visit = func(node *ast.Node) bool {
		if ast.IsExpressionNode(node) {
			tp.GetTypeAtLocation(node)
		}
		node.ForEachChild(visit)
		return false
	}
	sf.AsNode().ForEachChild(visit)

	if after := len(c.GetDiagnostics(context.Background(), sf)); after != before {
		t.Errorf("querying every expression added %d diagnostics", after-before)
	}
}

func scannerText(sf *ast.SourceFile, node *ast.Node) string {
	text := sf.Text()[node.Pos():node.End()]
	for len(text) > 0 && (text[0] == ' ' || text[0] == '\n') {
		text = text[1:]
	}
	return text
}
