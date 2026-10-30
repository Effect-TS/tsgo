package typeparser

import (
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
)

const effectContextFixtureTS = `import { Effect, pipe } from "effect"
import * as Eff from "effect/Effect"
import { gen, sync as syncAlias } from "effect/Effect"

class Service {
  readonly n = 1
  method() {
    return Effect.gen(this, function*() {
      return this.n
    })
  }
}

export const nested = Effect.gen(function*() {
  const a = yield* Effect.succeed(1)
  const inner = Effect.gen(function*() {
    const b: number = yield* Effect.succeed(a)
    try {
      yield* Effect.fail("x")
    } catch (e) {
      return e
    }
    return b
  })
  const plain = function* () {
    yield 1
    yield* [2, 3]
    return Effect.succeed(1)
  }
  const arrow = (x: number = a): number => x + 1
  function declared(this: unknown, y = () => Effect.succeed(1)) {
    return y()
  }
  const obj = {
    method() { return Effect.succeed(1) },
    get value() { return 2 },
    set value(v: number) {},
    *gen() { yield 1 },
    prop: () => Effect.sync(() => new Date())
  }
  class Local {
    field = Effect.sync(() => Date.now())
    constructor(readonly z: number = 1) {}
    static { Date.now() }
  }
  // @effect-diagnostics-next-line returnEffectInGen:off
  return yield* inner.pipe(Effect.map((n) => n + 1))
}).pipe(Effect.withSpan("nested"))

export const fnTraced = Effect.fn("fnTraced")(function*(x: number) {
  yield* Effect.log(x)
  const nestedFn = Effect.fn(function*() { return yield* Effect.succeed(x) })
  return x
}, Effect.map((n) => n + 1))

export const fnDirect = Effect.fn(function*(x: number) {
  return yield* Effect.succeed(x)
})

export const fnUntraced = Effect.fnUntraced(function*() {
  return yield* Effect.succeed(1)
})

export const fnArrow = Effect.fn("arrow")((x: number) => Effect.succeed(x))

export const fnOptions = Effect.fn({ self: 1 } as any, function*(this: unknown) {
  const self = this
  return yield* Effect.succeed(self)
})

export class WithAccessors {
  private readonly value = 1
  get eff() {
    return Effect.gen(this, function*() { return this.value })
  }
  set eff(_: unknown) {
    Effect.runSync(Effect.sync(() => Date.now()))
  }
  static make = Effect.fn("WithAccessors.make")(function*() {
    class Inner { m() { return Date.now() } }
    try {
      return new Inner().m()
    } catch {
      return yield* Effect.die("x")
    } finally {
      yield* Effect.log("done")
    }
  })
}

export const thunks = [
  Effect.sync(() => Date.now()),
  Effect.sync(function() { return Date.now() }),
  Effect.sync((() => Date.now())),
  Effect.sync((((() => Date.now())))),
  Effect.sync((() => Date.now()) as () => number),
  Effect.sync((() => Date.now())!),
  Effect.sync((() => Date.now()) satisfies () => number),
  Effect.sync(<() => number>(() => Date.now())),
  Effect.sync(async () => Date.now()),
  Effect.promise(async () => Date.now()),
  Effect.promise(async function*() { yield 1 } as any),
  Effect.suspend(() => Effect.succeed(Date.now())),
  Effect.async<number>((resume) => { resume(Effect.succeed(Date.now())) }),
  Effect.callback<number>((resume) => { resume(Effect.succeed(Date.now())) }),
  Effect.try(() => JSON.parse("1")),
  Effect.try(function() { return JSON.parse("1") }),
  Effect.try({ try: () => JSON.parse("1"), catch: (e) => e }),
  Effect.try(({ try: () => JSON.parse("1"), catch: (e) => e })),
  Effect.try({ try: (() => JSON.parse("1")), catch: (e) => e }),
  Effect.try({ try: () => 1, try: () => 2, catch: (e) => e } as any),
  Effect.try({ "try": () => 1, catch: (e: unknown) => e } as any),
  Effect.try({ try: { try: () => 1 }, catch: (e: unknown) => e } as any),
  Effect.tryPromise({ try: async () => 1, catch: (e) => e }),
  Effect.tryPromise({ try() { return Promise.resolve(1) }, catch: (e) => e }),
  Effect.sync(Date.now),
  Effect.sync(true ? () => 1 : () => 2),
  Effect.sync(...[() => 1]),
  Eff.sync(() => Date.now()),
  syncAlias(() => Date.now()),
  gen(function*() { return yield* Eff.sync(() => 1) }),
  Effect.gen({ self: 1 }, function*() { return yield* Effect.succeed(1) }),
  Effect.sync(() => Effect.runSync(Effect.gen(function*() { return 1 }))),
  Effect.sync(() => () => Date.now()),
  Effect.gen(function*() {
    const x = yield* Effect.sync(() => () => Date.now())
    const y = yield* Effect.try({ try: () => Effect.gen(function*() { return 1 }), catch: (e) => e })
    return Effect.sync(() => { const f = () => Date.now(); return f() })
  }),
]

export const piped = pipe(
  Effect.succeed(1),
  Effect.flatMap((n) => Effect.gen(function*() { return n }))
)

function* notEffect(): Generator<number> {
  yield* notEffect()
  return Effect.gen(function*() {})
}
`

