package typeparser

import (
	"context"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

const typeAtLocationGuardFixtureMain = `import { value } from "./dep.js"
import * as ns from "./dep.js"
import type { Shape } from "./dep.js"
import d, { type Shape as Alias } from "./dep.js"
export function run(n: number = value, shape?: Shape & Alias): number {
  label: if (n > 0) { for (const item of [n, ns.value, d]) { n += item; break label } }
  switch (n) { case 1: return 1; default: }
  try { return shape?.n ?? n } catch (error) { throw error } finally { n = 0 }
}
export default run
`

const typeAtLocationGuardFixtureDep = `export const value = 1
export interface Shape { n: number }
export default value
`

const typeAtLocationGuardFixtureSyntax = "export {}\n" +
	"namespace N { export interface Base { n: number } export class Value { n = 1 } }\n" +
	"interface Derived extends N.Base {}\n" +
	"class Child extends N.Value implements N.Base, N.Base {}\n" +
	"declare function tag(parts: TemplateStringsArray, ...values: symbol[]): string\n" +
	"declare const s: symbol\n" +
	"const tagged = tag`value ${s} and ${s}`\n" +
	"const text = `value ${1}`\n" +
	"const deferred = import.defer(\"./dep.js\")\n" +
	"function target() { return new.target }\n" +
	"const meta = import.meta\n" +
	"type Mapped = { [K in \"a\"]: K }\n" +
	"enum E { A = 1 }\n"

const typeAtLocationGuardFixtureView = `declare global { namespace JSX { interface IntrinsicElements { div: { title?: string } } } }
declare function View(props: { title: string }): any
export const view = <div title="value"><View title="child" />{"text"}</div>
export const fragment = <><View title={String(1)} /></>
`

const typeAtLocationGuardFixtureJS = `// @ts-check
import { value } from "./dep.js"
/** @typedef {{ n: number }} Local */
/** @param {Local} local @returns {number} */
export function read(local) { return local.n + value }
export const cast = /** @type {Local} */ ({ n: value })
`

// getTypeAtLocationUnhoisted is GetTypeAtLocation with the guards inside the
// cached computation, as before they were hoisted.
func getTypeAtLocationUnhoisted(tp *TypeParser, store *denseNodeStore[*checker.Type], node *ast.Node) *checker.Type {
	return CachedNode(store, node, func() *checker.Type {
		if !isTypeAtLocationCandidate(node) {
			return nil
		}
		return tp.getTypeAtLocationUncached(node)
	})
}

func TestGetTypeAtLocationHoistedGuardsMatchUnhoisted(t *testing.T) {
	t.Parallel()

	for _, tc := range []struct {
		name    string
		version bundledeffect.EffectVersion
		sources map[string]string
	}{
		{"syntax", "", map[string]string{
			"/.src/main.ts":   typeAtLocationGuardFixtureMain,
			"/.src/dep.ts":    typeAtLocationGuardFixtureDep,
			"/.src/syntax.ts": typeAtLocationGuardFixtureSyntax,
			"/.src/view.tsx":  typeAtLocationGuardFixtureView,
			"/.src/read.js":   typeAtLocationGuardFixtureJS,
		}},
		{"effect", bundledeffect.EffectV4, map[string]string{
			"/.src/fixture.ts": effectContextFixtureTS,
			"/.src/fixture.js": effectContextFixtureJS,
		}},
	} {
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			tp, sourceFiles, done := compileFixturesWithJS(t, tc.version, tc.sources)
			defer done()

			var unhoisted denseNodeStore[*checker.Type]
			var nodes, typed, rejected int
			guards := map[string]int{}
			for i, sf := range sourceFiles {
				// The first file is compared before any file is checked. Asked
				// about an unchecked file, the checker can build an equal but new
				// type on each evaluation, so there the types are compared printed.
				checked := i > 0
				if i == 1 {
					for _, other := range sourceFiles {
						tp.checker.GetDiagnostics(t.Context(), other)
					}
				}
				same := func(a, b *checker.Type) bool {
					if a == b {
						return true
					}
					return !checked && a != nil && b != nil && tp.checker.TypeToString(a) == tp.checker.TypeToString(b)
				}
				var visit ast.Visitor
				visit = func(node *ast.Node) bool {
					nodes++
					// even files ask the unhoisted ordering first, odd files the hoisted one
					var first, second *checker.Type
					if i%2 == 0 {
						first = getTypeAtLocationUnhoisted(tp, &unhoisted, node)
						second = tp.GetTypeAtLocation(node)
					} else {
						first = tp.GetTypeAtLocation(node)
						second = getTypeAtLocationUnhoisted(tp, &unhoisted, node)
					}
					if !same(first, second) {
						t.Errorf("%s: %v at %d: %v, then %v", sf.FileName(), node.Kind, node.Pos(), first, second)
					}
					// repeated queries answer from the caches
					if again := tp.GetTypeAtLocation(node); again != tp.GetTypeAtLocation(node) || !same(again, first) {
						t.Errorf("%s: %v at %d: repeated query got %v, want %v", sf.FileName(), node.Kind, node.Pos(), again, first)
					}
					if first != nil {
						typed++
					}
					if !isTypeAtLocationCandidate(node) {
						rejected++
						for guard, hit := range map[string]bool{
							"jsx tag name":       node.Parent != nil && ast.IsJsxTagName(node),
							"jsx attribute name": node.Parent != nil && ast.IsJsxAttribute(node.Parent) && node.Parent.Name() == node,
							"import clause":      node.Kind == ast.KindImportClause,
							"tagged template":    node.Kind == ast.KindTemplateExpression,
							"meta property call": node.Kind == ast.KindMetaProperty,
							"type-only heritage": isInsideTypeOnlyHeritageExpression(node),
						} {
							if hit {
								guards[guard]++
							}
						}
						if tp.links.TypeAtLocation.Has(node) {
							t.Errorf("%s: %v at %d: rejected node has a cache entry", sf.FileName(), node.Kind, node.Pos())
						}
					} else if !tp.links.TypeAtLocation.Has(node) {
						t.Errorf("%s: %v at %d: admitted node has no cache entry", sf.FileName(), node.Kind, node.Pos())
					}
					ast.ForEachChildAndJSDoc(node, sf, visit)
					return false
				}
				visit(sf.AsNode())
			}
			t.Logf("%d files, %d nodes, %d typed, %d rejected", len(sourceFiles), nodes, typed, rejected)
			if typed == 0 || rejected == 0 {
				t.Fatal("fixtures do not exercise the guards")
			}
			// Type-only heritage elements parse as type references with some
			// providers, so that guard is not required to fire.
			if tc.version == "" {
				for _, guard := range []string{"jsx tag name", "jsx attribute name", "import clause", "tagged template", "meta property call"} {
					if guards[guard] == 0 {
						t.Errorf("fixtures do not exercise the %s guard", guard)
					}
				}
			}
		})
	}
}

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
