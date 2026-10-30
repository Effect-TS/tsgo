// @effect-v3
// @effect-diagnostics *:off
// @effect-diagnostics yieldInCallArgument:warning
import { Effect, pipe } from "effect"

declare const load: Effect.Effect<number>
declare const loadFn: Effect.Effect<(n: number) => string>
declare const subject: Effect.Effect<number>
declare const obj: { prop: string }
declare const cond: boolean
declare const svc: { process(n: number): string } | undefined

declare function transform(n: number): string
declare function consume(n: number): void
declare function sum(...ns: Array<number>): number
declare function makeFn(options: { readonly tag: string }): (n: number) => string

// ---------------------------------------------------------------------------
// Reported, with quick fix
// ---------------------------------------------------------------------------

export const pipeableStep = Effect.gen(function*() {
  // MUST TRIGGER (fix): argument of a pipeable step, `loadFn` -> `loadedFn`
  return yield* subject.pipe(Effect.map(yield* loadFn))
})

export const pipeFunctionStep = Effect.gen(function*() {
  // MUST TRIGGER (fix): argument of a step in a pipe(...) call
  return yield* pipe(subject, Effect.map(yield* loadFn))
})

export const dataFirst = Effect.gen(function*() {
  // MUST TRIGGER (fix): data-first call
  return yield* Effect.map(subject, yield* loadFn)
})

export const curriedCallee = Effect.gen(function*() {
  // MUST TRIGGER (fix): the call holding the yield* is the callee of the outer call
  return yield* Effect.map(yield* loadFn)(subject)
})

export const pipeSubject = Effect.gen(function*() {
  // MUST TRIGGER (fix): first argument of pipe(...)
  return yield* pipe(yield* load, Effect.succeed)
})

export const insideYieldOperand = Effect.gen(function*() {
  // MUST TRIGGER (fix): only the inner yield* is a call argument
  return yield* Effect.succeed(transform(yield* load))
})

export const propertyAssignment = Effect.gen(function*() {
  // MUST TRIGGER (fix): the assignment target `obj.prop` is a property access
  obj.prop = transform(yield* load)
})

export const compoundAssignment = Effect.gen(function*() {
  let n = 0
  // MUST TRIGGER (fix): compound assignment to an identifier
  n += sum(yield* load)
  return n
})

export const oneLineBlock = Effect.gen(function*() {
  // MUST TRIGGER (fix): the const is inserted inline, inside the braces
  if (cond) { consume(yield* load) }
})

export const withJsDoc = Effect.gen(function*() {
  /** the result */
  // MUST TRIGGER (fix): the const is inserted above the JSDoc
  const result = transform(yield* load)
  return result
})

export const caseClause = Effect.gen(function*() {
  switch (cond) {
    case true:
      // MUST TRIGGER (fix): case clauses are statement lists
      consume(yield* load)
      break
  }
})

export const whileBody = Effect.gen(function*() {
  while (cond) {
    // MUST TRIGGER (fix): a braced loop body is a statement list
    consume(yield* load)
  }
})

// ---------------------------------------------------------------------------
// Reported, without quick fix
// ---------------------------------------------------------------------------

export const callCallee = Effect.gen(function*() {
  // MUST TRIGGER (no fix): the callee `makeFn(...)` runs before the effect
  return makeFn({ tag: "a" })(yield* load)
})

export const optionalMethod = Effect.gen(function*() {
  // MUST TRIGGER (no fix): with `?.` the argument only runs when `svc` is defined
  return svc?.process(yield* load)
})

export const logicalAssignment = Effect.gen(function*() {
  let n: number | undefined
  // MUST TRIGGER (no fix): `??=` only evaluates its right side when `n` is nullish
  n ??= sum(yield* load)
  return n
})

export const letDeclaration = Effect.gen(function*() {
  // MUST TRIGGER (no fix): only a single-declarator const is an allowed position
  let x = transform(yield* load)
  x += "!"
  return x
})

export const throwStatement = Effect.gen(function*() {
  // MUST TRIGGER (no fix): a throw statement is not an allowed position
  throw new Error(transform(yield* load))
})

class Holder {
  readonly svc = { process: (n: number) => String(n) }
  run() {
    return Effect.gen(this, function*() {
      // MUST TRIGGER (no fix): `this` is not a side-effect free receiver
      return this.svc.process(yield* load)
    })
  }
}
export const holder = new Holder()

// ---------------------------------------------------------------------------
// Not reported
// ---------------------------------------------------------------------------

export const asArgument = Effect.gen(function*() {
  // MUST NOT TRIGGER: the argument is an `as` expression wrapping the yield*
  return transform((yield* load) as number)
})
