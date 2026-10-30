// @effect-v4
import { Context, Effect } from "effect"

declare const items: ReadonlyArray<number>
declare const entries: ReadonlyArray<readonly [string, number]>
declare const lookup: ReadonlyMap<string, number>
declare const processItem: (item: number) => Effect.Effect<string>
declare const processIndexed: (item: number, index: number) => Effect.Effect<string>
declare const processWithOptions: (item: number, options?: { readonly retries: number }) => Effect.Effect<string>
declare const processPair: (key: string, value: number) => Effect.Effect<string>
declare const service: { readonly process: (item: number) => Effect.Effect<string> }
declare const loadItems: Effect.Effect<ReadonlyArray<number>>
declare function plainGenerator(item: number): Generator<never, string, unknown>

class Processor extends Context.Service<Processor, { readonly prefix: string }>()("Processor") {}

// ---------------------------------------------------------------------------
// MUST TRIGGER, with quick fix
// ---------------------------------------------------------------------------

// MUST TRIGGER: the issue's canonical example, block body.
// fix: const results = yield* Effect.forEach(items, processItem)
export const blockBody = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processItem(item))
  }
  return results
})

// MUST TRIGGER: loop body without a block.
// fix: const results = yield* Effect.forEach(items, processItem)
export const statementBody = Effect.gen(function*() {
  const results = []
  for (const item of items) results.push(yield* processItem(item))
  return results
})

// MUST TRIGGER: typed accumulator; the fix keeps the annotation.
// fix: const results: Array<string> = yield* Effect.forEach(items, processItem)
export const typedAccumulator = Effect.gen(function*() {
  const results: Array<string> = []
  for (const item of items) {
    results.push(yield* processItem(item))
  }
  return results
})

// MUST TRIGGER: inline effect expression, the fix wraps it in an arrow.
// fix: const doubled = yield* Effect.forEach(items, (item) => Effect.succeed(item * 2))
export const inlineExpression = Effect.gen(function*() {
  const doubled = []
  for (const item of items) {
    doubled.push(yield* Effect.succeed(item * 2))
  }
  return doubled
})

// MUST TRIGGER: the call passes more than the loop variable, so the fix keeps
// an arrow instead of the point-free form.
// fix: const results = yield* Effect.forEach(items, (item) => processIndexed(item, 0))
export const extraArgument = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processIndexed(item, 0))
  }
  return results
})

// MUST TRIGGER: the callee has an optional second parameter that is not an
// index; passing it point-free would feed the index into `options`, so the fix
// keeps an arrow.
// fix: const results = yield* Effect.forEach(items, (item) => processWithOptions(item))
export const optionalSecondParameter = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processWithOptions(item))
  }
  return results
})

// MUST TRIGGER: method call; passing service.process point-free would drop
// `this`, so the fix keeps an arrow.
// fix: const results = yield* Effect.forEach(items, (item) => service.process(item))
export const methodCall = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* service.process(item))
  }
  return results
})

// MUST TRIGGER: destructuring loop binding, reused as the arrow parameter.
// fix: const results = yield* Effect.forEach(entries, ([key, value]) => processPair(key, value))
export const destructuredBinding = Effect.gen(function*() {
  const results = []
  for (const [key, value] of entries) {
    results.push(yield* processPair(key, value))
  }
  return results
})

// MUST TRIGGER: any Iterable works, not only arrays.
// fix: const results = yield* Effect.forEach(lookup, ([key, value]) => processPair(key, value))
export const mapIterable = Effect.gen(function*() {
  const results = []
  for (const [key, value] of lookup) {
    results.push(yield* processPair(key, value))
  }
  return results
})

// MUST TRIGGER: only declarations of other variables between the accumulator
// and the loop; the iterable is one of them, so the fix replaces the loop in
// place and leaves those declarations where they are.
// fix: (const results = [] removed)
//      const loaded = yield* loadItems
//      const offset = 1
//      const results = yield* Effect.forEach(loaded, (item) => Effect.succeed(item + offset))
export const declarationsInBetween = Effect.gen(function*() {
  const results = []
  const loaded = yield* loadItems
  const offset = 1
  for (const item of loaded) {
    results.push(yield* Effect.succeed(item + offset))
  }
  return results
})

// MUST TRIGGER: the accumulator is only read after the loop.
// fix: const results = yield* Effect.forEach(items, processItem)
export const readAfterwards = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processItem(item))
  }
  yield* Effect.log(results.length)
  return results.join(",")
})