const effectContextFixtureJS = `// @ts-check
import { Effect } from "effect"

/** @typedef {{ a: number }} Shape */

/**
 * @template T
 * @param {T} value
 * @returns {Effect.Effect<T>}
 */
export function lift(value) {
  return Effect.gen(function* () {
    /** @type {number} */
    const n = yield* Effect.succeed(1)
    const cast = /** @type {number} */ (n)
    const sat = /** @satisfies {number} */ (n)
    return value
  })
}

/** @type {(x: number) => Effect.Effect<number>} */
export const typed = (x) => Effect.sync(() => /** @type {number} */ (Date.now()) + x)

export const castThunk = Effect.sync(/** @type {() => number} */ (() => Date.now()))

export const tried = Effect.try({ try: /** @type {() => number} */ (() => 1), catch: (e) => e })

/**
 * @overload
 * @param {string} a
 * @returns {void}
 */
/**
 * @param {string | number} a
 */
export function over(a) {}

export class K {
  /** @type {number} */
  x = 1
  /** @param {number} y */
  m(y) { return Effect.gen(this, function* () { return y }) }
}
`

func denseEffectContextReference(tp *TypeParser, sf *ast.SourceFile) (func(*ast.Node) EffectContextFlags, func(*ast.Node) *ast.FunctionExpression) {
	var flagsStore core.LinkStore[*ast.Node, EffectContextFlags]
	var generatorStore core.LinkStore[*ast.Node, *ast.FunctionExpression]

	var walk ast.Visitor
	var pendingEnableFlags core.LinkStore[*ast.Node, EffectContextFlags]
	var pendingDisableFlags core.LinkStore[*ast.Node, EffectContextFlags]
	pendingFlagsMask := EffectContextFlagPendingNextFunctionIsEffectThunk | EffectContextFlagPendingNextObjectTryPropertyIsEffectThunk
	functionScopeResetFlags := EffectContextFlagCanYieldEffect | EffectContextFlagInEffectConstructorThunk | pendingFlagsMask

	setPendingEnableFlags := func(node *ast.Node, flags EffectContextFlags) {
		if node == nil || flags == EffectContextFlagNone {
			return
		}
		*pendingEnableFlags.Get(node) |= flags
	}

	setPendingDisableFlags := func(node *ast.Node, flags EffectContextFlags) {
		if node == nil || flags == EffectContextFlagNone {
			return
		}
		*pendingDisableFlags.Get(node) |= flags
	}

	transparentPendingExpression := func(node *ast.Node) *ast.Node {
		if node == nil {
			return nil
		}
		switch node.Kind {
		case ast.KindParenthesizedExpression, ast.KindSatisfiesExpression, ast.KindAsExpression, ast.KindNonNullExpression, ast.KindTypeAssertionExpression:
			return node.Expression()
		default:
			return nil
		}
	}

	resetChildFunctionScopeFlags := func(node *ast.Node) bool {
		setPendingDisableFlags(node, functionScopeResetFlags)
		return false
	}

	resetPendingFlags := func(child *ast.Node) bool {
		setPendingDisableFlags(child, pendingFlagsMask)
		return false
	}

	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}

		if node.Parent != nil {
			*flagsStore.Get(node) = *flagsStore.Get(node.Parent)
			if !generatorStore.Has(node) {
				*generatorStore.Get(node) = *generatorStore.Get(node.Parent)
			}
		} else {
			*flagsStore.Get(node) = EffectContextFlagNone
		}

		if pendingDisableFlags.Has(node) {
			*flagsStore.Get(node) &^= *pendingDisableFlags.TryGet(node)
		}

		if pendingEnableFlags.Has(node) {
			*flagsStore.Get(node) |= *pendingEnableFlags.TryGet(node)
		}

		if *flagsStore.Get(node)&EffectContextFlagPendingNextFunctionIsEffectThunk != 0 && (node.Kind == ast.KindArrowFunction || node.Kind == ast.KindFunctionExpression) {
			if body := node.Body(); body != nil {
				setPendingEnableFlags(body.AsNode(), EffectContextFlagInEffectConstructorThunk)
				setPendingDisableFlags(body.AsNode(), pendingFlagsMask)
			}
		} else if *flagsStore.Get(node)&EffectContextFlagPendingNextObjectTryPropertyIsEffectThunk != 0 && node.Kind == ast.KindObjectLiteralExpression {
			node.ForEachChild(resetPendingFlags)

			obj := node.AsObjectLiteralExpression()
			if obj != nil && obj.Properties != nil {
				for _, prop := range obj.Properties.Nodes {
					if prop == nil || prop.Kind != ast.KindPropertyAssignment {
						continue
					}
					assignment := prop.AsPropertyAssignment()
					if assignment == nil || assignment.Name() == nil || assignment.Initializer == nil {
						continue
					}
					if assignment.Name().Text() != "try" {
						continue
					}
					setPendingEnableFlags(assignment.Initializer, EffectContextFlagPendingNextFunctionIsEffectThunk)
				}
			}
		} else if expr := transparentPendingExpression(node); expr != nil {
			setPendingEnableFlags(expr, *flagsStore.Get(node)&pendingFlagsMask)
		} else if *flagsStore.Get(node)&pendingFlagsMask != 0 {
			node.ForEachChild(resetPendingFlags)
		}

		if effectGen := tp.EffectGenCall(node); effectGen != nil {
			bodyNode := effectGen.body.AsNode()
			setPendingEnableFlags(bodyNode, EffectContextFlagCanYieldEffect)
			*generatorStore.Get(bodyNode) = effectGen.generatorFunction
		} else if effectFn := tp.EffectFnCall(node); effectFn != nil && effectFn.IsGenerator() {
			body := effectFn.Body()
			genFn := effectFn.GeneratorFunction()
			if body != nil && genFn != nil {
				bodyNode := body.AsNode()
				setPendingEnableFlags(bodyNode, EffectContextFlagCanYieldEffect)
				*generatorStore.Get(bodyNode) = genFn
			}
		}

		if node.Kind == ast.KindCallExpression {
			call := node.AsCallExpression()
			if call != nil && call.Arguments != nil && len(call.Arguments.Nodes) > 0 {
				effectThunkArg := call.Arguments.Nodes[0]
				switch {
				case tp.IsNodeReferenceToEffectModuleApi(call.Expression, "sync"),
					tp.IsNodeReferenceToEffectModuleApi(call.Expression, "promise"),
					tp.IsNodeReferenceToEffectModuleApi(call.Expression, "callback"),
					tp.IsNodeReferenceToEffectModuleApi(call.Expression, "suspend"):
					setPendingEnableFlags(effectThunkArg, EffectContextFlagPendingNextFunctionIsEffectThunk)
				case tp.IsNodeReferenceToEffectModuleApi(call.Expression, "try"),
					tp.IsNodeReferenceToEffectModuleApi(call.Expression, "tryPromise"):
					setPendingEnableFlags(effectThunkArg, EffectContextFlagPendingNextFunctionIsEffectThunk|EffectContextFlagPendingNextObjectTryPropertyIsEffectThunk)
				}
			}
		}

		if ast.IsFunctionLike(node) {
			node.ForEachChild(resetChildFunctionScopeFlags)
		}

		if *flagsStore.Get(node)&EffectContextFlagCanYieldEffect == 0 {
			*generatorStore.Get(node) = nil
		}

		node.ForEachChild(walk)
		return false
	}
	walk(sf.AsNode())

	flagsOf := func(node *ast.Node) EffectContextFlags {
		for current := node; current != nil; current = current.Parent {
			if flags := flagsStore.TryGet(current); flags != nil {
				return *flags & EffectContextFlagInEffect
			}
		}
		return EffectContextFlagNone
	}
	generatorOf := func(node *ast.Node) *ast.FunctionExpression {
		for current := node; current != nil; current = current.Parent {
			if genFn := generatorStore.TryGet(current); genFn != nil {
				return *genFn
			}
		}
		return nil
	}
	return flagsOf, generatorOf
}

