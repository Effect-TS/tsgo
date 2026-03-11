import * as Effect from "effect/Effect"

// Should trigger (with fix) - curried generator IIFE
export const iife1 = Effect.fn("test")(function*() {
  yield* Effect.succeed(1)
})()

// Should trigger (with fix) - curried generator IIFE with return
export const iife2 = Effect.fn("named")(function*() {
  yield* Effect.succeed(1)
  return 42
})()

// Should trigger (with fix) - fnUntraced generator IIFE
export const iife3 = Effect.fnUntraced(function*() {
  yield* Effect.succeed(1)
})()

// Should trigger (WITHOUT fix) - arrow function (non-generator)
export const iife4 = Effect.fn("arrow")(() => Effect.succeed(1))()

// Should trigger (WITHOUT fix) - function expression (non-generator)
export const iife5 = Effect.fn("func")(function() {
  return Effect.succeed(1)
})()

// Should trigger (with fix) - generator with pipe arguments
export const iife6 = Effect.fn("piped")(function*() {
  return yield* Effect.succeed(1)
}, Effect.map((n) => n + 1))()

// Should trigger (with fix) - fnUntraced generator with pipe arguments
export const iife7 = Effect.fnUntraced(function*() {
  return yield* Effect.succeed(1)
}, Effect.map((n) => n + 1))()

// Should NOT trigger - stored for reuse (generator)
export const myFn = Effect.fn("myFn")(function*() {
  yield* Effect.succeed(1)
})

// Should NOT trigger - stored for reuse (regular function)
export const myFn2 = Effect.fn("myFn2")(() => Effect.succeed(1))

// Should NOT trigger - Effect.gen (not Effect.fn)
export const gen = Effect.gen(function*() {
  yield* Effect.succeed(1)
})

// Should NOT trigger - regular function call
export const regularFn = (n: number) => Effect.succeed(n)
export const result = regularFn(1)