// MUST TRIGGER: the result of the loop body does not depend on the loop variable.
// fix: const results = yield* Effect.forEach(items, (item) => Effect.succeed("tick"))
export const loopVariableUnused = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* Effect.succeed("tick"))
  }
  return results
})

// MUST TRIGGER: the loop is nested in a block of the generator.
// fix: const results = yield* Effect.forEach(items, processItem)
export const nestedBlock = Effect.gen(function*() {
  if (items.length > 0) {
    const results = []
    for (const item of items) {
      results.push(yield* processItem(item))
    }
    return results
  }
  return []
})

// MUST TRIGGER: Effect.fn generator.
// fix: const results = yield* Effect.forEach(input, processItem)
export const withEffectFn = Effect.fn("withEffectFn")(function*(input: ReadonlyArray<number>) {
  const results = []
  for (const item of input) {
    results.push(yield* processItem(item))
  }
  return results
})

// MUST TRIGGER: Effect.fn generator with pipeable arguments.
// fix: const results = yield* Effect.forEach(input, processItem)
export const withEffectFnPipe = Effect.fn("withEffectFnPipe")(
  function*(input: ReadonlyArray<number>) {
    const results = []
    for (const item of input) {
      results.push(yield* processItem(item))
    }
    return results
  },
  Effect.withSpan("withEffectFnPipe.span")
)

// MUST TRIGGER: Effect.fnUntraced generator.
// fix: const results = yield* Effect.forEach(input, processItem)
export const withEffectFnUntraced = Effect.fnUntraced(function*(input: ReadonlyArray<number>) {
  const results = []
  for (const item of input) {
    results.push(yield* processItem(item))
  }
  return results
})

// MUST TRIGGER: Effect.gen nested in another Effect.gen.
// fix: const results = yield* Effect.forEach(items, processItem)
export const nestedGen = Effect.gen(function*() {
  const inner = yield* Effect.gen(function*() {
    const results = []
    for (const item of items) {
      results.push(yield* processItem(item))
    }
    return results
  })
  return inner
})

// MUST TRIGGER: a service key is itself an Effect in v4.
// fix: const services = yield* Effect.forEach(items, (_) => Processor)
export const yieldedService = Effect.gen(function*() {
  const services = []
  for (const _ of items) {
    services.push(yield* Processor)
  }
  return services
})

// ---------------------------------------------------------------------------
// MUST TRIGGER, without quick fix (the accumulator is mutated afterwards)
// ---------------------------------------------------------------------------

// MUST TRIGGER (no fix): another push after the loop.
export const pushedAfterwards = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processItem(item))
  }
  results.push("done")
  return results
})

// MUST TRIGGER (no fix): index write after the loop.
export const indexWriteAfterwards = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processItem(item))
  }
  results[0] = "first"
  return results
})

// MUST TRIGGER (no fix): splice after the loop.
export const splicedAfterwards = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processItem(item))
  }
  results.splice(0, 1)
  return results
})

// MUST TRIGGER (no fix) on the first loop only: the second loop keeps pushing
// into the same accumulator, which is a mutation after the first loop; the
// second loop is not right after the declaration.
export const twoLoopsSameAccumulator = Effect.gen(function*() {
  const results = []
  for (const item of items) {
    results.push(yield* processItem(item))
  }
  for (const item of items) {
    results.push(yield* processIndexed(item, 1))
  }
  return results
})

// ---------------------------------------------------------------------------
// MUST NOT TRIGGER
// ---------------------------------------------------------------------------

// MUST NOT TRIGGER: filtering inside the loop.
export const filtering = Effect.gen(function*() {
  const out = []
  for (const item of items) {
    if (item > 0) out.push(yield* processItem(item))
  }
  return out
})

// MUST NOT TRIGGER: another statement in the loop body.
export const otherStatement = Effect.gen(function*() {
  const out = []
  for (const item of items) {
    yield* Effect.log(item)
    out.push(yield* processItem(item))
  }
  return out
})

// MUST NOT TRIGGER: break in the loop.
export const withBreak = Effect.gen(function*() {
  const out = []
  for (const item of items) {
    out.push(yield* processItem(item))
    break
  }
  return out
})

// MUST NOT TRIGGER: continue in the loop.
export const withContinue = Effect.gen(function*() {
  const out = []
  for (const item of items) {
    out.push(yield* processItem(item))
    continue
  }
  return out
})

// MUST NOT TRIGGER: early return in the loop.
export const withReturn = Effect.gen(function*() {
  const out: Array<string> = []
  for (const item of items) {
    out.push(yield* processItem(item))
    return out
  }
  return out
})