func allEffectContextNodes(sf *ast.SourceFile) []*ast.Node {
	var nodes []*ast.Node
	var visit ast.Visitor
	visit = func(node *ast.Node) bool {
		nodes = append(nodes, node)
		ast.ForEachChildAndJSDoc(node, sf, visit)
		return false
	}
	visit(sf.AsNode())
	return nodes
}

type effectContextComparison struct {
	nodes, stored, canYield, inThunk, jsdoc int
	generators                              map[*ast.FunctionExpression]bool
}

func compareEffectContext(t *testing.T, tp *TypeParser, sourceFiles []*ast.SourceFile) effectContextComparison {
	t.Helper()
	result := effectContextComparison{generators: map[*ast.FunctionExpression]bool{}}
	for i, sf := range sourceFiles {
		nodes := allEffectContextNodes(sf)
		if i%2 == 1 {
			slices.Reverse(nodes)
		}
		var wantFlags func(*ast.Node) EffectContextFlags
		var wantGenerator func(*ast.Node) *ast.FunctionExpression
		referenceAfterFirstQuery := func() {
			if wantFlags == nil {
				wantFlags, wantGenerator = denseEffectContextReference(tp, sf)
			}
		}
		mismatches := 0
		for _, node := range nodes {
			var gotFlags EffectContextFlags
			var gotGenerator *ast.FunctionExpression
			if i%3 == 1 {
				gotGenerator = tp.GetEffectYieldGeneratorFunction(node)
				gotFlags = tp.GetEffectContextFlags(node)
			} else {
				gotFlags = tp.GetEffectContextFlags(node)
				gotGenerator = tp.GetEffectYieldGeneratorFunction(node)
			}
			referenceAfterFirstQuery()
			if want := wantFlags(node); gotFlags != want || gotGenerator != wantGenerator(node) {
				mismatches++
				if mismatches <= 10 {
					t.Errorf("%s: %v at %d: flags %d, want %d; generator %p, want %p", sf.FileName(), node.Kind, node.Pos(), gotFlags, want, gotGenerator, wantGenerator(node))
				}
			}
			result.nodes++
			if tp.links.EffectContextFlags.Has(node) || tp.links.EffectYieldGeneratorFunction.Has(node) {
				result.stored++
			}
			if node.Flags&ast.NodeFlagsJSDoc != 0 {
				result.jsdoc++
			}
			if gotFlags&EffectContextFlagCanYieldEffect != 0 {
				result.canYield++
			}
			if gotFlags&EffectContextFlagInEffectConstructorThunk != 0 {
				result.inThunk++
			}
			if gotGenerator != nil {
				result.generators[gotGenerator] = true
			}
		}
		if !tp.links.EffectContextFlags.Has(sf.AsNode()) || !tp.links.EffectYieldGeneratorFunction.Has(sf.AsNode()) {
			t.Errorf("%s: analyzed source file has no entry", sf.FileName())
		}
	}
	return result
}

