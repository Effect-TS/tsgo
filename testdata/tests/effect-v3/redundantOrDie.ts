// @effect-v3
// @effect-diagnostics redundantOrDie:warning
// @effect-diagnostics unnecessaryEffectGen:off
import * as Effect from "effect/Effect"

declare const first: Effect.Effect<number, unknown>
declare const second: Effect.Effect<string, unknown>
declare const third: Effect.Effect<boolean, unknown>

export const shouldReport = Effect.gen(function*() {
  const a = yield* first.pipe(Effect.orDie)
  const b = yield* second.pipe(Effect.orDie)

  return [a, b] as const
})

export const shouldReportEffectFn = Effect.fn("service.method")(function*() {
  const a = yield* first.pipe(Effect.orDie)
  const b = yield* second.pipe(Effect.orDie)

  return [a, b] as const
})

export const shouldReportMixedStyles = Effect.gen(function*() {
  const a = yield* first.pipe(Effect.orDie)
  const b = yield* Effect.orDie(second)

  return [a, b] as const
})

export const shouldNotReportMissingOrDie = Effect.gen(function*() {
  const a = yield* first.pipe(Effect.orDie)
  const b = yield* second

  return [a, b] as const
})

export const shouldNotReportOneYield = Effect.gen(function*() {
  return yield* first.pipe(Effect.orDie)
})

export const shouldNotReportNonTrailingOrDie = Effect.gen(function*() {
  const a = yield* first.pipe(
    Effect.orDie,
    Effect.map((value) => value + 1)
  )
  const b = yield* second.pipe(Effect.orDie)

  return [a, b] as const
})

export const shouldNotReportNestedMissingOrDie = Effect.gen(function*() {
  yield* first.pipe(Effect.orDie)

  yield* Effect.gen(function*() {
    yield* second.pipe(Effect.orDie)
    yield* third
  })
})