// MUST NOT TRIGGER: the pushed value is not directly a yield*.
export const transformedValue = Effect.gen(function*() {
  const out = []
  for (const item of items) {
    out.push((yield* processItem(item)).toUpperCase())
  }
  return out
})

// MUST NOT TRIGGER: the pushed value is a plain value.
export const plainPush = Effect.gen(function*() {
  const out = []
  for (const item of items) {
    out.push(item * 2)
  }
  return out
})

// MUST NOT TRIGGER: several arguments pushed at once.
export const multiplePushArguments = Effect.gen(function*() {
  const out = []
  for (const item of items) {
    out.push(yield* processItem(item), yield* processItem(item + 1))
  }
  return out
})

// MUST NOT TRIGGER: spread push of a yielded array.
export const spreadPush = Effect.gen(function*() {
  const out = []
  for (const item of items) {
    out.push(...(yield* Effect.succeed([item, item])))
  }
  return out
})

// MUST NOT TRIGGER: unshift instead of push changes the order.
export const unshift = Effect.gen(function*() {
  const out = []
  for (const item of items) {
    out.unshift(yield* processItem(item))
  }
  return out
})

// MUST NOT TRIGGER: the loop body reads the accumulator.
export const bodyReadsAccumulator = Effect.gen(function*() {
  const out: Array<string> = []
  for (const item of items) {
    out.push(yield* processIndexed(item, out.length))
  }
  return out
})

// MUST NOT TRIGGER: the accumulator is not empty.
export const nonEmptyAccumulator = Effect.gen(function*() {
  const out = ["header"]
  for (const item of items) {
    out.push(yield* processItem(item))
  }
  return out
})

// MUST NOT TRIGGER: the accumulator is not an empty array literal.
export const arrayConstructor = Effect.gen(function*() {
  const out = new Array<string>()
  for (const item of items) {
    out.push(yield* processItem(item))
  }
  return out
})

// MUST NOT TRIGGER: the accumulator is declared with let.
export const letAccumulator = Effect.gen(function*() {
  let out: Array<string> = []
  for (const item of items) {
    out.push(yield* processItem(item))
  }
  out = out.slice(1)
  return out
})

// MUST NOT TRIGGER: the accumulator comes from an outer scope.
const outerAccumulator: Array<string> = []
export const outerScope = Effect.gen(function*() {
  for (const item of items) {
    outerAccumulator.push(yield* processItem(item))
  }
  return outerAccumulator
})

// MUST NOT TRIGGER: a non-declaration statement between the accumulator and the loop.
export const statementInBetween = Effect.gen(function*() {
  const out = []
  yield* Effect.log("start")
  for (const item of items) {
    out.push(yield* processItem(item))
  }
  return out
})

// MUST NOT TRIGGER: a declaration in between references the accumulator.
export const declarationReferencesAccumulator = Effect.gen(function*() {
  const out: Array<string> = []
  const alias = out
  for (const item of items) {
    out.push(yield* processItem(item))
  }
  alias.push("done")
  return out
})

// MUST NOT TRIGGER: the loop binding is declared with let.
export const letLoopBinding = Effect.gen(function*() {
  const out = []
  for (let item of items) {
    out.push(yield* processItem(item))
  }
  return out
})

// MUST NOT TRIGGER: for-in loop.
export const forIn = Effect.gen(function*() {
  const out = []
  for (const key in lookup) {
    out.push(yield* Effect.succeed(key))
  }
  return out
})

// MUST NOT TRIGGER: classic for loop.
export const classicFor = Effect.gen(function*() {
  const out = []
  for (let i = 0; i < items.length; i++) {
    out.push(yield* processItem(items[i]))
  }
  return out
})

// MUST NOT TRIGGER: delegating to a plain generator, not an Effect.
export const yieldedGenerator = Effect.gen(function*() {
  const out = []
  for (const item of items) {
    out.push(yield* plainGenerator(item))
  }
  return out
})

// MUST NOT TRIGGER: plain generator function, not an Effect generator.
export function* plainGeneratorFunction() {
  const out = []
  for (const item of items) {
    out.push(yield* plainGenerator(item))
  }
  return out
}

// MUST NOT TRIGGER: plain generator nested inside an Effect generator.
export const nestedPlainGenerator = Effect.gen(function*() {
  function* inner() {
    const out = []
    for (const item of items) {
      out.push(yield* plainGenerator(item))
    }
    return out
  }
  return inner
})

// MUST NOT TRIGGER: already using Effect.forEach.
export const alreadyForEach = Effect.gen(function*() {
  const results = yield* Effect.forEach(items, processItem)
  return results
})