func TestEffectContextMatchesPerNodeAnalysis(t *testing.T) {
	t.Parallel()
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			tp, sourceFiles, done := compileFixturesWithJS(t, version, map[string]string{
				"/.src/fixture.ts": effectContextFixtureTS,
				"/.src/fixture.js": effectContextFixtureJS,
			})
			defer done()
			result := compareEffectContext(t, tp, sourceFiles)
			t.Logf("%d nodes, %d with an entry, %d JSDoc, %d can yield, %d in thunks, %d generator functions", result.nodes, result.stored, result.jsdoc, result.canYield, result.inThunk, len(result.generators))
			if result.canYield == 0 || result.inThunk == 0 || result.jsdoc == 0 || len(result.generators) < 10 {
				t.Fatalf("fixture does not exercise the analysis")
			}
			if result.stored*4 > result.nodes {
				t.Errorf("%d of %d nodes have an entry, want at most a quarter", result.stored, result.nodes)
			}
		})
	}
}

func TestEffectContextMatchesPerNodeAnalysisOnRuleFixtures(t *testing.T) {
	t.Parallel()
	for _, version := range []bundledeffect.EffectVersion{bundledeffect.EffectV3, bundledeffect.EffectV4} {
		t.Run(string(version), func(t *testing.T) {
			t.Parallel()
			root := filepath.Join("..", "..", "testdata", "tests")
			dirs, err := filepath.Glob(filepath.Join(root, string(version)+"*"))
			if err != nil || len(dirs) == 0 {
				t.Fatalf("no fixture directories for %s: %v", version, err)
			}
			sources := map[string]string{}
			for _, dir := range dirs {
				entries, err := os.ReadDir(dir)
				if err != nil {
					t.Fatalf("read %s: %v", dir, err)
				}
				for _, entry := range entries {
					if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".ts") {
						continue
					}
					content, err := os.ReadFile(filepath.Join(dir, entry.Name()))
					if err != nil {
						t.Fatalf("read %s: %v", entry.Name(), err)
					}
					sources["/.src/"+filepath.Base(dir)+"/"+entry.Name()] = string(content)
				}
			}
			tp, sourceFiles, done := compileFixturesWithJS(t, version, sources)
			defer done()
			result := compareEffectContext(t, tp, sourceFiles)
			t.Logf("%d files, %d nodes, %d with an entry, %d can yield, %d in thunks, %d generator functions", len(sourceFiles), result.nodes, result.stored, result.canYield, result.inThunk, len(result.generators))
			if result.canYield == 0 || result.inThunk == 0 {
				t.Fatalf("fixtures do not exercise the analysis")
			}
			if result.stored*10 > result.nodes {
				t.Errorf("%d of %d nodes have an entry, want at most a tenth", result.stored, result.nodes)
			}
		})
	}
}
